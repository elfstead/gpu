/* Standalone ABI-19 consumer: generated shaders, GPU counts and local identity.
 * No Vulkan headers, compiler or repository path is needed to build/run it. */
#include <identity.generated.h>
#include <range_count.generated.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CHECK(x) do { if (!(x)) { fprintf(stderr,"range check line %d: %s\n",__LINE__,#x); goto cleanup; } } while (0)
#define GPU(x) do { if ((x)!=OGPU_SUCCESS) { fprintf(stderr,"%s: %s\n",#x,a->error.message); goto cleanup; } } while (0)
enum { WIDTH=17, HEIGHT=13, PIXELS=WIDTH*HEIGHT*4, RECORD_BYTES=256 };
typedef struct App {
    OgpuError error;
    OgpuDevice *device;
    OgpuRaster *raster;
    OgpuKernel *kernel;
    OgpuImage *image;
    OgpuBuffer *vertices, *indices, *records, *count, *active, *output;
    OgpuRecordingStorage *storage;
    OgpuCommandList *list;
    OgpuBatch *batch;
    OgpuCompletion *completion;
    Identity_vertexArguments draw_root;
    Range_countArguments compute_root;
    unsigned format, shared, replay, mode;
} App;

static int buffer(App *a, OgpuBuffer **out, uint64_t size, uint32_t usage) {
    GPU(ogpu_buffer_create(a->device,&(OgpuBufferDesc){size,OGPU_MEMORY_HOST,usage},out,&a->error));
    return 1;
cleanup: return 0;
}
static int draw(App *a, const OgpuIndirectRange *range) {
    if (!a->format) {
        GPU(ogpu_batch_draw_indirect(a->batch,a->raster,range,&a->draw_root,sizeof(a->draw_root),&a->error));
    } else {
        const OgpuIndexRange index={.buffer=a->indices,.offset=8,
            .size_bytes=a->format==1 ? 6 : 12,.format=a->format==1 ? OGPU_INDEX_UINT16 : OGPU_INDEX_UINT32};
        GPU(ogpu_batch_draw_indexed_indirect(a->batch,a->raster,&index,range,&a->draw_root,sizeof(a->draw_root),&a->error));
    }
    return 1;
cleanup: return 0;
}
static int record(App *a) {
    GPU(ogpu_batch_create_in(a->storage,&a->batch,&a->error));
    GPU(ogpu_batch_retain_buffer(a->batch,a->vertices,&a->error));
    GPU(ogpu_batch_retain_buffer(a->batch,a->active,&a->error));
    GPU(ogpu_batch_retain_buffer(a->batch,a->shared ? a->records : a->count,&a->error));
    GPU(ogpu_batch_dispatch(a->batch,a->kernel,1,1,1,&a->compute_root,sizeof(a->compute_root),&a->error));
    GPU(ogpu_batch_barrier(a->batch,OGPU_ACCESS_COMPUTE_WRITE,OGPU_ACCESS_INDIRECT_READ,&a->error));
    GPU(ogpu_batch_discard_image(a->batch,a->image,&a->error));
    const OgpuRenderingDesc rendering={.color={a->image,OGPU_ATTACHMENT_CLEAR,OGPU_STORE_STORE,{0,0,0,1}}};
    GPU(ogpu_batch_begin_rendering(a->batch,&rendering,&a->error));
    OgpuIndirectRange range={.buffer=a->records,.offset=16,.stride_bytes=32,.max_draw_count=5,
        .count_buffer=a->shared ? a->records : a->count,.count_offset=a->shared ? 240 : 4};
    if(a->mode==0 || a->mode==4) { range.count_buffer=NULL;range.count_offset=0; } /* Fixed range. */
    if(a->mode==3) { range.offset=RECORD_BYTES;range.max_draw_count=0; }
    CHECK(draw(a,&range));
    if(a->mode==2 || a->mode==4) {
        /* Record 3 is nonempty, but is ordinal zero in this separate range. */
        range.offset=16+3*32;range.max_draw_count=2;range.count_buffer=NULL;range.count_offset=0;
        CHECK(draw(a,&range));
    }
    GPU(ogpu_batch_end_rendering(a->batch,&a->error));
    GPU(ogpu_batch_copy_image_to_buffer(a->batch,a->image,a->output,64,&a->error));
    return 1;
cleanup: return 0;
}
static int run_case(App *a) {
    int okay=0;
    unsigned char records[RECORD_BYTES], found[RECORD_BYTES], pixels[PIXELS+128];
    memset(records,0xa5,sizeof(records));
    for(unsigned i=0;i<5;++i) {
        const uint32_t words[]={i==1 || i==3 ? 3u : 0u,1,0,0,0};
        memcpy(records+16+32*i,words,a->format ? 20 : 16);
    }
    CHECK(buffer(a,&a->vertices,48,0) && buffer(a,&a->indices,32,OGPU_BUFFER_INDEX));
    CHECK(buffer(a,&a->records,sizeof(records),0) && buffer(a,&a->count,16,0));
    CHECK(buffer(a,&a->active,16,0) && buffer(a,&a->output,sizeof(pixels),0));
    const float vertices[]={-1,-1,0,1,3,-1,0,1,-1,3,0,1};
    GPU(ogpu_buffer_write(a->vertices,0,vertices,sizeof(vertices),&a->error));
    unsigned char indices[32];memset(indices,0xa5,sizeof(indices));
    const uint16_t u16[]={0,1,2};const uint32_t u32[]={0,1,2};
    memcpy(indices+8,a->format==1 ? (const void *)u16 : (const void *)u32,a->format==1 ? sizeof(u16) : sizeof(u32));
    GPU(ogpu_buffer_write(a->indices,0,indices,sizeof(indices),&a->error));
    GPU(ogpu_buffer_write(a->records,0,records,sizeof(records),&a->error));
    unsigned char counter[16];memset(counter,0xa5,sizeof(counter));
    GPU(ogpu_buffer_write(a->count,0,counter,sizeof(counter),&a->error));
    GPU(ogpu_buffer_device_address(a->vertices,&a->draw_root.arg_vertices,&a->error));
    GPU(ogpu_buffer_device_address(a->active,&a->compute_root.arg_active,&a->error));
    GPU(ogpu_buffer_device_address(a->shared ? a->records : a->count,&a->compute_root.arg_count,&a->error));
    a->compute_root.arg_active+=4;a->compute_root.arg_count+=a->shared ? 240 : 4;
    GPU(ogpu_image_create(a->device,&(OgpuImageDesc){OGPU_IMAGE_2D,WIDTH,HEIGHT,OGPU_FORMAT_RGBA8_UNORM,
        OGPU_IMAGE_USAGE_COLOR|OGPU_IMAGE_USAGE_COPY_SRC,0},&a->image,&a->error));
    GPU(ogpu_recording_storage_create(a->device,&a->storage,&a->error));
    if(a->replay) {
        CHECK(record(a));GPU(ogpu_batch_compile(a->batch,0,&a->list,&a->error));
        ogpu_batch_destroy(a->batch);a->batch=NULL;
    }
    const uint32_t counts[]={0,1,2,3,4,5,99,0,2};
    for(unsigned f=0;f<sizeof(counts)/sizeof(counts[0]);++f) {
        unsigned char control[16], read_control[16];memset(control,0xa5,sizeof(control));
        memcpy(control+4,&counts[f],4);
        GPU(ogpu_buffer_write(a->active,0,control,sizeof(control),&a->error));
        memset(pixels,0xa5,sizeof(pixels));GPU(ogpu_buffer_write(a->output,0,pixels,sizeof(pixels),&a->error));
        if(a->replay) { GPU(ogpu_command_list_submit(a->list,&a->completion,&a->error)); }
        else {
            CHECK(record(a));GPU(ogpu_batch_submit(a->batch,&a->completion,&a->error));
            ogpu_batch_destroy(a->batch);a->batch=NULL;
        }
        GPU(ogpu_completion_wait(a->completion,&a->error));
        GPU(ogpu_buffer_read(a->output,0,pixels,sizeof(pixels),&a->error));
        unsigned identity=a->mode==0 ? 8 : a->mode==2 || a->mode==4 ? 2 : a->mode==3 ? 0 : counts[f]>=4 ? 8 : counts[f]>=2 ? 4 : 0;
        const unsigned char expected[]={identity,0,identity ? 255 : 0,255};
        for(size_t p=0;p<PIXELS;p+=4) if(memcmp(pixels+64+p,expected,4)) {
            fprintf(stderr,"range pixel format=%u shared=%u replay=%u mode=%u frame=%u count=%u pixel=%zu: "
                "%u,%u,%u,%u expected %u,%u,%u,%u\n",a->format,a->shared,a->replay,a->mode,f,counts[f],p/4,
                pixels[64+p],pixels[65+p],pixels[66+p],pixels[67+p],expected[0],expected[1],expected[2],expected[3]);
            goto cleanup;
        }
        for(unsigned p=0;p<64;++p) CHECK(pixels[p]==0xa5 && pixels[64+PIXELS+p]==0xa5);
        GPU(ogpu_buffer_read(a->records,0,found,sizeof(found),&a->error));
        if(a->shared) memcpy(records+240,&counts[f],4);
        CHECK(!memcmp(records,found,sizeof(records)));
        GPU(ogpu_buffer_read(a->count,0,found,sizeof(counter),&a->error));
        if(!a->shared) memcpy(counter+4,&counts[f],4);
        CHECK(!memcmp(counter,found,sizeof(counter)));
        GPU(ogpu_buffer_read(a->indices,0,found,sizeof(indices),&a->error));
        CHECK(!memcmp(indices,found,sizeof(indices)));
        GPU(ogpu_buffer_read(a->active,0,read_control,sizeof(control),&a->error));
        CHECK(!memcmp(control,read_control,sizeof(control)));
        ogpu_completion_destroy(a->completion);a->completion=NULL;
    }
    printf("Range identity format=%u shared=%u replay=%u mode=%u: 9 full-image/count/guard frames PASS\n",
        a->format,a->shared,a->replay,a->mode);
    okay=1;
cleanup:
    /* Drain before destroying root-only backing, including on failed checks. */
    ogpu_completion_destroy(a->completion);a->completion=NULL;
    ogpu_batch_destroy(a->batch);a->batch=NULL;
    ogpu_command_list_destroy(a->list);a->list=NULL;
    ogpu_recording_storage_destroy(a->storage);a->storage=NULL;
    ogpu_image_destroy(a->image);a->image=NULL;
    ogpu_buffer_destroy(a->output);a->output=NULL;
    ogpu_buffer_destroy(a->active);a->active=NULL;
    ogpu_buffer_destroy(a->count);a->count=NULL;
    ogpu_buffer_destroy(a->records);a->records=NULL;
    ogpu_buffer_destroy(a->indices);a->indices=NULL;
    ogpu_buffer_destroy(a->vertices);a->vertices=NULL;
    return okay;
}
int main(void) {
    App app={0};App *a=&app;OgpuProbe *probe=NULL;int result=EXIT_FAILURE;unsigned tested=0,failed=0;
    GPU(ogpu_probe_create(OGPU_ABI_VERSION,&probe,&a->error));
    uint32_t devices;GPU(ogpu_probe_device_count(probe,&devices));
    for(uint32_t i=0;i<devices;++i) {
        OgpuResult status=ogpu_device_create_graphics(probe,i,&a->device,&a->error);
        if(status==OGPU_ERROR_UNSUPPORTED) continue;
        GPU(status);
        OgpuCapabilities caps;OgpuDeviceLimits limits;
        GPU(ogpu_device_capabilities(a->device,&caps,&a->error));GPU(ogpu_device_limits(a->device,&limits,&a->error));
        CHECK(identity_vertex_compatible(&caps,&limits) && identity_fragment_compatible(&caps,&limits));
        CHECK(range_count_compatible(&caps,&limits) && limits.max_indirect_draw_count>=5);
        CHECK(caps.multi_draw_indirect && caps.draw_indirect_count);
        OgpuCapabilities disabled=caps;disabled.shader_draw_parameters=0;
        CHECK(!identity_vertex_compatible(&disabled,&limits));
        OgpuShaderDesc vertex=identity_vertex_shader(),fragment=identity_fragment_shader(),compute=range_count_shader();
        GPU(ogpu_raster_create(a->device,&vertex,&fragment,&(OgpuRasterDesc){identity_push_size,OGPU_TOPOLOGY_TRIANGLE_LIST,
            OGPU_FORMAT_RGBA8_UNORM,OGPU_FORMAT_NONE,0,0,OGPU_COMPARE_ALWAYS,0},&a->raster,&a->error));
        GPU(ogpu_kernel_create(a->device,&compute,range_count_push_size,&a->kernel,&a->error));
        for(a->format=0;a->format<3;++a->format)
        for(a->shared=0;a->shared<2;++a->shared)
        for(a->replay=0;a->replay<2;++a->replay)
        for(a->mode=0;a->mode<5;++a->mode) if(!run_case(a)) ++failed;
        ogpu_kernel_destroy(a->kernel);a->kernel=NULL;ogpu_raster_destroy(a->raster);a->raster=NULL;
        ogpu_device_destroy(a->device);a->device=NULL;++tested;
    }
    if(failed) fprintf(stderr,"Generated indirect range consumer FAILED: %u configurations\n",failed);
    CHECK(tested && !failed);printf("Generated indirect range consumer PASS: 540 frames per device, ABI=%u\n",OGPU_ABI_VERSION);
    result=EXIT_SUCCESS;
cleanup:
    ogpu_kernel_destroy(a->kernel);ogpu_raster_destroy(a->raster);ogpu_device_destroy(a->device);ogpu_probe_destroy(probe);
    return result;
}
