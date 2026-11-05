/* Public foundation API only. No optional device features, hidden transitions,
 * inferred ownership or native Vulkan calls. Compile once per shader/profile. */
#include <storage.generated.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Caller policy for these fixtures: resource entries share one stage pair.
 * The API itself takes independent scopes; this helper edits caller-owned entries
 * before recording. Arrays must be mutable; no resources or payloads are copied. */
static inline void common_barrier_scopes(ogpu_next_dependency *d, ogpu_next_barrier_scope *s) {
    d->global = s;
    d->global_count = (s->access_before || s->access_after || (!d->memory_count && !d->image_count)) ? 1u : 0u;
    for (uint32_t i=0;i<d->memory_count;++i) {
        ogpu_next_memory_barrier *b=(ogpu_next_memory_barrier *)d->memory+i;
        b->scope.stages_before=s->stages_before; b->scope.stages_after=s->stages_after;
    }
    for (uint32_t i=0;i<d->image_count;++i) {
        ogpu_next_image_barrier *b=(ogpu_next_image_barrier *)d->images+i;
        b->scope.stages_before=s->stages_before; b->scope.stages_after=s->stages_after;
    }
}
static inline void common_barrier(ogpu_next_encoder *e, ogpu_next_dependency *d,
    ogpu_next_barrier_scope *s, ogpu_next_host_span scratch) {
    common_barrier_scopes(d,s); ogpu_next_barrier(e,d,scratch);
}


#define HEADER(type, kind) {kind, OGPU_NEXT_VERSION, sizeof(type), 0, NULL}
#define REQUIRE(c) do { if (!(c)) { fprintf(stderr,"line %d: %s\n",__LINE__,#c); goto cleanup; } } while (0)
#define TRY(call) do { ogpu_next_status s_ = (call); if (s_ != OGPU_NEXT_OK) { fprintf(stderr,"%s: %" PRId32 "\n",#call,s_); goto cleanup; } } while (0)
static ogpu_next_query query(uint32_t kind, void *data, uint32_t capacity, uint32_t size) {
    return (ogpu_next_query){HEADER(ogpu_next_query,kind),data,capacity,0,size,0};
}
static uint64_t aligned(uint64_t size, uint64_t alignment) {
    return (size + alignment - 1) / alignment * alignment;
}
static int memory(ogpu_next_device *device, uint64_t size, uint64_t alignment,
                  uint64_t usage, ogpu_next_memory **out) {
    int result = EXIT_FAILURE;
    ogpu_next_memory_type_info types[32];
    ogpu_next_query q = query(OGPU_NEXT_QUERY_MEMORY_TYPES,types,32,sizeof(types[0]));
    TRY(ogpu_next_device_query(device,&q));
    uint32_t compatible[32]; ogpu_next_requirements req = {0};
    req.compatible_type_capacity = 32; req.compatible_types = compatible;
    ogpu_next_memory_desc desc = {HEADER(ogpu_next_memory_desc,OGPU_NEXT_MEMORY_DESC),
        size,alignment,usage,UINT32_MAX,OGPU_NEXT_MEMORY_LINEAR,NULL,0,0};
    TRY(ogpu_next_memory_requirements(device,&desc,&req));
    for (uint32_t i = 0; i < req.compatible_type_count && desc.memory_type == UINT32_MAX; ++i)
        for (uint32_t j = 0; j < q.count; ++j)
            if (types[j].id == compatible[i] && (types[j].properties & OGPU_NEXT_MEMORY_HOST_VISIBLE)) {
                desc.memory_type = types[j].id; break;
            }
    REQUIRE(desc.memory_type != UINT32_MAX);
    TRY(ogpu_next_memory_create(device,&desc,out)); result = EXIT_SUCCESS;
cleanup:
    return result;
}
static void pixel(unsigned char *dst, uint32_t i, uint32_t replay, int output) {
#if OPERATION == 3
    float value[2] = {(float)(i + replay), -(float)(i + replay + 1)};
    if (output) for (uint32_t c = 0; c < 2; ++c) value[c] = value[c]*2.0f+1.0f;
    memcpy(dst,value,sizeof(value));
#else
    for (uint32_t c = 0; c < 4; ++c) {
        int value = (int)((i*3+c*13+replay*7)%101);
#if OPERATION == 0
        if (output) value = 255-value;
#elif OPERATION == 1
        value -= 50;
        if (output) value = -value;
#else
        if (output) value += 17;
#endif
        dst[c] = (unsigned char)value;
    }
#endif
}
/* Consumer preparation: query a supported maximum to learn reservation geometry,
 * then choose the actual capacity and backing explicitly. No allocation here. */
