/* Public ABI-19 counterpart of native.c. No Vulkan headers or private runtime APIs. */
#include "ogpu.h"
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifndef SCENE_RASTER_BYTES
#define SCENE_RASTER_BYTES 8u
#endif
#ifdef SCENE_GENERATED
#ifdef SCENE_REUSE_CONSUMER
#error Generated scene handoff uses typed per-frame roots, not the raw-root reuse harness
#endif
#include "scene_prepare.generated.h"
#include "scene_prepare16.generated.h"
#include "scene_pair.generated.h"
#endif

#define NEED(x) do { if (!(x)) { fprintf(stderr,"Check failed line %d: %s\n",__LINE__,#x); return 0; } } while (0)
#define GPU(x) do { OgpuResult status_=(x); if(status_!=OGPU_SUCCESS) { \
    fprintf(stderr,"%s: status=%" PRId32 " native=%" PRId32 " %s\n",#x,status_,s->error.vulkan_result,s->error.message); return 0; } } while(0)

enum { GUARD=64, MODES=10 };
typedef struct { uint64_t vertices, indices, draws; uint32_t phase, empty; } SceneRoot;
/* The raw control uses this fixed layout. The generated handoff treats it as
 * logical host data and constructs typed shader arguments by generated field name. */
_Static_assert(sizeof(SceneRoot)==32 && offsetof(SceneRoot,phase)==24, "compute root layout");
_Static_assert(sizeof(OgpuDrawIndexedArguments)==20 && offsetof(OgpuDrawIndexedArguments,vertex_offset)==12, "indirect layout");
typedef struct {
    OgpuProbe *probe;
    OgpuDevice *device;
    OgpuKernel *prepare;
    OgpuRaster *raster[4];
    OgpuImage *color, *depth;
    OgpuBuffer *vertices, *indices, *draws, *upload, *readback, *geometry;
    OgpuBatch *batch;
    OgpuCompletion *completion;
    OgpuError error;
    SceneRoot root;
    uint32_t width, height, index_bytes;
    size_t pixels;
    unsigned char *cpu;
    uint32_t *shaders[3];
    uint64_t shader_bytes[3];
#ifdef SCENE_ARGUMENT_REUSE
    uint64_t argument_root[SCENE_RASTER_BYTES/8];
#endif
} Scene;
#ifdef SCENE_PUBLIC_FRONTIER
static int frontier_draw(Scene *s, unsigned pipeline, const OgpuIndexRange *indices, unsigned first);
#define SCENE_DRAW_BYTES (132u+20u*frontier_records)
#else
#define SCENE_DRAW_BYTES 168u
#endif
#define SCENE_UPLOAD_BYTES (SCENE_DRAW_BYTES>256 ? SCENE_DRAW_BYTES : 256)

static int path_join(char out[4096], const char *directory, const char *name) {
    int n=snprintf(out,4096,"%s/%s",directory,name); return n>=0 && n<4096;
}
#ifndef SCENE_GENERATED
static int read_shader(Scene *s, unsigned index, const char *directory, const char *name) {
    char path[4096]; NEED(path_join(path,directory,name));
    FILE *f=fopen(path,"rb"); if(!f) { perror(path); return 0; }
    int okay=0; long bytes=0;
    if(fseek(f,0,SEEK_END) || (bytes=ftell(f))<20 || bytes>1024*1024 || bytes%4 || fseek(f,0,SEEK_SET)) goto done;
    s->shaders[index]=malloc((size_t)bytes); if(!s->shaders[index]) goto done;
    if(fread(s->shaders[index],1,(size_t)bytes,f)!=(size_t)bytes || ferror(f)) goto done;
    s->shader_bytes[index]=(uint64_t)bytes; okay=1;
done:
    if(fclose(f)) okay=0; return okay;
}
#endif
#if !defined(SCENE_REUSE_CONSUMER) || (defined(SCENE_PUBLIC_FRONTIER) && !defined(FRONTIER_REUSE))
static int write_file(const char *directory, const char *name, const void *data, size_t bytes) {
    char path[4096]; NEED(path_join(path,directory,name));
    FILE *f=fopen(path,"wb"); if(!f) { perror(path); return 0; }
    int okay=fwrite(data,1,bytes,f)==bytes && !ferror(f);
    if(fclose(f)) okay=0; return okay;
}
#endif
static int buffer_create(Scene *s, OgpuBuffer **out, uint64_t bytes, uint32_t placement, uint32_t extra_usage) {
    OgpuBufferDesc desc={bytes,placement,extra_usage};
    GPU(ogpu_buffer_create(s->device,&desc,out,&s->error)); return 1;
}
static int submit_wait(Scene *s) {
    GPU(ogpu_batch_submit(s->batch,&s->completion,&s->error));
    ogpu_batch_destroy(s->batch); s->batch=NULL;
    GPU(ogpu_completion_wait(s->completion,&s->error));
    ogpu_completion_destroy(s->completion); s->completion=NULL; return 1;
}
static uint32_t scene_compute_bytes(const Scene *s) {
#ifdef SCENE_GENERATED
    return s->index_bytes==2 ? scene_prepare16_push_size : scene_prepare_push_size;
#else
    (void)s;return sizeof(SceneRoot);
#endif
}
static int scene_create_context(Scene *s, const char *directory) {
    GPU(ogpu_probe_create(OGPU_ABI_VERSION,&s->probe,&s->error));
    uint32_t count=0; GPU(ogpu_probe_device_count(s->probe,&count));
    NEED(count==1); /* Same unambiguous device selection as the native control. */
    OgpuDeviceInfo info; GPU(ogpu_probe_device_info(s->probe,0,&info));
    NEED(info.backend==OGPU_BACKEND_VULKAN);
    GPU(ogpu_device_create_graphics(s->probe,0,&s->device,&s->error));
    printf("DEVICE {\"vendor\":%u,\"device\":%u,\"api\":[%u,%u,%u]}\n",
        info.vendor_id,info.device_id,info.vulkan_api_major,info.vulkan_api_minor,info.vulkan_api_patch);
    printf("PUBLIC_SCENE {\"abi\":%u,\"index_bytes\":%u,\"gpu_generated\":true}\n",OGPU_ABI_VERSION,s->index_bytes);
#ifdef SCENE_GENERATED
    (void)directory;
    OgpuCapabilities caps;OgpuDeviceLimits limits;
    GPU(ogpu_device_capabilities(s->device,&caps,&s->error));GPU(ogpu_device_limits(s->device,&limits,&s->error));
    NEED(scene_pair_vertex_compatible(&caps,&limits) && scene_pair_fragment_compatible(&caps,&limits));
    NEED(s->index_bytes==2 ? scene_prepare16_compatible(&caps,&limits) : scene_prepare_compatible(&caps,&limits));
    OgpuShaderDesc shaders[]={s->index_bytes==2 ? scene_prepare16_shader() : scene_prepare_shader(),
        scene_pair_vertex_shader(),scene_pair_fragment_shader()};
    uint32_t raster_bytes=scene_pair_push_size;
    printf("GENERATED_SCENE {\"compute_push_bytes\":%u,\"vertex_push_bytes\":%u}\n",scene_compute_bytes(s),raster_bytes);
#else
    NEED(read_shader(s,0,directory,"compute.spv") && read_shader(s,1,directory,"vertex.spv") && read_shader(s,2,directory,"fragment.spv"));
    OgpuShaderDesc shaders[3]={0};
    for(unsigned i=0;i<3;++i) { shaders[i].code=s->shaders[i]; shaders[i].code_size=s->shader_bytes[i]; shaders[i].format=OGPU_SHADER_SPIRV; }
    uint32_t raster_bytes=SCENE_RASTER_BYTES;
#endif
    GPU(ogpu_kernel_create(s->device,&shaders[0],scene_compute_bytes(s),&s->prepare,&s->error));
    for(unsigned i=0;i<4;++i) {
        OgpuRasterDesc desc={raster_bytes,OGPU_TOPOLOGY_TRIANGLE_LIST,OGPU_FORMAT_RGBA8_UNORM,OGPU_FORMAT_D32_FLOAT,
            i!=1,i!=2,i==3 ? OGPU_COMPARE_ALWAYS : OGPU_COMPARE_LESS,0};
        GPU(ogpu_raster_create(s->device,&shaders[1],&shaders[2],&desc,&s->raster[i],&s->error));
    }
    return 1;
}
static int scene_create_resources(Scene *s) {
    OgpuImageDesc color={OGPU_IMAGE_2D,s->width,s->height,OGPU_FORMAT_RGBA8_UNORM,OGPU_IMAGE_USAGE_COLOR|OGPU_IMAGE_USAGE_COPY_SRC,0};
    OgpuImageDesc depth={OGPU_IMAGE_2D,s->width,s->height,OGPU_FORMAT_D32_FLOAT,OGPU_IMAGE_USAGE_DEPTH|OGPU_IMAGE_USAGE_COPY_SRC,0};
    GPU(ogpu_image_check_support(s->device,&color,&s->error)); GPU(ogpu_image_check_support(s->device,&depth,&s->error));
    GPU(ogpu_image_create(s->device,&color,&s->color,&s->error)); GPU(ogpu_image_create(s->device,&depth,&s->depth,&s->error));
    NEED(buffer_create(s,&s->vertices,256,OGPU_MEMORY_DEVICE,0)
        && buffer_create(s,&s->indices,160,OGPU_MEMORY_DEVICE,OGPU_BUFFER_INDEX)
        && buffer_create(s,&s->draws,SCENE_DRAW_BYTES,OGPU_MEMORY_DEVICE,0)
        && buffer_create(s,&s->upload,SCENE_UPLOAD_BYTES,OGPU_MEMORY_HOST,0)
        && buffer_create(s,&s->readback,2*s->pixels+4*GUARD,OGPU_MEMORY_HOST,0)
        && buffer_create(s,&s->geometry,416+SCENE_DRAW_BYTES,OGPU_MEMORY_HOST,0));
    GPU(ogpu_buffer_device_address(s->vertices,&s->root.vertices,&s->error)); s->root.vertices+=GUARD;
    GPU(ogpu_buffer_device_address(s->indices,&s->root.indices,&s->error)); s->root.indices+=GUARD;
    GPU(ogpu_buffer_device_address(s->draws,&s->root.draws,&s->error)); s->root.draws+=GUARD;
    s->cpu=malloc(2*s->pixels+4*GUARD); NEED(s->cpu); memset(s->cpu,0xa5,2*s->pixels+4*GUARD);
    GPU(ogpu_buffer_write(s->upload,0,s->cpu,SCENE_UPLOAD_BYTES,&s->error));
    GPU(ogpu_buffer_write(s->readback,0,s->cpu,2*s->pixels+4*GUARD,&s->error));
    GPU(ogpu_batch_create(s->device,&s->batch,&s->error));
    GPU(ogpu_batch_copy_buffer(s->batch,s->upload,0,s->vertices,0,256,&s->error));
    GPU(ogpu_batch_copy_buffer(s->batch,s->upload,0,s->indices,0,160,&s->error));
    GPU(ogpu_batch_copy_buffer(s->batch,s->upload,0,s->draws,0,SCENE_DRAW_BYTES,&s->error));
    return submit_wait(s);
}
static int scene_pass(Scene *s, unsigned pipeline, int color_load, int depth_load, float clear,
                      unsigned first, unsigned count) {
    OgpuRenderingDesc desc={.color={s->color,color_load ? OGPU_ATTACHMENT_LOAD : OGPU_ATTACHMENT_CLEAR,
        OGPU_STORE_STORE,{0,0,0,1}},.depth={s->depth,depth_load ? OGPU_ATTACHMENT_LOAD : OGPU_ATTACHMENT_CLEAR,OGPU_STORE_STORE,clear,0}};
    GPU(ogpu_batch_begin_rendering(s->batch,&desc,&s->error));
    OgpuIndexRange indices={s->indices,GUARD,8*s->index_bytes,
        s->index_bytes==2 ? OGPU_INDEX_UINT16 : OGPU_INDEX_UINT32,0};
#ifdef SCENE_PUBLIC_FRONTIER
    (void)count;NEED(frontier_draw(s,pipeline,&indices,first));
#else
#ifdef SCENE_GENERATED
    const Scene_pair_vertexArguments arguments={.arg_vertices=s->root.vertices};
    GPU(ogpu_batch_set_arguments(s->batch,0,&arguments,sizeof(arguments),&s->error));
#else
    const void *draw_root=&s->root.vertices;uint32_t draw_bytes=8;
#endif
    for(unsigned i=0;i<count;++i) {
        unsigned which=(first+i)%2;
#ifdef SCENE_GENERATED
        GPU(ogpu_batch_draw_indexed_indirect_current(s->batch,s->raster[pipeline],&indices,
            &(OgpuIndirectRange){.buffer=s->draws,.offset=GUARD+20*which,.stride_bytes=20,.max_draw_count=1},&s->error));
#else
        GPU(ogpu_batch_draw_indexed_indirect(s->batch, s->raster[pipeline], &indices, &(OgpuIndirectRange){.buffer=s->draws,.offset=GUARD+20*which,.stride_bytes=20,.max_draw_count=1}, draw_root, draw_bytes, &s->error));
#endif
    }
#endif
    GPU(ogpu_batch_end_rendering(s->batch,&s->error)); return 1;
}
static int scene_commands(Scene *s, unsigned mode, const void *compute_root) {
    GPU(ogpu_batch_barrier(s->batch,OGPU_ACCESS_COMPUTE_WRITE|OGPU_ACCESS_VERTEX_READ|OGPU_ACCESS_INDEX_READ|
        OGPU_ACCESS_INDIRECT_READ|OGPU_ACCESS_TRANSFER_READ|OGPU_ACCESS_TRANSFER_WRITE,OGPU_ACCESS_COMPUTE_WRITE,&s->error));
#ifdef SCENE_GENERATED
    GPU(ogpu_batch_set_arguments(s->batch,0,compute_root,scene_compute_bytes(s),&s->error));
    GPU(ogpu_batch_dispatch_current(s->batch,s->prepare,1,1,1,&s->error));
#else
    GPU(ogpu_batch_dispatch(s->batch,s->prepare,1,1,1,compute_root,scene_compute_bytes(s),&s->error));
#endif
    GPU(ogpu_batch_barrier(s->batch,OGPU_ACCESS_COMPUTE_WRITE,
        OGPU_ACCESS_INDEX_READ|OGPU_ACCESS_VERTEX_READ|OGPU_ACCESS_INDIRECT_READ,&s->error));
    /* Match the native control's per-frame discard policy, not a requirement to
       reinitialize layout on every rendering scope in the public contract. */
    GPU(ogpu_batch_discard_image(s->batch,s->color,&s->error));
    GPU(ogpu_batch_discard_image(s->batch,s->depth,&s->error));
#ifdef SCENE_RANGE_SCOPES
    NEED(mode==0 && frontier_strategy==0);
    for(unsigned i=0;i<frontier_records;++i) {
        if(i) GPU(ogpu_batch_barrier(s->batch,OGPU_ACCESS_COLOR_WRITE|OGPU_ACCESS_DEPTH_WRITE,
            OGPU_ACCESS_COLOR_READ|OGPU_ACCESS_COLOR_WRITE|OGPU_ACCESS_DEPTH_READ|OGPU_ACCESS_DEPTH_WRITE,&s->error));
        NEED(scene_pass(s,0,i!=0,i!=0,1,i,1));
    }
#else
    if(mode>=6 && mode<=8) {
        NEED(scene_pass(s,0,0,0,1,1,1));
        GPU(ogpu_batch_barrier(s->batch,OGPU_ACCESS_COLOR_WRITE|OGPU_ACCESS_DEPTH_WRITE,
            OGPU_ACCESS_COLOR_READ|OGPU_ACCESS_COLOR_WRITE|OGPU_ACCESS_DEPTH_READ|OGPU_ACCESS_DEPTH_WRITE,&s->error));
        NEED(scene_pass(s,0,mode!=7,mode!=8,1,0,1));
    } else NEED(scene_pass(s,mode>=2 && mode<=4 ? mode-1 : 0,0,0,mode==5 ? 0 : 1,mode>=1 && mode<=4,2));
#endif
    GPU(ogpu_batch_copy_image_to_buffer(s->batch,s->color,s->readback,GUARD,&s->error));
    GPU(ogpu_batch_copy_image_to_buffer(s->batch,s->depth,s->readback,s->pixels+3*GUARD,&s->error));
    GPU(ogpu_batch_copy_buffer(s->batch,s->vertices,0,s->geometry,0,256,&s->error));
    GPU(ogpu_batch_copy_buffer(s->batch,s->indices,0,s->geometry,256,160,&s->error));
    GPU(ogpu_batch_copy_buffer(s->batch,s->draws,0,s->geometry,416,SCENE_DRAW_BYTES,&s->error));
    return 1;
}
#ifndef SCENE_REUSE_CONSUMER
static int scene_frame(Scene *s, unsigned mode, unsigned phase, unsigned frame, const char *directory) {
    GPU(ogpu_batch_create(s->device,&s->batch,&s->error));
    s->root.phase=phase; s->root.empty=mode==9;
#ifdef SCENE_GENERATED
    if(s->index_bytes==2) {
        const Scene_prepare16Arguments root={.arg_vertices=s->root.vertices,.arg_indices=s->root.indices,
            .arg_draws=s->root.draws,.arg_phase=phase,.arg_empty=mode==9};
        NEED(scene_commands(s,mode,&root));
    } else {
        const Scene_prepareArguments root={.arg_vertices=s->root.vertices,.arg_indices=s->root.indices,
            .arg_draws=s->root.draws,.arg_phase=phase,.arg_empty=mode==9};
        NEED(scene_commands(s,mode,&root));
    }
#else
    NEED(scene_commands(s,mode,&s->root));
#endif
    NEED(submit_wait(s));
    GPU(ogpu_buffer_read(s->readback,0,s->cpu,2*s->pixels+4*GUARD,&s->error));
    char name[128]; snprintf(name,sizeof(name),"mode-%u-frame-%u.images",mode,frame);
    NEED(write_file(directory,name,s->cpu,2*s->pixels+4*GUARD));
    GPU(ogpu_buffer_read(s->geometry,0,s->cpu,584,&s->error));
    snprintf(name,sizeof(name),"mode-%u-frame-%u.geometry",mode,frame);
    NEED(write_file(directory,name,s->cpu,584));
    printf("SCENE_FRAME {\"mode\":%u,\"phase\":%u,\"frame\":%u}\n",mode,phase,frame); return 1;
}
#endif
static void scene_destroy_resources(Scene *s) {
    /* Drain an accepted submission even on read/wait errors, before releasing
       any allocation reachable only through compute/vertex root addresses. */
    ogpu_completion_destroy(s->completion); ogpu_batch_destroy(s->batch);
    ogpu_buffer_destroy(s->geometry); ogpu_buffer_destroy(s->readback); ogpu_buffer_destroy(s->upload);
    ogpu_buffer_destroy(s->draws); ogpu_buffer_destroy(s->indices); ogpu_buffer_destroy(s->vertices);
    ogpu_image_destroy(s->depth); ogpu_image_destroy(s->color);
    free(s->cpu);
}
static void scene_destroy_context(Scene *s) {
    for(unsigned i=0;i<4;++i) ogpu_raster_destroy(s->raster[i]);
    ogpu_kernel_destroy(s->prepare); ogpu_device_destroy(s->device); ogpu_probe_destroy(s->probe);
    for(unsigned i=0;i<3;++i) free(s->shaders[i]);
}
#ifndef SCENE_REUSE_CONSUMER
int main(int argc, char **argv) {
    if(argc!=5 && argc!=6) { fprintf(stderr,"Usage: public width height shaders output-directory [16|32]\n"); return 1; }
    Scene s={.index_bytes=4};
    if(argc==6) {
        if(!strcmp(argv[5],"16")) s.index_bytes=2;
        else if(strcmp(argv[5],"32")) return 1;
    }
    if(!strcmp(argv[1],"257") && !strcmp(argv[2],"193")) { s.width=257;s.height=193; }
    else if(!strcmp(argv[1],"640") && !strcmp(argv[2],"360")) { s.width=640;s.height=360; }
    else if(!strcmp(argv[1],"1280") && !strcmp(argv[2],"720")) { s.width=1280;s.height=720; }
    else return 1;
    s.pixels=(size_t)s.width*s.height*4; int okay=0;
    if(!scene_create_context(&s,argv[3]) || !scene_create_resources(&s)) goto done;
    for(unsigned mode=0;mode<MODES;++mode) for(unsigned frame=0;frame<3;++frame)
        if(!scene_frame(&s,mode,frame==1,frame,argv[4])) goto done;
    okay=1;
done:
    scene_destroy_resources(&s); scene_destroy_context(&s);
    if(okay) puts("Public indexed/depth frames drained; CPU oracle must independently accept outputs");
    return !okay;
}
#endif
