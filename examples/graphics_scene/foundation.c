/* Matched indexed/depth scene; public foundation API only. No runtime retention,
 * implicit placement, layout tracking or completion-receipt policy. */
#include "ogpu_next.h"
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifndef SCENE_DRAW_BYTES
#define SCENE_DRAW_BYTES 168u
#endif
#ifdef SCENE_GENERATED
#ifdef SCENE_REUSE_CONSUMER
#include <foundation_scene_reuse_prepare.generated.h>
#include <foundation_scene_reuse_prepare16.generated.h>
#else
#include <foundation_scene_prepare.generated.h>
#include <foundation_scene_prepare16.generated.h>
#endif
#include <foundation_scene_pair.generated.h>
#endif
#define HEADER(type,kind) {kind,OGPU_NEXT_VERSION,sizeof(type),0,NULL}
#define NEED(x) do { if (!(x)) { fprintf(stderr,"line %d: %s\n",__LINE__,#x); return 0; } } while (0)
#define GPU(x) do { ogpu_next_status status_=(x); if(status_!=OGPU_NEXT_OK) { \
    fprintf(stderr,"%s: status=%" PRId32 "\n",#x,status_); return 0; } } while(0)
enum { GUARD=64 };
enum { VERTICES, INDICES, DRAWS, UPLOAD, READBACK, GEOMETRY, CONTROL, BUFFER_COUNT };
typedef struct { uint64_t vertices, indices, draws; uint32_t phase, empty; } SceneRoot;
_Static_assert(sizeof(SceneRoot)==32 && offsetof(SceneRoot,phase)==24,"compute layout");
_Static_assert(sizeof(ogpu_next_draw_indexed_arguments)==20,"indirect layout");
typedef struct {
    ogpu_next_discovery *discovery;
    ogpu_next_device *device;
    ogpu_next_queue *queue;
    ogpu_next_arena *arena;
    ogpu_next_encoder *encoder;
    ogpu_next_timeline *done;
    ogpu_next_executable *prepare, *raster[4];
    ogpu_next_memory *buffers[BUFFER_COUNT], *backing[2];
    ogpu_next_image *images[2];
    ogpu_next_view *views[2];
    ogpu_next_mapping maps[BUFFER_COUNT];
    uint64_t sizes[BUFFER_COUNT], serial;
    ogpu_next_host_requirements barrier_scratch, render_scratch, submit_scratch;
    void *bs, *rs, *ss;
    uint32_t width, height, index_bytes, domain;
    size_t pixels;
    int pending;
    SceneRoot root;
#ifdef SCENE_REUSE_CONSUMER
    uint64_t reuse_root[4];
#endif
    void *code[3]; uint64_t code_size[3];
} Scene;
static ogpu_next_query query(uint32_t kind,void *data,uint32_t count,uint32_t size) {
    return (ogpu_next_query){HEADER(ogpu_next_query,kind),data,count,0,size,0};
}
static ogpu_next_span span(Scene *s,unsigned buffer,uint64_t offset,uint64_t bytes) {
    return (ogpu_next_span){s->buffers[buffer],offset,bytes};
}
#ifndef SCENE_GENERATED
static int shader_read(Scene *s,unsigned i,const char *directory,const char *name) {
    char path[4096]; int n=snprintf(path,sizeof(path),"%s/%s",directory,name); NEED(n>=0 && n<(int)sizeof(path));
    FILE *f=fopen(path,"rb"); if(!f) { perror(path); return 0; }
    int okay=0; long bytes=0;
    if(fseek(f,0,SEEK_END) || (bytes=ftell(f))<20 || bytes>1024*1024 || bytes%4 || fseek(f,0,SEEK_SET)) goto done;
    s->code[i]=malloc((size_t)bytes); if(!s->code[i]) goto done;
    if(fread(s->code[i],1,(size_t)bytes,f)!=(size_t)bytes || ferror(f)) goto done;
    s->code_size[i]=(uint64_t)bytes; okay=1;
done:
    if(fclose(f)) okay=0;
    return okay;
}
#endif
/* Explicit consumer preference, not an OGPU allocator: require properties, then
 * prefer non-host-visible LOCAL storage when suitable. UMA remains eligible. */
static int memory_type(Scene *s,const ogpu_next_requirements *req,uint32_t properties,uint32_t *out) {
    ogpu_next_memory_type_info types[32]; ogpu_next_query q=query(OGPU_NEXT_QUERY_MEMORY_TYPES,types,32,sizeof(types[0]));
    GPU(ogpu_next_device_query(s->device,&q)); *out=UINT32_MAX; unsigned best=0;
    for(uint32_t i=0;i<req->compatible_type_count;++i) for(uint32_t j=0;j<q.count;++j) {
        uint32_t p=types[j].properties;
        if(types[j].id!=req->compatible_types[i] || (p&properties)!=properties) continue;
        unsigned score=1+((properties&OGPU_NEXT_MEMORY_LOCAL) && !(p&OGPU_NEXT_MEMORY_HOST_VISIBLE));
        if(score>best) { best=score; *out=types[j].id; }
    }
    NEED(*out!=UINT32_MAX); return 1;
}
static int buffer_create(Scene *s,unsigned i,uint64_t size,uint64_t usage,uint32_t properties) {
    ogpu_next_memory_desc md={HEADER(ogpu_next_memory_desc,OGPU_NEXT_MEMORY_DESC),size,8,usage,UINT32_MAX,OGPU_NEXT_MEMORY_LINEAR,NULL,0,0};
    uint32_t types[32]; ogpu_next_requirements req={.compatible_type_capacity=32,.compatible_types=types};
    GPU(ogpu_next_memory_requirements(s->device,&md,&req)); NEED(memory_type(s,&req,properties,&md.memory_type));
    GPU(ogpu_next_memory_create(s->device,&md,&s->buffers[i])); s->sizes[i]=size;
    if(properties&OGPU_NEXT_MEMORY_HOST_VISIBLE) GPU(ogpu_next_memory_map(span(s,i,0,size),&s->maps[i]));
    return 1;
}
static int context(Scene *s,const char *directory) {
    GPU(ogpu_next_discovery_create(OGPU_NEXT_VERSION,&s->discovery)); NEED(ogpu_next_discovery_count(s->discovery)==1);
    const ogpu_next_adapter *adapter=ogpu_next_discovery_adapter(s->discovery,0);
    ogpu_next_adapter_info info; ogpu_next_query q=query(OGPU_NEXT_QUERY_INFO,&info,1,sizeof(info)); GPU(ogpu_next_adapter_query(adapter,&q));
    ogpu_next_queue_info queues[32]; q=query(OGPU_NEXT_QUERY_QUEUES,queues,32,sizeof(queues[0])); GPU(ogpu_next_adapter_query(adapter,&q));
    s->domain=UINT32_MAX;
    for(uint32_t i=0;i<q.count;++i) if(queues[i].count &&
        (queues[i].flags&(OGPU_NEXT_QUEUE_GRAPHICS|OGPU_NEXT_QUEUE_COMPUTE))==(OGPU_NEXT_QUEUE_GRAPHICS|OGPU_NEXT_QUEUE_COMPUTE)) {
        s->domain=queues[i].domain; break;
    }
    NEED(s->domain!=UINT32_MAX);
    ogpu_next_queue_request request={s->domain,1,0.5f};
    uint64_t required=OGPU_NEXT_FEATURE_RASTER;
#ifdef SCENE_GENERATED
#ifdef SCENE_REUSE_CONSUMER
    required=scene_pair_required_features | (s->index_bytes==2 ? scene_reuse_prepare16_required_features : scene_reuse_prepare_required_features);
#else
    required=scene_pair_required_features | (s->index_bytes==2 ? scene_prepare16_required_features : scene_prepare_required_features);
#endif
#endif
    ogpu_next_device_desc dd={HEADER(ogpu_next_device_desc,OGPU_NEXT_DEVICE_DESC),&request,1,0,required};
    GPU(ogpu_next_device_create(adapter,&dd,&s->device)); s->queue=ogpu_next_device_queue(s->device,s->domain,0); NEED(s->queue);
    printf("DEVICE {\"vendor\":%u,\"device\":%u,\"api\":[%u,%u,%u]}\n",info.vendor_id,info.device_id,
        (info.native_api_version>>22)&127,(info.native_api_version>>12)&1023,info.native_api_version&4095);
    printf("PUBLIC_SCENE {\"foundation\":%u,\"index_bytes\":%u,\"gpu_generated\":true}\n",OGPU_NEXT_VERSION,s->index_bytes);
#ifdef SCENE_GENERATED
    (void)directory;
    ogpu_next_feature_info features; ogpu_next_execution_limits limits;
    q=query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features)); GPU(ogpu_next_device_query(s->device,&q));
    q=query(OGPU_NEXT_QUERY_EXECUTION_LIMITS,&limits,1,sizeof(limits)); GPU(ogpu_next_device_query(s->device,&q));
    NEED(scene_pair_compatible(&features,&limits));
