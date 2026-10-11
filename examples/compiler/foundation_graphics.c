/* Same public-only caller for all compiler-linked stage/argument variants. */
#include <foundation-pattern.generated.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define HEADER(type,kind) {kind,OGPU_NEXT_VERSION,sizeof(type),0,NULL}
#define REQUIRE(c) do { if (!(c)) { fprintf(stderr,"line %d: %s\n",__LINE__,#c); goto cleanup; } } while (0)
#define TRY(call) do { ogpu_next_status s_ = (call); if (s_ != OGPU_NEXT_OK) { fprintf(stderr,"%s: %" PRId32 "\n",#call,s_); goto cleanup; } } while (0)
static ogpu_next_query query(uint32_t kind, void *data, uint32_t capacity, uint32_t size) {
    return (ogpu_next_query){HEADER(ogpu_next_query,kind),data,capacity,0,size,0};
}
static int run_case(ogpu_next_device *device, ogpu_next_executable *pipeline, uint32_t domain, uint32_t width, uint32_t height) {
    int result = EXIT_FAILURE, pending = 0;
    ogpu_next_image *image = NULL; ogpu_next_view *view = NULL;
    ogpu_next_memory *backing = NULL, *host = NULL;
    ogpu_next_arena *arena = NULL; ogpu_next_timeline *done = NULL;
    void *bs = NULL, *rs = NULL, *ss = NULL;
    const uint64_t bytes = (uint64_t)width*height*4;
    ogpu_next_image_desc id = {HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),OGPU_NEXT_RGBA8_UNORM,OGPU_NEXT_IMAGE_2D,
        1,1,1,{width,height,1},OGPU_NEXT_IMAGE_COLOR_ATTACHMENT | OGPU_NEXT_IMAGE_COPY_SRC,0,NULL,0,0,NULL};
    uint32_t compatible[32]; ogpu_next_requirements req = {0}; req.compatible_types = compatible; req.compatible_type_capacity = 32;
    TRY(ogpu_next_image_requirements(device,&id,&req)); REQUIRE(req.compatible_type_count > 0);
    TRY(ogpu_next_image_create_unbound(device,&id,&image));
    TRY(ogpu_next_memory_create_dedicated_image(image,compatible[0],&backing));
    TRY(ogpu_next_image_bind(image,(ogpu_next_span){backing,0,req.size}));
    ogpu_next_subresources range = {OGPU_NEXT_ASPECT_COLOR,0,1,0,1};
    ogpu_next_view_desc vd = {HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),OGPU_NEXT_RGBA8_UNORM,OGPU_NEXT_VIEW_2D,
        OGPU_NEXT_IMAGE_COLOR_ATTACHMENT,{0,0,0,0},range};
    TRY(ogpu_next_view_create(image,&vd,&view));
    ogpu_next_memory_desc md = {HEADER(ogpu_next_memory_desc,OGPU_NEXT_MEMORY_DESC),bytes+128,8,
        OGPU_NEXT_USAGE_COPY_DST | OGPU_NEXT_USAGE_INDIRECT,UINT32_MAX,OGPU_NEXT_MEMORY_LINEAR,NULL,0,0};
    TRY(ogpu_next_memory_requirements(device,&md,&req));
    ogpu_next_memory_type_info types[32]; ogpu_next_query q = query(OGPU_NEXT_QUERY_MEMORY_TYPES,types,32,sizeof(types[0]));
    TRY(ogpu_next_device_query(device,&q));
    for (uint32_t i = 0; i < req.compatible_type_count && md.memory_type == UINT32_MAX; ++i)
        for (uint32_t j = 0; j < q.count; ++j) if (types[j].id == compatible[i] && (types[j].properties & OGPU_NEXT_MEMORY_HOST_VISIBLE)) {
            md.memory_type = types[j].id; break;
        }
    REQUIRE(md.memory_type != UINT32_MAX); TRY(ogpu_next_memory_create(device,&md,&host));
    ogpu_next_span all = {host,0,md.size}; ogpu_next_mapping map = {0}; TRY(ogpu_next_memory_map(all,&map));
    ogpu_next_host_requirements br = {0}, rr = {0}, sr = {0};
    TRY(ogpu_next_barrier_scratch_requirements(0,1,&br)); TRY(ogpu_next_render_scratch_requirements(1,&rr));
    TRY(ogpu_next_submit_scratch_requirements(1,0,1,&sr));
    REQUIRE(br.alignment <= _Alignof(max_align_t) && rr.alignment <= _Alignof(max_align_t) && sr.alignment <= _Alignof(max_align_t));
    bs = malloc((size_t)br.size); rs = malloc((size_t)rr.size); ss = malloc((size_t)sr.size); REQUIRE(bs && rs && ss);
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena)); TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    ogpu_next_encoder *encoder = NULL; ogpu_next_list *list = NULL; TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    ogpu_next_image_barrier barrier = {image,range,OGPU_NEXT_ACCESS_COPY_READ,OGPU_NEXT_ACCESS_COLOR_WRITE,
        OGPU_NEXT_STATE_UNDEFINED,OGPU_NEXT_STATE_COLOR_ATTACHMENT,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1};
    ogpu_next_dependency dep = {HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_COLOR,
        0,0,0,1,0,NULL,&barrier,bs,br.size}; ogpu_next_barrier(encoder,&dep);
    ogpu_next_attachment attachment = {view,NULL,OGPU_NEXT_STATE_COLOR_ATTACHMENT,0,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.f32={0,0,0,0}}};
    ogpu_next_render_desc render = {HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),0,0,width,height,1,0,1,0,1,&attachment,NULL,NULL,{rs,rr.size}};
    ogpu_next_viewport_state vp = {HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),0,0,(float)width,(float)height,0,1,0,0,width,height};
    ogpu_next_render_begin(encoder,&render); ogpu_next_bind_executable(encoder,pipeline); ogpu_next_set_graphics_state(encoder,&vp.header);
    Pattern_fragmentArguments args = {.arg_width=width,.arg_height=height};
    ogpu_next_set_inline(encoder,OGPU_NEXT_STAGE_VERTEX | OGPU_NEXT_STAGE_FRAGMENT,0,sizeof(args),&args);
    memset(&args,0xcc,sizeof(args));
    ogpu_next_indirect draw = {{host,0,16},16,1,{NULL,0,0}};
    ogpu_next_draw_indirect(encoder,&draw); ogpu_next_render_end(encoder);
    barrier.before = OGPU_NEXT_ACCESS_COLOR_WRITE; barrier.after = OGPU_NEXT_ACCESS_COPY_READ;
    barrier.old_state = OGPU_NEXT_STATE_COLOR_ATTACHMENT; barrier.new_state = OGPU_NEXT_STATE_COPY_SRC; barrier.discard = 0;
    dep.before = OGPU_NEXT_STAGE_COLOR; dep.after = OGPU_NEXT_STAGE_COPY; ogpu_next_barrier(encoder,&dep);
    ogpu_next_image_copy copy = {{OGPU_NEXT_ASPECT_COLOR,0,0,1,{0,0,0},{width,height,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};
    ogpu_next_copy_from_image(encoder,(ogpu_next_span){host,64,bytes},image,&copy);
    dep.before = OGPU_NEXT_STAGE_COPY; dep.after = OGPU_NEXT_STAGE_HOST;
    dep.global_before = OGPU_NEXT_ACCESS_COPY_WRITE; dep.global_after = OGPU_NEXT_ACCESS_HOST_READ; dep.image_count = 0; dep.images = NULL;
    ogpu_next_barrier(encoder,&dep); TRY(ogpu_next_commands_end(encoder,&list));
    ogpu_next_sync_point signal = {{done,1},OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {HEADER(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),1,0,1,&list,NULL,&signal,ss,sr.size};
    for (uint32_t replay = 0; replay < 3; ++replay) {
        unsigned char *pixels = map.data; memset(pixels,0xa5,(size_t)md.size);
        const uint32_t arguments[4] = {replay == 1 ? 0 : 3,1,0,0}; memcpy(pixels,arguments,sizeof(arguments));
        TRY(ogpu_next_memory_flush(all)); signal.point.value = replay+1; pending = 1;
        TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),&submit));
        TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0; TRY(ogpu_next_memory_invalidate(all));
        REQUIRE(memcmp(pixels,arguments,sizeof(arguments)) == 0);
        for (uint64_t i = 16; i < md.size; ++i) if (i < 64 || i >= 64+bytes) REQUIRE(pixels[i] == 0xa5);
        for (uint32_t y = 0; y < height; ++y) for (uint32_t x = 0; x < width; ++x) {
            unsigned char expected[] = {(unsigned char)(x*17+y*3),(unsigned char)(x*5+y*29),(unsigned char)(x*11+y*7),255};
            if (replay == 1) memset(expected,0,sizeof(expected));
            REQUIRE(memcmp(pixels+64+((uint64_t)y*width+x)*4,expected,4) == 0);
        }
    }
    printf("Foundation graphics %ux%u: three indirect replays including empty draw, exact pixels/guards PASS\n",width,height);
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending foundation graphics after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_timeline_destroy(done); ogpu_next_view_destroy(view); ogpu_next_image_destroy(image);
    ogpu_next_memory_destroy(backing); ogpu_next_memory_destroy(host); free(bs); free(rs); free(ss); return result;
}
int main(void) {
    int result = EXIT_FAILURE;
    ogpu_next_discovery *discovery = NULL; ogpu_next_device *device = NULL; ogpu_next_executable *pipeline = NULL;
    uint32_t domain = UINT32_MAX;
    TRY(ogpu_next_discovery_create(OGPU_NEXT_VERSION,&discovery));
    for (uint32_t index = 0; index < ogpu_next_discovery_count(discovery); ++index) {
        const ogpu_next_adapter *adapter = ogpu_next_discovery_adapter(discovery,index);
        ogpu_next_feature_info f = {0}; ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&f,1,sizeof(f)); TRY(ogpu_next_adapter_query(adapter,&q));
        if (!f.baseline_supported || (f.available & pattern_required_features) != pattern_required_features) continue;
        ogpu_next_queue_info queues[32]; q = query(OGPU_NEXT_QUERY_QUEUES,queues,32,sizeof(queues[0])); TRY(ogpu_next_adapter_query(adapter,&q));
        for (uint32_t i = 0; i < q.count; ++i) if (queues[i].count && (queues[i].flags & OGPU_NEXT_QUEUE_GRAPHICS)) { domain = queues[i].domain; break; }
        if (domain == UINT32_MAX) continue;
        ogpu_next_queue_request request = {domain,1,0.5f};
        ogpu_next_device_desc dd = {HEADER(ogpu_next_device_desc,OGPU_NEXT_DEVICE_DESC),&request,1,0,pattern_required_features};
        TRY(ogpu_next_device_create(adapter,&dd,&device)); break;
    }
    REQUIRE(device != NULL); ogpu_next_discovery_destroy(discovery); discovery = NULL;
    ogpu_next_feature_info f = {0}; ogpu_next_execution_limits limits = {0};
    ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&f,1,sizeof(f)); TRY(ogpu_next_device_query(device,&q));
    q = query(OGPU_NEXT_QUERY_EXECUTION_LIMITS,&limits,1,sizeof(limits)); TRY(ogpu_next_device_query(device,&q));
    REQUIRE(pattern_compatible(&f,&limits));
    ogpu_next_feature_info missing = f; missing.enabled &= ~OGPU_NEXT_FEATURE_RASTER; REQUIRE(!pattern_compatible(&missing,&limits));
    ogpu_next_execution_limits small = limits; small.max_inline_size = pattern_push_size-1; REQUIRE(!pattern_compatible(&f,&small));
    ogpu_next_shader shaders[2]; pattern_shaders(shaders);
    REQUIRE(shaders[0].interface_metadata == &pattern_interface.header && shaders[1].interface_metadata == &pattern_interface.header);
    ogpu_next_color_state color = {OGPU_NEXT_RGBA8_UNORM,15,0,1,0,0,1,0,0};
    ogpu_next_graphics_state gs = {HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),
        OGPU_NEXT_TRIANGLES,OGPU_NEXT_CULL_NONE,OGPU_NEXT_FRONT_CCW,1,1,&color,0,0,0,0,{0},0,{0},{0},NULL,NULL,0,0};
    ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_GRAPHICS,2,shaders,
        &gs.header,OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR,&pattern_requirements.header,NULL,0,0};
    TRY(ogpu_next_executable_create(device,&ed,&pipeline));
    const uint32_t sizes[][2] = {{1,1},{2,3},{63,65},{64,64},{65,63},{97,65}};
    for (uint32_t i = 0; i < sizeof(sizes)/sizeof(sizes[0]); ++i)
        REQUIRE(run_case(device,pipeline,domain,sizes[i][0],sizes[i][1]) == EXIT_SUCCESS);
    result = EXIT_SUCCESS;
cleanup:
    ogpu_next_executable_destroy(pipeline); ogpu_next_device_destroy(device); ogpu_next_discovery_destroy(discovery); return result;
}
