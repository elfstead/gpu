/* Public-only foundation compiler consumer. Same host source for both layouts. */
#include <foundation.generated.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define HEADER(type, kind) {kind,OGPU_NEXT_VERSION,sizeof(type),0,NULL}
#define REQUIRE(c) do { if (!(c)) { fprintf(stderr,"line %d: %s\n",__LINE__,#c); goto cleanup; } } while (0)
#define TRY(call) do { ogpu_next_status s_ = (call); if (s_ != OGPU_NEXT_OK) { fprintf(stderr,"%s: %" PRId32 "\n",#call,s_); goto cleanup; } } while (0)
static ogpu_next_query query(uint32_t kind, void *data, uint32_t capacity, uint32_t size) {
    return (ogpu_next_query){HEADER(ogpu_next_query,kind),data,capacity,0,size,0};
}
int main(void) {
    int result = EXIT_FAILURE, pending = 0;
    ogpu_next_discovery *discovery = NULL; ogpu_next_device *device = NULL;
    ogpu_next_memory *memory = NULL; ogpu_next_executable *pipeline = NULL;
    ogpu_next_arena *arena = NULL; ogpu_next_timeline *done = NULL; void *scratch = NULL;
    const uint32_t count = 4099, sentinel = UINT32_C(0xa55adead);
    uint32_t domain = UINT32_MAX;
    TRY(ogpu_next_discovery_create(OGPU_NEXT_VERSION,&discovery));
    for (uint32_t index = 0; index < ogpu_next_discovery_count(discovery); ++index) {
        const ogpu_next_adapter *adapter = ogpu_next_discovery_adapter(discovery,index);
        ogpu_next_feature_info features = {0}; ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));
        TRY(ogpu_next_adapter_query(adapter,&q));
        if (!features.baseline_supported || (features.available & transform_required_features) != transform_required_features) continue;
        ogpu_next_queue_info queues[32]; q = query(OGPU_NEXT_QUERY_QUEUES,queues,32,sizeof(queues[0]));
        TRY(ogpu_next_adapter_query(adapter,&q));
        for (uint32_t i = 0; i < q.count; ++i) if (queues[i].count && (queues[i].flags & OGPU_NEXT_QUEUE_COMPUTE)) { domain = queues[i].domain; break; }
        if (domain == UINT32_MAX) continue;
        ogpu_next_queue_request request = {domain,1,0.5f};
        ogpu_next_device_desc desc = {HEADER(ogpu_next_device_desc,OGPU_NEXT_DEVICE_DESC),&request,1,0,transform_required_features};
        TRY(ogpu_next_device_create(adapter,&desc,&device)); break;
    }
    REQUIRE(device != NULL);
    ogpu_next_discovery_destroy(discovery); discovery = NULL;
    ogpu_next_feature_info features = {0}; ogpu_next_execution_limits limits = {0};
    ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features)); TRY(ogpu_next_device_query(device,&q));
    q = query(OGPU_NEXT_QUERY_EXECUTION_LIMITS,&limits,1,sizeof(limits)); TRY(ogpu_next_device_query(device,&q));
    REQUIRE(transform_compatible(&features,&limits));
    ogpu_next_feature_info missing = features; missing.baseline_supported = 0; REQUIRE(!transform_compatible(&missing,&limits));
    ogpu_next_execution_limits small = limits; small.max_inline_size = sizeof(TransformArguments)-1;
    REQUIRE(!transform_compatible(&features,&small));
    small = limits; small.max_local_invocations = 0; REQUIRE(!transform_compatible(&features,&small));
    for (uint32_t axis = 0; axis < 3; ++axis) { small = limits; small.max_local_size[axis] = 0; REQUIRE(!transform_compatible(&features,&small)); }
    REQUIRE(transform_local[1] == 1 && transform_local[2] == 1);
    uint32_t groups = count/transform_local[0]+(count%transform_local[0] != 0);
    REQUIRE(groups <= limits.max_groups[0]);
    ogpu_next_shader shader = transform_shader();
    ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_COMPUTE,
        1,&shader,NULL,0,&transform_requirements.header,NULL,0,0};
    TRY(ogpu_next_executable_create(device,&ed,&pipeline));
    ogpu_next_memory_desc md = {HEADER(ogpu_next_memory_desc,OGPU_NEXT_MEMORY_DESC),64+(uint64_t)(count+2)*4,8,
        OGPU_NEXT_USAGE_STORAGE,UINT32_MAX,OGPU_NEXT_MEMORY_LINEAR,NULL,0,0};
    uint32_t compatible[32]; ogpu_next_requirements req = {0}; req.compatible_types = compatible; req.compatible_type_capacity = 32;
    TRY(ogpu_next_memory_requirements(device,&md,&req));
    ogpu_next_memory_type_info types[32]; q = query(OGPU_NEXT_QUERY_MEMORY_TYPES,types,32,sizeof(types[0]));
    TRY(ogpu_next_device_query(device,&q));
    for (uint32_t i = 0; i < req.compatible_type_count && md.memory_type == UINT32_MAX; ++i)
        for (uint32_t j = 0; j < q.count; ++j) if (types[j].id == compatible[i] && (types[j].properties & OGPU_NEXT_MEMORY_HOST_VISIBLE)) {
            md.memory_type = types[j].id; break;
        }
    REQUIRE(md.memory_type != UINT32_MAX); TRY(ogpu_next_memory_create(device,&md,&memory));
    ogpu_next_span all = {memory,0,md.size}; ogpu_next_mapping mapping = {0}; TRY(ogpu_next_memory_map(all,&mapping));
    uint64_t address = 0; TRY(ogpu_next_memory_address((ogpu_next_span){memory,68,(uint64_t)count*4},&address));
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena)); TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    ogpu_next_encoder *encoder = NULL; ogpu_next_list *list = NULL; TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    ogpu_next_bind_executable(encoder,pipeline);
    TransformArguments args = {0}; args.arg_data = address; args.arg_count = count;
    ogpu_next_set_inline(encoder,OGPU_NEXT_STAGE_COMPUTE,0,sizeof(args),&args);
    memset(&args,0xcc,sizeof(args)); /* Command owns immediate bytes, not this host struct. */
    ogpu_next_launch launch = {{groups,1,1},0,NULL};
    ogpu_next_dependency edge = {HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_STAGE_COMPUTE,
        OGPU_NEXT_ACCESS_STORAGE_WRITE,OGPU_NEXT_ACCESS_STORAGE_READ | OGPU_NEXT_ACCESS_STORAGE_WRITE,0,0,0,NULL,NULL,NULL,0};
    for (uint32_t pass = 0; pass < 3; ++pass) {
        if (pass) ogpu_next_barrier(encoder,&edge);
        ogpu_next_dispatch(encoder,&launch);
    }
    edge.after = OGPU_NEXT_STAGE_HOST; edge.global_after = OGPU_NEXT_ACCESS_HOST_READ; ogpu_next_barrier(encoder,&edge);
    TRY(ogpu_next_commands_end(encoder,&list));
    ogpu_next_host_requirements sr = {0}; TRY(ogpu_next_submit_scratch_requirements(1,0,1,&sr));
    REQUIRE(sr.alignment <= _Alignof(max_align_t)); scratch = malloc((size_t)sr.size); REQUIRE(scratch != NULL);
    ogpu_next_sync_point signal = {{done,1},OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {HEADER(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),1,0,1,&list,NULL,&signal,scratch,sr.size};
    for (uint32_t replay = 0; replay < 3; ++replay) {
        memset(mapping.data,0xa5,(size_t)md.size); uint32_t *words = (uint32_t *)((char *)mapping.data+64);
        words[0] = words[count+1] = sentinel;
        for (uint32_t i = 0; i < count; ++i) words[i+1] = i*13+replay*31;
        TRY(ogpu_next_memory_flush(all)); signal.point.value = replay+1; pending = 1;
        TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),&submit));
        TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0;
        TRY(ogpu_next_memory_invalidate(all));
        for (uint32_t i = 0; i < count; ++i) {
            uint32_t expected = i*13+replay*31;
            for (uint32_t pass = 0; pass < 3; ++pass) expected = expected*3+7;
            REQUIRE(words[i+1] == expected);
        }
        REQUIRE(words[0] == sentinel && words[count+1] == sentinel);
        for (uint32_t i = 0; i < 64; ++i) REQUIRE(((unsigned char *)mapping.data)[i] == 0xa5);
    }
    printf("Foundation compiler consumer passes: %u elements, local=%u, data offset=%zu, count offset=%zu, three passes/replays and guards.\n",
        count,transform_local[0],offsetof(TransformArguments,arg_data),offsetof(TransformArguments,arg_count));
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending compiler-consumer work after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_timeline_destroy(done); ogpu_next_executable_destroy(pipeline);
    ogpu_next_memory_destroy(memory); ogpu_next_device_destroy(device); ogpu_next_discovery_destroy(discovery);
    free(scratch); return result;
}