#ifdef SCENE_REUSE_CONSUMER
    NEED(s->index_bytes==2 ? scene_reuse_prepare16_compatible(&features,&limits) : scene_reuse_prepare_compatible(&features,&limits));
    ogpu_next_shader shaders[3]; shaders[0]=s->index_bytes==2 ? scene_reuse_prepare16_shader() : scene_reuse_prepare_shader(); scene_pair_shaders(&shaders[1]);
    const ogpu_next_shader_requirements *compute_req=s->index_bytes==2 ? &scene_reuse_prepare16_requirements : &scene_reuse_prepare_requirements;
    uint32_t compute_bytes=s->index_bytes==2 ? scene_reuse_prepare16_push_size : scene_reuse_prepare_push_size;
#else
    NEED(s->index_bytes==2 ? scene_prepare16_compatible(&features,&limits) : scene_prepare_compatible(&features,&limits));
    ogpu_next_shader shaders[3]; shaders[0]=s->index_bytes==2 ? scene_prepare16_shader() : scene_prepare_shader(); scene_pair_shaders(&shaders[1]);
    const ogpu_next_shader_requirements *compute_req=s->index_bytes==2 ? &scene_prepare16_requirements : &scene_prepare_requirements;
    uint32_t compute_bytes=s->index_bytes==2 ? scene_prepare16_push_size : scene_prepare_push_size;
