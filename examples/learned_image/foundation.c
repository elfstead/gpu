/* Foundation port of the frozen learned-image acceptance workload, not its
 * timing harness. Allocation, synchronization and replay belong to this caller. */
#include "ogpu_next.h"
#include "extent.h"
#include <application.generated.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define HEADER(type,kind) {kind,OGPU_NEXT_VERSION,sizeof(type),0,NULL}
#define REQUIRE(c) do { if (!(c)) { fprintf(stderr,"line %d: %s\n",__LINE__,#c); goto cleanup; } } while (0)
#define TRY(call) do { ogpu_next_status s_ = (call); if (s_ != OGPU_NEXT_OK) { fprintf(stderr,"%s: %" PRId32 "\n",#call,s_); goto cleanup; } } while (0)
enum { INPUT, WEIGHTS, HIDDEN, DENOISED, COLOR, BUFFER_COUNT, GUARD=64 };
static ogpu_next_query query(uint32_t kind, void *data, uint32_t capacity, uint32_t size) {
    return (ogpu_next_query){HEADER(ogpu_next_query,kind),data,capacity,0,size,0};
}
static int path_join(char path[4096], const char *directory, const char *file) {
    int n = snprintf(path,4096,"%s/%s",directory,file); return n >= 0 && n < 4096;
}
static int file_bytes(const char *path, void *data, size_t size, int write) {
    FILE *file = fopen(path,write ? "wb" : "rb"); if (!file) { perror(path); return 0; }
    int okay = write ? fwrite(data,1,size,file) == size : fread(data,1,size,file) == size && fgetc(file) == EOF && !ferror(file);
    if (fclose(file)) okay = 0;
    return okay;
}
static void poison_host(void *data, size_t bytes) {
    const uint32_t word = UINT32_C(0x7fc000a5);
    for (size_t i = 0; i < bytes; i += 4) memcpy((char *)data+i,&word,4);
}
static int guards(const unsigned char *data, size_t bytes) {
    for (uint32_t i = 0; i < GUARD; i += 4) {
        uint32_t a,b; memcpy(&a,data+i,4); memcpy(&b,data+bytes-GUARD+i,4);
        if (a != UINT32_C(0x7fc000a5) || b != UINT32_C(0x7fc000a5)) return 0;
    }
    return 1;
}
static int memory_create(ogpu_next_device *device, uint64_t size, uint64_t usage, uint32_t properties, ogpu_next_memory **out) {
    int result = EXIT_FAILURE;
    ogpu_next_memory_desc md = {HEADER(ogpu_next_memory_desc,OGPU_NEXT_MEMORY_DESC),size,8,usage,UINT32_MAX,OGPU_NEXT_MEMORY_LINEAR,NULL,0,0};
    uint32_t compatible[32]; ogpu_next_requirements req = {0}; req.compatible_types = compatible; req.compatible_type_capacity = 32;
    TRY(ogpu_next_memory_requirements(device,&md,&req));
    ogpu_next_memory_type_info types[32]; ogpu_next_query q = query(OGPU_NEXT_QUERY_MEMORY_TYPES,types,32,sizeof(types[0]));
    TRY(ogpu_next_device_query(device,&q));
    for (uint32_t i = 0; i < req.compatible_type_count && md.memory_type == UINT32_MAX; ++i)
        for (uint32_t j = 0; j < q.count; ++j) if (types[j].id == compatible[i] && (types[j].properties & properties) == properties) {
            md.memory_type = types[j].id; break;
        }
    REQUIRE(md.memory_type != UINT32_MAX); TRY(ogpu_next_memory_create(device,&md,out)); result = EXIT_SUCCESS;
cleanup:
    return result;
}
static void dependency(ogpu_next_encoder *encoder, uint64_t before, uint64_t after, uint64_t before_access, uint64_t after_access) {
    ogpu_next_dependency dep = {HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),before,after,before_access,after_access,0,0,0,NULL,NULL,NULL,0};
    ogpu_next_barrier(encoder,&dep);
}
static void dispatch(ogpu_next_encoder *encoder, ogpu_next_executable *pipeline, Launch grid, const void *args, uint32_t bytes) {
    ogpu_next_bind_executable(encoder,pipeline); ogpu_next_set_inline(encoder,OGPU_NEXT_STAGE_COMPUTE,0,bytes,args);
    ogpu_next_launch launch = {{grid.x,grid.y,1},0,NULL}; ogpu_next_dispatch(encoder,&launch);
}
int main(int argc, char **argv) {
    int result = EXIT_FAILURE, pending = 0;
    ogpu_next_discovery *discovery = NULL; ogpu_next_device *device = NULL;
    ogpu_next_memory *buffers[BUFFER_COUNT] = {NULL}, *upload = NULL, *readback = NULL, *draw = NULL, *backing = NULL;
    ogpu_next_executable *kernels[4] = {NULL}, *raster = NULL;
    ogpu_next_image *target = NULL; ogpu_next_view *view = NULL;
    ogpu_next_arena *arena = NULL; ogpu_next_timeline *done = NULL;
    void *bs = NULL, *rs = NULL, *ss = NULL; unsigned char *weights = NULL;
    char path[4096], name[96];
    REQUIRE(argc >= 8); const uint16_t endian = 1; REQUIRE(*(const unsigned char *)&endian == 1);
    uint32_t width = extent(argv[2]), height = extent(argv[3]), ow = extent(argv[4]), oh = extent(argv[5]), count, output_count;
    REQUIRE(image_count(width,height,8,&count) && image_count(ow,oh,4,&output_count));
    REQUIRE(resize_axis(width,ow) && resize_axis(height,oh));
    size_t payloads[BUFFER_COUNT] = {(size_t)count*4,89u*4u,(size_t)count*32,(size_t)count*4,(size_t)output_count*16};
    size_t final_bytes = (size_t)output_count*4, sizes[BUFFER_COUNT], offsets[BUFFER_COUNT], total = final_bytes+2*GUARD;
    uint64_t addresses[BUFFER_COUNT] = {0};
    for (uint32_t i = 0; i < BUFFER_COUNT; ++i) { sizes[i] = payloads[i]+2*GUARD; offsets[i] = total; REQUIRE(add_size(total,sizes[i],&total)); }
    size_t upload_size = sizes[INPUT] > sizes[WEIGHTS] ? sizes[INPUT] : sizes[WEIGHTS];
    uint64_t required = hidden_required_features | denoise_required_features | process_required_features | poison_required_features | application_required_features;
    uint32_t domain = UINT32_MAX;
    TRY(ogpu_next_discovery_create(OGPU_NEXT_VERSION,&discovery));
    for (uint32_t index = 0; index < ogpu_next_discovery_count(discovery); ++index) {
        const ogpu_next_adapter *adapter = ogpu_next_discovery_adapter(discovery,index);
        ogpu_next_feature_info f = {0}; ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&f,1,sizeof(f)); TRY(ogpu_next_adapter_query(adapter,&q));
        if (!f.baseline_supported || (f.available & required) != required) continue;
        ogpu_next_queue_info queues[32]; q = query(OGPU_NEXT_QUERY_QUEUES,queues,32,sizeof(queues[0])); TRY(ogpu_next_adapter_query(adapter,&q));
        for (uint32_t i = 0; i < q.count; ++i) if (queues[i].count && (queues[i].flags & (OGPU_NEXT_QUEUE_GRAPHICS | OGPU_NEXT_QUEUE_COMPUTE)) == (OGPU_NEXT_QUEUE_GRAPHICS | OGPU_NEXT_QUEUE_COMPUTE)) { domain = queues[i].domain; break; }
        if (domain == UINT32_MAX) continue;
        ogpu_next_queue_request request = {domain,1,0.5f};
        ogpu_next_device_desc dd = {HEADER(ogpu_next_device_desc,OGPU_NEXT_DEVICE_DESC),&request,1,0,required};
        TRY(ogpu_next_device_create(adapter,&dd,&device)); break;
    }
    REQUIRE(device != NULL); ogpu_next_discovery_destroy(discovery); discovery = NULL;
    ogpu_next_feature_info f = {0}; ogpu_next_execution_limits limits = {0};
    ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&f,1,sizeof(f)); TRY(ogpu_next_device_query(device,&q));
    q = query(OGPU_NEXT_QUERY_EXECUTION_LIMITS,&limits,1,sizeof(limits)); TRY(ogpu_next_device_query(device,&q));
    REQUIRE(hidden_compatible(&f,&limits) && denoise_compatible(&f,&limits) && process_compatible(&f,&limits)
        && poison_compatible(&f,&limits) && application_compatible(&f,&limits));
    Launch hidden_grid, denoise_grid, process_grid, poison_grid[BUFFER_COUNT] = {{0}};
    REQUIRE(launch(count*8u,hidden_local,limits.max_groups,&hidden_grid));
    REQUIRE(launch(count,denoise_local,limits.max_groups,&denoise_grid));
    REQUIRE(launch(output_count,process_local,limits.max_groups,&process_grid));
    for (uint32_t i = HIDDEN; i < BUFFER_COUNT; ++i) REQUIRE(launch((uint32_t)(sizes[i]/4),poison_local,limits.max_groups,&poison_grid[i]));
    ogpu_next_shader shaders[] = {hidden_shader(),denoise_shader(),process_shader(),poison_shader()};
    const ogpu_next_shader_requirements *requirements[] = {&hidden_requirements,&denoise_requirements,&process_requirements,&poison_requirements};
    for (uint32_t i = 0; i < 4; ++i) {
        ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_COMPUTE,
            1,&shaders[i],NULL,0,&requirements[i]->header,NULL,0,0}; TRY(ogpu_next_executable_create(device,&ed,&kernels[i]));
    }
    ogpu_next_shader graphics[2]; application_shaders(graphics);
    ogpu_next_color_state color = {OGPU_NEXT_RGBA8_UNORM,15,0,1,0,0,1,0,0};
    ogpu_next_graphics_state gs = {HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),OGPU_NEXT_TRIANGLES,OGPU_NEXT_CULL_NONE,
        OGPU_NEXT_FRONT_CCW,1,1,&color,0,0,0,0,{0},0,{0},{0},NULL,NULL,0,0};
    ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_GRAPHICS,2,graphics,
        &gs.header,OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR,&application_requirements.header,NULL,0,0}; TRY(ogpu_next_executable_create(device,&ed,&raster));
    for (uint32_t i = 0; i < BUFFER_COUNT; ++i) {
        REQUIRE(memory_create(device,sizes[i],OGPU_NEXT_USAGE_STORAGE | OGPU_NEXT_USAGE_COPY_SRC | OGPU_NEXT_USAGE_COPY_DST,
            OGPU_NEXT_MEMORY_LOCAL,&buffers[i]) == EXIT_SUCCESS);
        TRY(ogpu_next_memory_address((ogpu_next_span){buffers[i],GUARD,payloads[i]},&addresses[i]));
    }
    REQUIRE(memory_create(device,upload_size,OGPU_NEXT_USAGE_COPY_SRC,OGPU_NEXT_MEMORY_HOST_VISIBLE,&upload) == EXIT_SUCCESS);
    REQUIRE(memory_create(device,total,OGPU_NEXT_USAGE_COPY_DST,OGPU_NEXT_MEMORY_HOST_VISIBLE,&readback) == EXIT_SUCCESS);
    REQUIRE(memory_create(device,16,OGPU_NEXT_USAGE_INDIRECT,OGPU_NEXT_MEMORY_HOST_VISIBLE,&draw) == EXIT_SUCCESS);
    ogpu_next_mapping um = {0}, rm = {0}, dm = {0};
    TRY(ogpu_next_memory_map((ogpu_next_span){upload,0,upload_size},&um)); TRY(ogpu_next_memory_map((ogpu_next_span){readback,0,total},&rm));
    TRY(ogpu_next_memory_map((ogpu_next_span){draw,0,16},&dm));
    const ogpu_next_draw_arguments triangle = {3,1,0,0}; memcpy(dm.data,&triangle,sizeof(triangle)); TRY(ogpu_next_memory_flush((ogpu_next_span){draw,0,16}));
    ogpu_next_image_desc id = {HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),OGPU_NEXT_RGBA8_UNORM,OGPU_NEXT_IMAGE_2D,
        1,1,1,{ow,oh,1},OGPU_NEXT_IMAGE_COLOR_ATTACHMENT | OGPU_NEXT_IMAGE_COPY_SRC,0,NULL,0,0,NULL};
    uint32_t compatible[32]; ogpu_next_requirements req = {0}; req.compatible_types = compatible; req.compatible_type_capacity = 32;
    TRY(ogpu_next_image_requirements(device,&id,&req)); REQUIRE(req.compatible_type_count > 0);
    TRY(ogpu_next_image_create_unbound(device,&id,&target)); TRY(ogpu_next_memory_create_dedicated_image(target,compatible[0],&backing));
    TRY(ogpu_next_image_bind(target,(ogpu_next_span){backing,0,req.size}));
    ogpu_next_subresources range = {OGPU_NEXT_ASPECT_COLOR,0,1,0,1};
    ogpu_next_view_desc vd = {HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),OGPU_NEXT_RGBA8_UNORM,OGPU_NEXT_VIEW_2D,OGPU_NEXT_IMAGE_COLOR_ATTACHMENT,{0,0,0,0},range};
    TRY(ogpu_next_view_create(target,&vd,&view));
    ogpu_next_host_requirements br = {0}, rr = {0}, sr = {0};
    TRY(ogpu_next_barrier_scratch_requirements(0,1,&br)); TRY(ogpu_next_render_scratch_requirements(1,&rr)); TRY(ogpu_next_submit_scratch_requirements(1,0,1,&sr));
    REQUIRE(br.alignment <= _Alignof(max_align_t) && rr.alignment <= _Alignof(max_align_t) && sr.alignment <= _Alignof(max_align_t));
    bs = malloc((size_t)br.size); rs = malloc((size_t)rr.size); ss = malloc((size_t)sr.size); REQUIRE(bs && rs && ss);
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena)); TRY(ogpu_next_timeline_create(device,0,&done));
    weights = malloc(sizes[WEIGHTS]); REQUIRE(weights != NULL); poison_host(weights,sizes[WEIGHTS]);
    REQUIRE(file_bytes(argv[1],weights+GUARD,payloads[WEIGHTS],0)); memcpy(um.data,weights,sizes[WEIGHTS]);
    TRY(ogpu_next_memory_flush((ogpu_next_span){upload,0,sizes[WEIGHTS]}));
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    ogpu_next_encoder *encoder = NULL; ogpu_next_list *list = NULL; TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    ogpu_next_copy_memory(encoder,(ogpu_next_span){buffers[WEIGHTS],0,sizes[WEIGHTS]},(ogpu_next_span){upload,0,sizes[WEIGHTS]});
    for (uint32_t i = HIDDEN; i < BUFFER_COUNT; ++i) {
        PoisonArguments args = {.arg_output_data=addresses[i]-GUARD,.arg_count=(uint32_t)(sizes[i]/4),.arg_dispatch_width=poison_grid[i].stride};
        dispatch(encoder,kernels[3],poison_grid[i],&args,sizeof(args));
    }
    TRY(ogpu_next_commands_end(encoder,&list)); ogpu_next_sync_point signal = {{done,1},OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {HEADER(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),1,0,1,&list,NULL,&signal,ss,sr.size};
    ogpu_next_queue *queue = ogpu_next_device_queue(device,domain,0); pending = 1; TRY(ogpu_next_queue_submit(queue,&submit));
    TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0;
    HiddenArguments hidden = {.arg_input_data=addresses[INPUT],.arg_weights=addresses[WEIGHTS],.arg_output_data=addresses[HIDDEN],
        .arg_width=width,.arg_height=height,.arg_dispatch_width=hidden_grid.stride};
    DenoiseArguments denoise = {.arg_input_data=addresses[INPUT],.arg_weights=addresses[WEIGHTS],.arg_hidden=addresses[HIDDEN],
        .arg_output_data=addresses[DENOISED],.arg_count=count,.arg_dispatch_width=denoise_grid.stride};
    ProcessArguments process = {.arg_input_data=addresses[DENOISED],.arg_output_data=addresses[COLOR],.arg_width=width,.arg_height=height,
        .arg_out_width=ow,.arg_out_height=oh,.arg_dispatch_width=process_grid.stride};
    DisplayArguments display = {.arg_pixels=addresses[COLOR],.arg_width=ow,.arg_height=oh};
    for (uint32_t diagnostic = 0; diagnostic < 2; ++diagnostic) {
        TRY(ogpu_next_arena_reset(arena)); TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
        dependency(encoder,OGPU_NEXT_STAGE_ALL,OGPU_NEXT_STAGE_COPY | OGPU_NEXT_STAGE_COMPUTE,
            OGPU_NEXT_ACCESS_READ | OGPU_NEXT_ACCESS_WRITE,OGPU_NEXT_ACCESS_COPY_WRITE | OGPU_NEXT_ACCESS_STORAGE_READ | OGPU_NEXT_ACCESS_STORAGE_WRITE);
        ogpu_next_copy_memory(encoder,(ogpu_next_span){buffers[INPUT],0,sizes[INPUT]},(ogpu_next_span){upload,0,sizes[INPUT]});
        dependency(encoder,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_STORAGE_READ);
        if (diagnostic) {
            for (uint32_t i = HIDDEN; i < BUFFER_COUNT; ++i) {
                PoisonArguments args = {.arg_output_data=addresses[i]-GUARD,.arg_count=(uint32_t)(sizes[i]/4),.arg_dispatch_width=poison_grid[i].stride};
                dispatch(encoder,kernels[3],poison_grid[i],&args,sizeof(args));
            }
            dependency(encoder,OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_ACCESS_STORAGE_WRITE,OGPU_NEXT_ACCESS_STORAGE_WRITE);
        }
        dispatch(encoder,kernels[0],hidden_grid,&hidden,sizeof(hidden));
        dependency(encoder,OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_ACCESS_STORAGE_WRITE,OGPU_NEXT_ACCESS_STORAGE_READ);
        dispatch(encoder,kernels[1],denoise_grid,&denoise,sizeof(denoise));
        dependency(encoder,OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_ACCESS_STORAGE_WRITE,OGPU_NEXT_ACCESS_STORAGE_READ);
        dispatch(encoder,kernels[2],process_grid,&process,sizeof(process));
        dependency(encoder,OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_STAGE_FRAGMENT,OGPU_NEXT_ACCESS_STORAGE_WRITE,OGPU_NEXT_ACCESS_STORAGE_READ);
        ogpu_next_image_barrier barrier = {target,range,OGPU_NEXT_ACCESS_READ | OGPU_NEXT_ACCESS_WRITE,OGPU_NEXT_ACCESS_COLOR_WRITE,
            OGPU_NEXT_STATE_UNDEFINED,OGPU_NEXT_STATE_COLOR_ATTACHMENT,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1};
        ogpu_next_dependency image_dep = {HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),OGPU_NEXT_STAGE_ALL,OGPU_NEXT_STAGE_COLOR,0,0,0,1,0,NULL,&barrier,bs,br.size};
        ogpu_next_barrier(encoder,&image_dep);
        ogpu_next_attachment attachment = {view,NULL,OGPU_NEXT_STATE_COLOR_ATTACHMENT,0,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.f32={0,0,0,1}}};
        ogpu_next_render_desc render = {HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),0,0,ow,oh,1,0,1,0,1,&attachment,NULL,NULL,{rs,rr.size}};
        ogpu_next_viewport_state vp = {HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),0,0,(float)ow,(float)oh,0,1,0,0,ow,oh};
        ogpu_next_render_begin(encoder,&render); ogpu_next_bind_executable(encoder,raster); ogpu_next_set_graphics_state(encoder,&vp.header);
        ogpu_next_set_inline(encoder,OGPU_NEXT_STAGE_VERTEX | OGPU_NEXT_STAGE_FRAGMENT,0,sizeof(display),&display);
        ogpu_next_indirect indirect = {{draw,0,16},16,1,{NULL,0,0}}; ogpu_next_draw_indirect(encoder,&indirect); ogpu_next_render_end(encoder);
        image_dep.before = OGPU_NEXT_STAGE_COLOR; image_dep.after = OGPU_NEXT_STAGE_COPY;
        barrier.before = OGPU_NEXT_ACCESS_COLOR_WRITE; barrier.after = OGPU_NEXT_ACCESS_COPY_READ; barrier.old_state = OGPU_NEXT_STATE_COLOR_ATTACHMENT;
        barrier.new_state = OGPU_NEXT_STATE_COPY_SRC; barrier.discard = 0; ogpu_next_barrier(encoder,&image_dep);
        ogpu_next_image_copy copy = {{OGPU_NEXT_ASPECT_COLOR,0,0,1,{0,0,0},{ow,oh,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};
        ogpu_next_copy_from_image(encoder,(ogpu_next_span){readback,GUARD,final_bytes},target,&copy);
        if (diagnostic) {
            dependency(encoder,OGPU_NEXT_STAGE_COMPUTE | OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_COPY,
                OGPU_NEXT_ACCESS_STORAGE_WRITE | OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_COPY_READ);
            for (uint32_t i = 0; i < BUFFER_COUNT; ++i)
                ogpu_next_copy_memory(encoder,(ogpu_next_span){readback,offsets[i],sizes[i]},(ogpu_next_span){buffers[i],0,sizes[i]});
        }
        dependency(encoder,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_HOST,OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_HOST_READ);
        TRY(ogpu_next_commands_end(encoder,&list));
        /* The command list is recorded once per mode, not once per image. */
        for (int frame = 0; frame < argc-7; ++frame) {
            REQUIRE(path_join(path,argv[7+frame],"input.f32")); poison_host(um.data,sizes[INPUT]);
            REQUIRE(file_bytes(path,(char *)um.data+GUARD,payloads[INPUT],0));
            TRY(ogpu_next_memory_flush((ogpu_next_span){upload,0,sizes[INPUT]}));
            size_t read_size = diagnostic ? total : final_bytes+2*GUARD; poison_host(rm.data,read_size);
            TRY(ogpu_next_memory_flush((ogpu_next_span){readback,0,read_size}));
            ++signal.point.value; pending = 1; TRY(ogpu_next_queue_submit(queue,&submit));
            TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0;
            TRY(ogpu_next_memory_invalidate((ogpu_next_span){readback,0,read_size})); unsigned char *pixels = rm.data;
            REQUIRE(guards(pixels,final_bytes+2*GUARD));
            snprintf(name,sizeof(name),"%s-%d-final.rgba",diagnostic ? "diagnostic" : "normal",frame);
            REQUIRE(path_join(path,argv[6],name)); REQUIRE(file_bytes(path,pixels+GUARD,final_bytes,1));
            if (diagnostic) {
                const char *names[] = {"input","weights","hidden","denoised","processed"};
                for (uint32_t i = 0; i < BUFFER_COUNT; ++i) {
                    REQUIRE(guards(pixels+offsets[i],sizes[i])); snprintf(name,sizeof(name),"diagnostic-%d-%s.f32",frame,names[i]);
                    REQUIRE(path_join(path,argv[6],name)); REQUIRE(file_bytes(path,pixels+offsets[i]+GUARD,payloads[i],1));
                }
                REQUIRE(memcmp(pixels+offsets[INPUT],um.data,sizes[INPUT]) == 0);
                REQUIRE(memcmp(pixels+offsets[WEIGHTS],weights,sizes[WEIGHTS]) == 0);
            }
            printf("Foundation %s frame %d %ux%u -> %ux%u: replay, output guards, no intermediate CPU read/wait PASS\n",
                diagnostic ? "diagnostic" : "normal",frame,width,height,ow,oh);
        }
    }
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending learned-image foundation work after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_timeline_destroy(done); ogpu_next_view_destroy(view); ogpu_next_image_destroy(target);
    ogpu_next_memory_destroy(backing); ogpu_next_executable_destroy(raster);
    for (uint32_t i = 0; i < 4; ++i) ogpu_next_executable_destroy(kernels[i]);
    for (uint32_t i = 0; i < BUFFER_COUNT; ++i) ogpu_next_memory_destroy(buffers[i]);
    ogpu_next_memory_destroy(upload); ogpu_next_memory_destroy(readback); ogpu_next_memory_destroy(draw);
    ogpu_next_device_destroy(device); ogpu_next_discovery_destroy(discovery); free(weights); return result;
}