static ogpu_next_status heap_requirements_for(ogpu_next_device *device, uint32_t kind,
    uint32_t usage, ogpu_next_heap_requirements *out) {
    ogpu_next_descriptor_limits caps;
    ogpu_next_query q={{OGPU_NEXT_QUERY_DESCRIPTOR_LIMITS,OGPU_NEXT_VERSION,sizeof(q),0,NULL},&caps,1,0,sizeof(caps),0};
    ogpu_next_status status=ogpu_next_device_query(device,&q);
    if(status!=OGPU_NEXT_OK)return status;
    ogpu_next_heap_request request={kind,OGPU_NEXT_HEAP_MEMORY,usage,0,
        kind==OGPU_NEXT_HEAP_RESOURCE ? caps.max_resource_capacity : caps.max_sampler_capacity};
    if(request.capacity>PTRDIFF_MAX)request.capacity=PTRDIFF_MAX;
    return ogpu_next_heap_requirements_query(device,&request,out);
}

int main(void) {
    int result = EXIT_FAILURE, pending = 0;
    ogpu_next_descriptor_heap *binding=NULL;
    ogpu_next_discovery *discovery = NULL; ogpu_next_device *device = NULL;
    ogpu_next_format_info *formats = NULL;
    ogpu_next_memory *host = NULL, *heap = NULL, *backing[2] = {NULL,NULL};
    ogpu_next_image *images[2] = {NULL,NULL}; ogpu_next_view *views[2] = {NULL,NULL};
    ogpu_next_executable *pipeline = NULL; ogpu_next_argument_layout *arguments = NULL; ogpu_next_arena *arena = NULL; ogpu_next_timeline *done = NULL;
    void *descriptor_scratch = NULL, *barrier_scratch = NULL, *submit_scratch = NULL;
    const uint32_t width = 8, height = 4;
    const uint64_t bytes = width*height*TEXEL_BYTES, host_size = 2048;
    uint32_t domain = UINT32_MAX;
    TRY(ogpu_next_discovery_create(OGPU_NEXT_VERSION,&discovery));
    for (uint32_t index = 0; index < ogpu_next_discovery_count(discovery); ++index) {
        const ogpu_next_adapter *adapter = ogpu_next_discovery_adapter(discovery,index);
        ogpu_next_feature_info features = {0}; ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));
        TRY(ogpu_next_adapter_query(adapter,&q)); if (!features.baseline_supported) continue;
        ogpu_next_queue_info queues[32]; q = query(OGPU_NEXT_QUERY_QUEUES,queues,32,sizeof(queues[0]));
        TRY(ogpu_next_adapter_query(adapter,&q));
        for (uint32_t i = 0; i < q.count; ++i)
            if (queues[i].count && (queues[i].flags & OGPU_NEXT_QUEUE_COMPUTE)) { domain = queues[i].domain; break; }
        if (domain == UINT32_MAX) continue;
        ogpu_next_queue_request request = {domain,1,0.5f};
        ogpu_next_device_desc desc = {HEADER(ogpu_next_device_desc,OGPU_NEXT_DEVICE_DESC),&request,1,0,0};
        TRY(ogpu_next_device_create(adapter,&desc,&device)); break;
    }
    REQUIRE(device != NULL);
    ogpu_next_query q = query(OGPU_NEXT_QUERY_FORMATS,NULL,0,sizeof(*formats));
    TRY(ogpu_next_device_query(device,&q));
    formats = calloc(q.count,sizeof(*formats)); REQUIRE(formats != NULL); q.data = formats; q.capacity = q.count;
    TRY(ogpu_next_device_query(device,&q));
    uint64_t available = 0;
    for (uint32_t i = 0; i < q.count; ++i) if (formats[i].format == IMAGE_FORMAT) available = formats[i].optimal_features;
    /* Generated per-kind unions suffice here because both views have one format.
     * Neither shader compatibility() nor device creation checks view formats. */
    uint64_t needed = INPUT_REQUIREMENTS | OUTPUT_REQUIREMENTS | OGPU_NEXT_FORMAT_COPY_SRC | OGPU_NEXT_FORMAT_COPY_DST;
    if ((available & needed) != needed) {
        printf("Storage format=%u mode=%u UNSUPPORTED flags: need=%" PRIx64 " have=%" PRIx64 "\n",
               IMAGE_FORMAT,FORMATLESS_MODE,needed,available); result = EXIT_SUCCESS; goto cleanup;
    }
    ogpu_next_image_profile profile = {HEADER(ogpu_next_image_profile,OGPU_NEXT_IMAGE_PROFILE),
        IMAGE_FORMAT,OGPU_NEXT_IMAGE_2D,OGPU_NEXT_IMAGE_STORAGE | OGPU_NEXT_IMAGE_COPY_SRC | OGPU_NEXT_IMAGE_COPY_DST,0,0};
    ogpu_next_image_limits il = {0}; ogpu_next_status support = ogpu_next_image_limits_query(device,&profile,&il);
    if (support == OGPU_NEXT_UNSUPPORTED) {
        printf("Storage format=%u mode=%u UNSUPPORTED profile\n",IMAGE_FORMAT,FORMATLESS_MODE);
        result = EXIT_SUCCESS; goto cleanup;
    }
    REQUIRE(support == OGPU_NEXT_OK && il.max_extent[0] >= width && il.max_extent[1] >= height && (il.sample_counts & 1));
    ogpu_next_feature_info features = {0}; ogpu_next_execution_limits execution = {0};
    q = query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features)); TRY(ogpu_next_device_query(device,&q));
    q = query(OGPU_NEXT_QUERY_EXECUTION_LIMITS,&execution,1,sizeof(execution)); TRY(ogpu_next_device_query(device,&q));
    REQUIRE(storage_compatible(&features,&execution) && features.enabled == 0);
    ogpu_next_subresources all = {OGPU_NEXT_ASPECT_COLOR,0,1,0,1};
    for (uint32_t i = 0; i < 2; ++i) {
        ogpu_next_image_desc desc = {HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),
            IMAGE_FORMAT,OGPU_NEXT_IMAGE_2D,1,1,1,{width,height,1},profile.usage,0,NULL,0,0,NULL};
        uint32_t types[32]; ogpu_next_requirements req = {0}; req.compatible_types = types; req.compatible_type_capacity = 32;
        TRY(ogpu_next_image_requirements(device,&desc,&req)); REQUIRE(req.compatible_type_count > 0);
        TRY(ogpu_next_image_create_unbound(device,&desc,&images[i]));
        TRY(ogpu_next_memory_create_dedicated_image(images[i],types[0],&backing[i]));
        TRY(ogpu_next_image_bind(images[i],(ogpu_next_span){backing[i],0,req.size}));
        ogpu_next_view_desc vd = {HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),IMAGE_FORMAT,OGPU_NEXT_VIEW_2D,
            OGPU_NEXT_IMAGE_STORAGE,{0,0,0,0},all};
        TRY(ogpu_next_view_create(images[i],&vd,&views[i]));
    }
    REQUIRE(memory(device,host_size,64,OGPU_NEXT_USAGE_COPY_SRC | OGPU_NEXT_USAGE_COPY_DST,&host) == EXIT_SUCCESS);
    ogpu_next_mapping host_map = {0}, heap_map = {0};
    TRY(ogpu_next_memory_map((ogpu_next_span){host,0,host_size},&host_map));
    ogpu_next_resource_descriptor descriptors[2] = {
        {OGPU_NEXT_DESCRIPTOR_STORAGE_IMAGE,OGPU_NEXT_STATE_GENERAL,views[0],{NULL,0,0}},
        {OGPU_NEXT_DESCRIPTOR_STORAGE_IMAGE,OGPU_NEXT_STATE_GENERAL,views[1],{NULL,0,0}}};
    ogpu_next_descriptor_shape image_shape;
    TRY(ogpu_next_resource_descriptor_shape(device,&descriptors[0],&image_shape));
    ogpu_next_heap_requirements resource_req;
    TRY(heap_requirements_for(device,1,OGPU_NEXT_HEAP_SHADER_VISIBLE|OGPU_NEXT_HEAP_CPU_WRITE,&resource_req));
    uint64_t stride = image_shape.array_stride;
    REQUIRE(stride >= image_shape.size && stride % image_shape.alignment == 0);
    uint64_t reserved = aligned(stride*2,resource_req.reserved_alignment), heap_size = reserved+resource_req.reserved_size;
    REQUIRE(memory(device,heap_size,resource_req.backing_alignment,OGPU_NEXT_USAGE_DESCRIPTOR_HEAP,&heap) == EXIT_SUCCESS);
    TRY(ogpu_next_memory_map((ogpu_next_span){heap,0,heap_size},&heap_map));
    ogpu_next_host_requirements ds = {0}, bs = {0}, ss = {0};
    TRY(ogpu_next_descriptor_scratch_requirements(device,OGPU_NEXT_DESCRIPTOR_WRITE_RESOURCE,2,&ds));
    TRY(ogpu_next_barrier_scratch_requirements(device,1,0,2,&bs)); TRY(ogpu_next_submit_scratch_requirements(device,1,&(ogpu_next_submit_desc){.list_count=1,.wait_count=0,.signal_count=1},&ss));
    REQUIRE(ds.alignment <= _Alignof(max_align_t) && bs.alignment <= _Alignof(max_align_t) && ss.alignment <= _Alignof(max_align_t));
    descriptor_scratch = malloc((size_t)ds.size); barrier_scratch = malloc((size_t)bs.size); submit_scratch = malloc((size_t)ss.size);
    REQUIRE(descriptor_scratch && barrier_scratch && submit_scratch);

    ogpu_next_heap_desc hd={{1,OGPU_NEXT_HEAP_MEMORY,OGPU_NEXT_HEAP_SHADER_VISIBLE|OGPU_NEXT_HEAP_CPU_WRITE,0,heap_size},
        {heap,0,heap_size},{heap_map.data,heap_map.size},reserved,resource_req.reserved_size};
    TRY(ogpu_next_heap_create(device,&hd,&binding));
    ogpu_next_heap_location destinations[2]={{binding,0},{binding,stride}};
    TRY(ogpu_next_write_resource_descriptors(device,2,descriptors,destinations,(ogpu_next_host_span){descriptor_scratch,ds.size}));
    TRY(ogpu_next_memory_flush((ogpu_next_span){heap,0,stride*2}));

    ogpu_next_shader shader = storage_shader();
    ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_COMPUTE,
        1,&shader,NULL,0,&storage_requirements.header,NULL,0,0};
    TRY(ogpu_next_argument_layout_create(device,&storage_arguments,&arguments));
    TRY(ogpu_next_executable_create(device,&ed,arguments,&pipeline));
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena)); TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    ogpu_next_encoder *encoder = NULL; ogpu_next_list *list = NULL; TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    ogpu_next_image_barrier barriers[2];
    for (uint32_t i = 0; i < 2; ++i) barriers[i] = (ogpu_next_image_barrier){{0,0,0,i ? OGPU_NEXT_ACCESS_STORAGE_WRITE : OGPU_NEXT_ACCESS_COPY_WRITE},images[i],all,OGPU_NEXT_STATE_UNDEFINED,OGPU_NEXT_STATE_GENERAL,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1,0};
    ogpu_next_barrier_scope dep_scope = {OGPU_NEXT_STAGE_ALL,OGPU_NEXT_STAGE_COPY | OGPU_NEXT_STAGE_COMPUTE,0,0};
    ogpu_next_host_span dep_scratch = {barrier_scratch,bs.size};
    ogpu_next_dependency dep = {1,0,2,0,&dep_scope,NULL,barriers};
    common_barrier(encoder,&dep,&dep_scope,dep_scratch);
    ogpu_next_image_copy copy = {{OGPU_NEXT_ASPECT_COLOR,0,0,1,{0,0,0},{width,height,1}},0,0,OGPU_NEXT_STATE_GENERAL,0};
    ogpu_next_copy_to_image(encoder,images[0],(ogpu_next_span){host,64,bytes},&copy);
    barriers[0].scope.access_before = OGPU_NEXT_ACCESS_COPY_WRITE; barriers[0].scope.access_after = OGPU_NEXT_ACCESS_STORAGE_READ;
    barriers[0].old_state = OGPU_NEXT_STATE_GENERAL; barriers[0].discard = 0;
    dep_scope.stages_before = OGPU_NEXT_STAGE_COPY; dep_scope.stages_after = OGPU_NEXT_STAGE_COMPUTE; dep.image_count = 1;
    common_barrier(encoder,&dep,&dep_scope,dep_scratch);
    ogpu_next_bind_heap(encoder,binding); ogpu_next_bind_executable(encoder,pipeline);
    StorageArguments args = {0}; args.arg_source = 0; args.arg_destination = (uint32_t)(stride/image_shape.array_stride);
    args.arg_width = width; args.arg_height = height;
    ogpu_next_set_arguments(encoder, arguments, 0, sizeof(args), &args);
    ogpu_next_launch launch = {{width/storage_local[0],height,1},0,NULL}; ogpu_next_dispatch(encoder,&launch);
    barriers[1].scope.access_before = OGPU_NEXT_ACCESS_STORAGE_WRITE; barriers[1].scope.access_after = OGPU_NEXT_ACCESS_COPY_READ;
    barriers[1].old_state = OGPU_NEXT_STATE_GENERAL; barriers[1].discard = 0;
    dep.images = &barriers[1]; dep_scope.stages_before = OGPU_NEXT_STAGE_COMPUTE; dep_scope.stages_after = OGPU_NEXT_STAGE_COPY;
    common_barrier(encoder,&dep,&dep_scope,dep_scratch);
    ogpu_next_copy_from_image(encoder,(ogpu_next_span){host,1024,bytes},images[1],&copy);
    dep.images = NULL; dep.image_count = 0; dep_scope.stages_before = OGPU_NEXT_STAGE_COPY; dep_scope.stages_after = OGPU_NEXT_STAGE_HOST;
    dep_scope.access_before = OGPU_NEXT_ACCESS_COPY_WRITE; dep_scope.access_after = OGPU_NEXT_ACCESS_HOST_READ;
    common_barrier(encoder,&dep,&dep_scope,dep_scratch); TRY(ogpu_next_commands_end(encoder,&list));
    ogpu_next_sync_point signal = {{done,1},OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {1,0,1,0,0,0,&list,NULL,&signal,NULL,NULL};
    ogpu_next_host_span submit_host = {submit_scratch,ss.size};
    ogpu_next_submit_outcome submit_outcome = {0};
    for (uint32_t replay = 0; replay < 2; ++replay) {
        memset(host_map.data,0xa5,(size_t)host_size);
        for (uint32_t i = 0; i < width*height; ++i) pixel((unsigned char *)host_map.data+64+i*TEXEL_BYTES,i,replay,0);
        TRY(ogpu_next_memory_flush((ogpu_next_span){host,0,host_size})); signal.point.value = replay+1;
        pending = 1; TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),1,&submit,&submit_outcome,submit_host));
        TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0;
        TRY(ogpu_next_memory_invalidate((ogpu_next_span){host,0,host_size}));
        for (uint32_t i = 0; i < width*height; ++i) {
            unsigned char expected[TEXEL_BYTES]; pixel(expected,i,replay,1);
            REQUIRE(memcmp((unsigned char *)host_map.data+1024+i*TEXEL_BYTES,expected,TEXEL_BYTES) == 0);
            pixel(expected,i,replay,0);
            REQUIRE(memcmp((unsigned char *)host_map.data+64+i*TEXEL_BYTES,expected,TEXEL_BYTES) == 0);
        }
        for (uint64_t i = 0; i < host_size; ++i)
            if (!(i >= 64 && i < 64+bytes) && !(i >= 1024 && i < 1024+bytes))
                REQUIRE(((unsigned char *)host_map.data)[i] == 0xa5);
    }
    printf("Storage format=%u mode=%u PASS: exact read/write/copy, two replays, guards, no optional device enables\n",
           IMAGE_FORMAT,FORMATLESS_MODE); result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending storage-image work after failure\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_timeline_destroy(done); ogpu_next_executable_destroy(pipeline); ogpu_next_argument_layout_destroy(arguments);
    for (uint32_t i = 0; i < 2; ++i) { ogpu_next_view_destroy(views[i]); ogpu_next_image_destroy(images[i]); ogpu_next_memory_destroy(backing[i]); }
    ogpu_next_heap_destroy(binding); ogpu_next_memory_destroy(heap); ogpu_next_memory_destroy(host); ogpu_next_device_destroy(device); ogpu_next_discovery_destroy(discovery);
    free(formats); free(descriptor_scratch); free(barrier_scratch); free(submit_scratch); return result;
}