#endif
    ogpu_next_executable_desc ed={HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_COMPUTE,1,shaders,NULL,0,&compute_req->header,NULL,0,0};
    printf("GENERATED_SCENE {\"compute_push_bytes\":%u,\"vertex_push_bytes\":%u}\n",
        compute_bytes,scene_pair_push_size);
#else
    NEED(shader_read(s,0,directory,"compute.spv") && shader_read(s,1,directory,"vertex.spv") && shader_read(s,2,directory,"fragment.spv"));
    /* Metadata is checked against compiler reflection by run.py. Same native
     * shader bytes as the Vulkan control, with no runtime shader inspection. */
    ogpu_next_argument_interface compute={HEADER(ogpu_next_argument_interface,OGPU_NEXT_ARGUMENT_INTERFACE),32,0,NULL};
    ogpu_next_argument_interface raster={HEADER(ogpu_next_argument_interface,OGPU_NEXT_ARGUMENT_INTERFACE),8,0,NULL};
    ogpu_next_shader shaders[]={
        {OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_SHADER_SPIRV,{s->code[0],s->code_size[0]},"main",&compute.header,NULL,NULL},
        {OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_SHADER_SPIRV,{s->code[1],s->code_size[1]},"main",&raster.header,NULL,NULL},
        {OGPU_NEXT_STAGE_FRAGMENT,OGPU_NEXT_SHADER_SPIRV,{s->code[2],s->code_size[2]},"main",&raster.header,NULL,NULL}};
    ogpu_next_shader_requirements req={HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),0,{8,1,1},0};
    ogpu_next_executable_desc ed={HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_COMPUTE,1,shaders,NULL,0,&req.header,NULL,0,0};
#endif
    GPU(ogpu_next_executable_create(s->device,&ed,&s->prepare));
    ogpu_next_color_state color={OGPU_NEXT_RGBA8_UNORM,15,0,1,0,0,1,0,0};
    ogpu_next_graphics_state gs={HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),OGPU_NEXT_TRIANGLES,OGPU_NEXT_CULL_NONE,
        OGPU_NEXT_FRONT_CCW,1,1,&color,OGPU_NEXT_D32_FLOAT,1,1,OGPU_NEXT_COMPARE_LESS,{0},0,{0},{0},NULL,NULL,0,0};
#ifdef SCENE_GENERATED
    ed.requirements=&scene_pair_requirements.header;
#else
    req.features=OGPU_NEXT_FEATURE_RASTER; memset(req.local_size,0,sizeof(req.local_size));
#endif
    ed.kind=OGPU_NEXT_EXECUTABLE_GRAPHICS; ed.shader_count=2; ed.shaders=&shaders[1]; ed.static_state=&gs.header;
    ed.dynamic_state=OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR;
    for(unsigned i=0;i<4;++i) {
        gs.depth_test=i!=1; gs.depth_write=i!=2; gs.depth_compare=i==3 ? OGPU_NEXT_COMPARE_ALWAYS : OGPU_NEXT_COMPARE_LESS;
        GPU(ogpu_next_executable_create(s->device,&ed,&s->raster[i]));
    }
    return 1;
}
static int storage(Scene *s) {
    GPU(ogpu_next_barrier_scratch_requirements(0,1,&s->barrier_scratch)); GPU(ogpu_next_render_scratch_requirements(1,&s->render_scratch));
    GPU(ogpu_next_submit_scratch_requirements(1,0,1,&s->submit_scratch));
    NEED(s->barrier_scratch.alignment<=_Alignof(max_align_t) && s->render_scratch.alignment<=_Alignof(max_align_t) && s->submit_scratch.alignment<=_Alignof(max_align_t));
    s->bs=malloc(s->barrier_scratch.size); s->rs=malloc(s->render_scratch.size); s->ss=malloc(s->submit_scratch.size); NEED(s->bs && s->rs && s->ss);
    ogpu_next_arena_desc ad={HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),s->domain,1,OGPU_NEXT_PRIMARY,0};
    GPU(ogpu_next_arena_create(s->device,&ad,&s->arena)); GPU(ogpu_next_timeline_create(s->device,0,&s->done)); return 1;
}
static int begin(Scene *s) {
    ogpu_next_recording_desc rd={HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    GPU(ogpu_next_commands_begin(s->arena,&rd,&s->encoder)); return 1;
}
static int submit_wait(Scene *s) {
    ogpu_next_list *list=NULL; ogpu_next_status status=ogpu_next_commands_end(s->encoder,&list); s->encoder=NULL; GPU(status);
    ogpu_next_sync_point signal={{s->done,++s->serial},OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit={HEADER(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),1,0,1,&list,NULL,&signal,s->ss,s->submit_scratch.size};
    status=ogpu_next_queue_submit(s->queue,&submit);
    /* A backend failure can leave acceptance unknown. Fail-stop; never release
     * GPU operands or pretend that a failed wait establishes completion. */
    s->pending=status==OGPU_NEXT_OK || status==OGPU_NEXT_BACKEND_ERROR || status==OGPU_NEXT_DEVICE_LOST;
    GPU(status); GPU(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); s->pending=0;
    GPU(ogpu_next_arena_reset(s->arena)); return 1;
}
static void dependency(Scene *s,uint64_t before,uint64_t after,uint64_t src,uint64_t dst) {
    ogpu_next_dependency dep={HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),before,after,src,dst,0,0,0,NULL,NULL,NULL,0};
    ogpu_next_barrier(s->encoder,&dep);
}
static void transition(Scene *s,unsigned i,int discard) {
    uint32_t aspect=i ? OGPU_NEXT_ASPECT_DEPTH : OGPU_NEXT_ASPECT_COLOR;
    uint32_t state=i ? OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT : OGPU_NEXT_STATE_COLOR_ATTACHMENT;
    uint64_t access=i ? OGPU_NEXT_ACCESS_DEPTH_READ|OGPU_NEXT_ACCESS_DEPTH_WRITE : OGPU_NEXT_ACCESS_COLOR_WRITE;
    uint64_t stage=i ? OGPU_NEXT_STAGE_DEPTH : OGPU_NEXT_STAGE_COLOR;
    ogpu_next_image_barrier b={s->images[i],{aspect,0,1,0,1},discard ? OGPU_NEXT_ACCESS_COPY_READ : access,
        discard ? access : OGPU_NEXT_ACCESS_COPY_READ,discard ? OGPU_NEXT_STATE_UNDEFINED : state,
        discard ? state : OGPU_NEXT_STATE_COPY_SRC,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,(uint32_t)discard};
    ogpu_next_dependency dep={HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),discard ? OGPU_NEXT_STAGE_COPY : stage,
        discard ? stage : OGPU_NEXT_STAGE_COPY,0,0,0,1,0,NULL,&b,s->bs,s->barrier_scratch.size}; ogpu_next_barrier(s->encoder,&dep);
}
static int resources(Scene *s) {
    for(unsigned i=0;i<2;++i) {
        uint32_t format=i ? OGPU_NEXT_D32_FLOAT : OGPU_NEXT_RGBA8_UNORM;
        uint64_t usage=i ? OGPU_NEXT_IMAGE_DEPTH_STENCIL_ATTACHMENT : OGPU_NEXT_IMAGE_COLOR_ATTACHMENT;
        ogpu_next_image_desc id={HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),format,OGPU_NEXT_IMAGE_2D,1,1,1,
            {s->width,s->height,1},usage|OGPU_NEXT_IMAGE_COPY_SRC,0,NULL,0,0,NULL};
        uint32_t types[32],type; ogpu_next_requirements req={.compatible_type_capacity=32,.compatible_types=types};
        GPU(ogpu_next_image_requirements(s->device,&id,&req)); NEED(memory_type(s,&req,OGPU_NEXT_MEMORY_LOCAL,&type));
        GPU(ogpu_next_image_create_unbound(s->device,&id,&s->images[i])); GPU(ogpu_next_memory_create_dedicated_image(s->images[i],type,&s->backing[i]));
        GPU(ogpu_next_image_bind(s->images[i],(ogpu_next_span){s->backing[i],0,req.size}));
        ogpu_next_view_desc vd={HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),format,OGPU_NEXT_VIEW_2D,usage,{0,0,0,0},
            {i ? OGPU_NEXT_ASPECT_DEPTH : OGPU_NEXT_ASPECT_COLOR,0,1,0,1}};
        GPU(ogpu_next_view_create(s->images[i],&vd,&s->views[i]));
    }
    uint64_t usage=OGPU_NEXT_USAGE_STORAGE|OGPU_NEXT_USAGE_COPY_SRC|OGPU_NEXT_USAGE_COPY_DST;
    NEED(buffer_create(s,VERTICES,256,usage,OGPU_NEXT_MEMORY_LOCAL) && buffer_create(s,INDICES,160,usage|OGPU_NEXT_USAGE_INDEX,OGPU_NEXT_MEMORY_LOCAL)
        && buffer_create(s,DRAWS,SCENE_DRAW_BYTES,usage|OGPU_NEXT_USAGE_INDIRECT,OGPU_NEXT_MEMORY_LOCAL)
        && buffer_create(s,UPLOAD,SCENE_DRAW_BYTES>256 ? SCENE_DRAW_BYTES : 256,OGPU_NEXT_USAGE_COPY_SRC,OGPU_NEXT_MEMORY_HOST_VISIBLE)
        && buffer_create(s,READBACK,2*s->pixels+4*GUARD,OGPU_NEXT_USAGE_COPY_DST,OGPU_NEXT_MEMORY_HOST_VISIBLE)
        && buffer_create(s,GEOMETRY,416+SCENE_DRAW_BYTES,OGPU_NEXT_USAGE_COPY_DST,OGPU_NEXT_MEMORY_HOST_VISIBLE));
    GPU(ogpu_next_memory_address(span(s,VERTICES,GUARD,128),&s->root.vertices));
    GPU(ogpu_next_memory_address(span(s,INDICES,GUARD,32),&s->root.indices)); GPU(ogpu_next_memory_address(span(s,DRAWS,GUARD,SCENE_DRAW_BYTES-2*GUARD),&s->root.draws));
    memset(s->maps[UPLOAD].data,0xa5,s->sizes[UPLOAD]); GPU(ogpu_next_memory_flush(span(s,UPLOAD,0,s->sizes[UPLOAD])));
    memset(s->maps[READBACK].data,0xa5,s->sizes[READBACK]); GPU(ogpu_next_memory_flush(span(s,READBACK,0,s->sizes[READBACK])));
    NEED(begin(s));
    for(unsigned i=VERTICES;i<=DRAWS;++i) ogpu_next_copy_memory(s->encoder,span(s,i,0,s->sizes[i]),span(s,UPLOAD,0,s->sizes[i]));
    /* Untouched guards are read back too, not only bytes subsequently written
     * by compute. Make the initial poison visible to later device reads. */
    dependency(s,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_ALL,OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_READ);
    return submit_wait(s);
}
static void pass(Scene *s,unsigned pipeline,int color_load,int depth_load,float clear,unsigned first,unsigned count) {
    ogpu_next_attachment color={s->views[0],NULL,OGPU_NEXT_STATE_COLOR_ATTACHMENT,0,color_load ? OGPU_NEXT_LOAD : OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.f32={0,0,0,1}}};
    ogpu_next_attachment depth={s->views[1],NULL,OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT,0,depth_load ? OGPU_NEXT_LOAD : OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.depth_stencil={clear,0}}};
    ogpu_next_render_desc rd={HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),0,0,s->width,s->height,1,0,1,0,1,&color,&depth,NULL,{s->rs,s->render_scratch.size}};
    ogpu_next_render_begin(s->encoder,&rd); ogpu_next_bind_executable(s->encoder,s->raster[pipeline]);
    ogpu_next_viewport_state vp={HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),0,0,(float)s->width,(float)s->height,0,1,0,0,s->width,s->height};
    ogpu_next_set_graphics_state(s->encoder,&vp.header);
    ogpu_next_bind_indices(s->encoder,span(s,INDICES,GUARD,8*s->index_bytes),s->index_bytes==2 ? OGPU_NEXT_INDEX_U16 : OGPU_NEXT_INDEX_U32);
#ifdef SCENE_GENERATED
    const Scene_pair_vertexArguments args={.arg_vertices=s->root.vertices};
    ogpu_next_set_inline(s->encoder,OGPU_NEXT_STAGE_VERTEX|OGPU_NEXT_STAGE_FRAGMENT,0,sizeof(args),&args);
#else
    ogpu_next_set_inline(s->encoder,OGPU_NEXT_STAGE_VERTEX|OGPU_NEXT_STAGE_FRAGMENT,0,8,&s->root.vertices);
#endif
#ifdef SCENE_FOUNDATION_FRONTIER
    (void)first; (void)count;
    ogpu_next_indirect draw={span(s,DRAWS,GUARD,20*frontier_records),20,frontier_records,{NULL,0,0}};
    if(frontier_strategy==2) draw.count=span(s,DRAWS,GUARD+20*frontier_records,4);
    if(frontier_strategy) ogpu_next_draw_indexed_indirect(s->encoder,&draw);
    else for(unsigned i=0;i<frontier_records;++i) {
        draw.arguments=span(s,DRAWS,GUARD+20*i,20); draw.maximum_count=1;
        ogpu_next_draw_indexed_indirect(s->encoder,&draw);
    }
#else
    for(unsigned i=0;i<count;++i) {
        ogpu_next_indirect draw={span(s,DRAWS,GUARD+20*((first+i)%2),20),20,1,{NULL,0,0}};
        ogpu_next_draw_indexed_indirect(s->encoder,&draw);
    }
#endif
    ogpu_next_render_end(s->encoder);
}
#ifndef SCENE_REUSE_CONSUMER
static int write_output(const char *directory,unsigned mode,unsigned frame,const char *suffix,const void *bytes,size_t count) {
    char path[4096]; int n=snprintf(path,sizeof(path),"%s/mode-%u-frame-%u.%s",directory,mode,frame,suffix); NEED(n>=0 && n<(int)sizeof(path));
    FILE *f=fopen(path,"wb"); if(!f) { perror(path); return 0; }
    int okay=fwrite(bytes,1,count,f)==count && !ferror(f); if(fclose(f)) okay=0; return okay;
}
#endif
static void commands(Scene *s,unsigned mode) {
    dependency(s,OGPU_NEXT_STAGE_ALL,OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_ACCESS_READ|OGPU_NEXT_ACCESS_WRITE,OGPU_NEXT_ACCESS_SHADER_WRITE);
    ogpu_next_bind_executable(s->encoder,s->prepare);
#ifdef SCENE_REUSE_CONSUMER
#ifdef SCENE_GENERATED
    if(s->index_bytes==2) {
        const Scene_reuse_prepare16Arguments args={.arg_vertices=s->reuse_root[0],.arg_indices=s->reuse_root[1],
            .arg_draws=s->reuse_root[2],.arg_control=s->reuse_root[3]};
        ogpu_next_set_inline(s->encoder,OGPU_NEXT_STAGE_COMPUTE,0,sizeof(args),&args);
    } else {
        const Scene_reuse_prepareArguments args={.arg_vertices=s->reuse_root[0],.arg_indices=s->reuse_root[1],
            .arg_draws=s->reuse_root[2],.arg_control=s->reuse_root[3]};
        ogpu_next_set_inline(s->encoder,OGPU_NEXT_STAGE_COMPUTE,0,sizeof(args),&args);
    }
#else
    ogpu_next_set_inline(s->encoder,OGPU_NEXT_STAGE_COMPUTE,0,sizeof(s->reuse_root),s->reuse_root);
#endif
#elif defined(SCENE_GENERATED)
    if(s->index_bytes==2) {
        const Scene_prepare16Arguments args={.arg_vertices=s->root.vertices,.arg_indices=s->root.indices,
            .arg_draws=s->root.draws,.arg_phase=s->root.phase,.arg_empty=s->root.empty};
        ogpu_next_set_inline(s->encoder,OGPU_NEXT_STAGE_COMPUTE,0,sizeof(args),&args);
    } else {
        const Scene_prepareArguments args={.arg_vertices=s->root.vertices,.arg_indices=s->root.indices,
            .arg_draws=s->root.draws,.arg_phase=s->root.phase,.arg_empty=s->root.empty};
        ogpu_next_set_inline(s->encoder,OGPU_NEXT_STAGE_COMPUTE,0,sizeof(args),&args);
    }
#else
    ogpu_next_set_inline(s->encoder,OGPU_NEXT_STAGE_COMPUTE,0,32,&s->root);
#endif
    ogpu_next_launch launch={{1,1,1},0,NULL}; ogpu_next_dispatch(s->encoder,&launch);
    dependency(s,OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_STAGE_INDEX_INPUT|OGPU_NEXT_STAGE_VERTEX|OGPU_NEXT_STAGE_INDIRECT,
        OGPU_NEXT_ACCESS_SHADER_WRITE,OGPU_NEXT_ACCESS_INDEX_READ|OGPU_NEXT_ACCESS_SHADER_READ|OGPU_NEXT_ACCESS_INDIRECT_READ);
    transition(s,0,1); transition(s,1,1);
    if(mode>=6 && mode<=8) {
        pass(s,0,0,0,1,1,1);
        dependency(s,OGPU_NEXT_STAGE_COLOR|OGPU_NEXT_STAGE_DEPTH,OGPU_NEXT_STAGE_COLOR|OGPU_NEXT_STAGE_DEPTH,
            OGPU_NEXT_ACCESS_COLOR_WRITE|OGPU_NEXT_ACCESS_DEPTH_WRITE,
            OGPU_NEXT_ACCESS_COLOR_READ|OGPU_NEXT_ACCESS_COLOR_WRITE|OGPU_NEXT_ACCESS_DEPTH_READ|OGPU_NEXT_ACCESS_DEPTH_WRITE);
        pass(s,0,mode!=7,mode!=8,1,0,1);
    } else pass(s,mode>=2 && mode<=4 ? mode-1 : 0,0,0,mode==5 ? 0 : 1,mode>=1 && mode<=4,2);
    for(unsigned i=0;i<2;++i) {
        transition(s,i,0);
        ogpu_next_image_copy copy={{i ? OGPU_NEXT_ASPECT_DEPTH : OGPU_NEXT_ASPECT_COLOR,0,0,1,{0,0,0},{s->width,s->height,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};
        ogpu_next_copy_from_image(s->encoder,span(s,READBACK,i ? s->pixels+3*GUARD : GUARD,s->pixels),s->images[i],&copy);
    }
    dependency(s,OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_ACCESS_SHADER_WRITE,OGPU_NEXT_ACCESS_COPY_READ);
    uint64_t offset=0;
    for(unsigned i=VERTICES;i<=DRAWS;++i) { ogpu_next_copy_memory(s->encoder,span(s,GEOMETRY,offset,s->sizes[i]),span(s,i,0,s->sizes[i])); offset+=s->sizes[i]; }
    dependency(s,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_HOST,OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_HOST_READ);
}
#ifndef SCENE_REUSE_CONSUMER
static int frame(Scene *s,unsigned mode,unsigned frame,const char *directory) {
    NEED(begin(s)); s->root.phase=frame==1; s->root.empty=mode==9; commands(s,mode);
    NEED(submit_wait(s));
    GPU(ogpu_next_memory_invalidate(span(s,READBACK,0,s->sizes[READBACK]))); GPU(ogpu_next_memory_invalidate(span(s,GEOMETRY,0,584)));
    NEED(write_output(directory,mode,frame,"images",s->maps[READBACK].data,s->sizes[READBACK]) && write_output(directory,mode,frame,"geometry",s->maps[GEOMETRY].data,584));
    printf("SCENE_FRAME {\"mode\":%u,\"phase\":%u,\"frame\":%u}\n",mode,frame==1,frame); return 1;
}
#endif
static void destroy_resources(Scene *s) {
    if(s->pending) { fputs("Unknown foundation scene completion; refusing unsafe teardown\n",stderr); _Exit(EXIT_FAILURE); }
    if(s->encoder) ogpu_next_commands_cancel(s->encoder);
    ogpu_next_arena_destroy(s->arena); ogpu_next_timeline_destroy(s->done);
    for(unsigned i=0;i<2;++i) { ogpu_next_view_destroy(s->views[i]); ogpu_next_image_destroy(s->images[i]); ogpu_next_memory_destroy(s->backing[i]); }
    for(unsigned i=0;i<BUFFER_COUNT;++i) if(s->buffers[i]) ogpu_next_memory_destroy(s->buffers[i]);
    free(s->bs); free(s->rs); free(s->ss);
}
static void destroy_context(Scene *s) {
    for(unsigned i=0;i<4;++i) ogpu_next_executable_destroy(s->raster[i]);
    ogpu_next_executable_destroy(s->prepare);
    ogpu_next_device_destroy(s->device); ogpu_next_discovery_destroy(s->discovery);
    for(unsigned i=0;i<3;++i) free(s->code[i]);
}
#ifndef SCENE_REUSE_CONSUMER
int main(int argc,char **argv) {
    if(argc!=5 && argc!=6) return 1;
    Scene s={.index_bytes=4}; int okay=0;
    if(argc==6) { if(!strcmp(argv[5],"16")) s.index_bytes=2; else if(strcmp(argv[5],"32")) return 1; }
    if(!strcmp(argv[1],"257") && !strcmp(argv[2],"193")) { s.width=257; s.height=193; }
    else if(!strcmp(argv[1],"640") && !strcmp(argv[2],"360")) { s.width=640; s.height=360; }
    else if(!strcmp(argv[1],"1280") && !strcmp(argv[2],"720")) { s.width=1280; s.height=720; }
    else return 1;
    s.pixels=(size_t)s.width*s.height*4;
    if(!context(&s,argv[3]) || !storage(&s) || !resources(&s)) goto done;
    for(unsigned mode=0;mode<10;++mode) for(unsigned f=0;f<3;++f) if(!frame(&s,mode,f,argv[4])) goto done;
    okay=1;
done:
    destroy_resources(&s); destroy_context(&s);
    if(okay) puts("Public indexed/depth frames drained; CPU oracle must independently accept outputs");
    return !okay;
}
#endif
