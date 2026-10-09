#include "ogpu_next.h"
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define REQUIRE(c) do { if (!(c)) { fprintf(stderr, "line %d: %s\n", __LINE__, #c); goto cleanup; } } while (0)
#define TRY(call) do { ogpu_next_status s_ = (call); if (s_ != OGPU_NEXT_OK) { fprintf(stderr, "%s: %" PRId32 "\n", #call, s_); goto cleanup; } } while (0)
#define HEADER(type, kind_) { (kind_), OGPU_NEXT_VERSION, sizeof(type), 0, NULL }
_Static_assert(sizeof(ogpu_next_record) == 24, "record ABI");
_Static_assert(sizeof(ogpu_next_query) == 48, "query ABI");
_Static_assert(offsetof(ogpu_next_query, data) == 24, "query data ABI");
_Static_assert(sizeof(ogpu_next_device_desc) == 48, "device ABI");
_Static_assert(offsetof(ogpu_next_device_desc, required_features) == 40, "features ABI");
_Static_assert(sizeof(ogpu_next_queue_request) == 12, "queue request ABI");
_Static_assert(sizeof(ogpu_next_adapter_info) == 276, "adapter ABI");
_Static_assert(sizeof(ogpu_next_queue_info) == 32, "queue ABI");
_Static_assert(sizeof(ogpu_next_memory_type_info) == 16, "memory type ABI");
_Static_assert(sizeof(ogpu_next_memory_heap_info) == 16, "heap ABI");
_Static_assert(sizeof(ogpu_next_feature_info) == 32, "feature ABI");
_Static_assert(sizeof(ogpu_next_point) == 16, "point ABI");
_Static_assert(sizeof(ogpu_next_memory_desc) == 72, "memory description ABI");
_Static_assert(sizeof(ogpu_next_span) == 24, "span ABI");
_Static_assert(sizeof(ogpu_next_requirements) == 40, "requirements ABI");
_Static_assert(sizeof(ogpu_next_mapping) == 40, "mapping ABI");
_Static_assert(sizeof(ogpu_next_memory_limits) == 32, "memory limits ABI");
_Static_assert(sizeof(ogpu_next_arena_desc) == 32, "arena ABI");
_Static_assert(sizeof(ogpu_next_recording_desc) == 40, "recording ABI");
_Static_assert(sizeof(ogpu_next_submit_desc) == 80, "submit ABI");
_Static_assert(sizeof(ogpu_next_dependency) == 104, "dependency ABI");
_Static_assert(sizeof(ogpu_next_memory_barrier) == 48, "memory barrier ABI");
_Static_assert(sizeof(ogpu_next_sync_point) == 24, "sync point ABI");
_Static_assert(offsetof(ogpu_next_submit_desc, scratch) == 64, "submit scratch ABI");
_Static_assert(offsetof(ogpu_next_dependency, scratch) == 88, "barrier scratch ABI");
_Static_assert(sizeof(ogpu_next_image_desc) == 96, "image description ABI");
_Static_assert(sizeof(ogpu_next_view_desc) == 72, "view description ABI");
_Static_assert(sizeof(ogpu_next_image_barrier) == 72, "image barrier ABI");
_Static_assert(sizeof(ogpu_next_image_copy) == 64, "image copy ABI");
_Static_assert(offsetof(ogpu_next_image_desc, concurrent_domains) == 88, "image domains ABI");
_Static_assert(sizeof(ogpu_next_descriptor_limits) == 152, "descriptor limits ABI");
_Static_assert(sizeof(ogpu_next_host_span) == 16, "host span ABI");
_Static_assert(sizeof(ogpu_next_resource_descriptor) == 40, "resource descriptor ABI");
_Static_assert(sizeof(ogpu_next_sampler_desc) == 80, "sampler ABI");
_Static_assert(sizeof(ogpu_next_heap_binding) == 72, "heap binding ABI");
_Static_assert(sizeof(ogpu_next_execution_limits) == 48, "execution limits ABI");
_Static_assert(sizeof(ogpu_next_argument_interface) == 40, "argument interface ABI");
_Static_assert(sizeof(ogpu_next_root_slot) == 16, "root slot ABI");
_Static_assert(sizeof(ogpu_next_shader_requirements) == 48, "shader requirements ABI");
_Static_assert(sizeof(ogpu_next_specialization) == 56, "specialization ABI");
_Static_assert(sizeof(ogpu_next_shader) == 48, "shader ABI");
_Static_assert(sizeof(ogpu_next_executable_desc) == 72, "executable ABI");
_Static_assert(sizeof(ogpu_next_executable_cache_desc) == 48, "executable cache ABI");
_Static_assert(sizeof(ogpu_next_launch) == 24, "launch ABI");
_Static_assert(sizeof(ogpu_next_graphics_limits) == 56, "graphics limits ABI");
_Static_assert(sizeof(ogpu_next_graphics_state) == 152, "graphics state ABI");
_Static_assert(sizeof(ogpu_next_stencil_state) == 28, "stencil state ABI");
_Static_assert(sizeof(ogpu_next_color_state) == 36, "color state ABI");
_Static_assert(sizeof(ogpu_next_attachment) == 56, "attachment ABI");
_Static_assert(sizeof(ogpu_next_render_desc) == 104, "render ABI");
_Static_assert(sizeof(ogpu_next_viewport_state) == 64, "viewport ABI");
_Static_assert(sizeof(ogpu_next_draw_desc) == 20, "draw ABI");

static ogpu_next_query query(uint32_t kind, void *data, uint32_t capacity, uint32_t size) {
    ogpu_next_query q = { HEADER(ogpu_next_query, kind), data, capacity, 0, size, 0 };
    return q;
}

static uint64_t aligned(uint64_t n, uint64_t a) { return (n + a - 1) / a * a; }
#include "foundation_heap.h"
#include "foundation_cache.h"
#include "foundation_graphics.h"

static int compute(ogpu_next_device *device, ogpu_next_memory_desc desc,
                   uint32_t domain, const char *shader_path) {
    int result = EXIT_FAILURE, pending = 0;
    FILE *file = NULL;
    void *code = NULL, *scratch = NULL;
    ogpu_next_executable *executable = NULL;
    ogpu_next_executable_cache *cache = NULL;
    ogpu_next_memory *memory = NULL;
    ogpu_next_arena *arena = NULL;
    ogpu_next_timeline *done = NULL;
    file = fopen(shader_path, "rb"); REQUIRE(file != NULL);
    REQUIRE(fseek(file, 0, SEEK_END) == 0);
    long code_size = ftell(file); REQUIRE(code_size >= 20 && code_size % 4 == 0);
    REQUIRE(fseek(file, 0, SEEK_SET) == 0);
    code = malloc((size_t)code_size); REQUIRE(code != NULL);
    REQUIRE(fread(code, 1, (size_t)code_size, file) == (size_t)code_size);
    fclose(file); file = NULL;
    ogpu_next_execution_limits limits = {0};
    ogpu_next_query q = query(OGPU_NEXT_QUERY_EXECUTION_LIMITS, &limits, 1, sizeof(limits));
    TRY(ogpu_next_device_query(device, &q));
    REQUIRE(limits.max_inline_size >= 24 && limits.argument_flags == OGPU_NEXT_ARGUMENT_SHARED_BYTES);
    ogpu_next_root_slot slots[2] = {{OGPU_NEXT_STAGE_COMPUTE, 0, 8}, {OGPU_NEXT_STAGE_COMPUTE, 8, 8}};
    ogpu_next_argument_interface abi = {HEADER(ogpu_next_argument_interface, OGPU_NEXT_ARGUMENT_INTERFACE), 24, 2, slots};
    ogpu_next_shader_requirements requirements = {HEADER(ogpu_next_shader_requirements, OGPU_NEXT_SHADER_REQUIREMENTS), 0, {64, 1, 1}, 0};
    uint32_t extra = 13;
    ogpu_next_specialization_entry entry = {7, 0, sizeof(extra)};
    ogpu_next_specialization spec = {HEADER(ogpu_next_specialization, OGPU_NEXT_SPECIALIZATION), 1, 0, &entry, {&extra, sizeof(extra)}};
    ogpu_next_shader shader = {OGPU_NEXT_STAGE_COMPUTE, OGPU_NEXT_SHADER_SPIRV,
        {code, (size_t)code_size}, "transform", &abi.header, &spec.header};
    ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc, OGPU_NEXT_EXECUTABLE_DESC),
        OGPU_NEXT_EXECUTABLE_COMPUTE, 1, &shader, NULL, 0, &requirements.header, NULL};
    slots[1].offset = 0;
    REQUIRE(ogpu_next_executable_create(device, &ed, &executable) == OGPU_NEXT_INVALID && executable == NULL);
    slots[1].offset = 8;
    requirements.local_size[0] = 0;
    REQUIRE(ogpu_next_executable_create(device, &ed, &executable) == OGPU_NEXT_INVALID && executable == NULL);
    requirements.local_size[0] = 64;
    entry.size = 8;
    REQUIRE(ogpu_next_executable_create(device, &ed, &executable) == OGPU_NEXT_INVALID && executable == NULL);
    entry.size = 4;
    ogpu_next_executable_cache_desc cache_desc = {HEADER(ogpu_next_executable_cache_desc, OGPU_NEXT_EXECUTABLE_CACHE_DESC),
        OGPU_NEXT_CACHE_NATIVE_SYNCHRONIZATION, 0, {NULL, 0}};
    TRY(ogpu_next_executable_cache_create(device, &cache_desc, &cache)); ed.cache = cache;
    TRY(ogpu_next_executable_create(device, &ed, &executable));
    REQUIRE(cache_roundtrip(device, &cache, cache_desc.synchronization) == EXIT_SUCCESS);
    ogpu_next_executable_destroy(executable); executable = NULL; ed.cache = cache;
    TRY(ogpu_next_executable_create(device, &ed, &executable));
    ogpu_next_executable_cache_destroy(cache); cache = NULL; ed.cache = NULL;
    /* Preparation does not retain artifact bytes or metadata. */
    free(code); code = NULL;
    slots[0].offset = UINT32_MAX; extra = 99;
    TRY(ogpu_next_memory_create(device, &desc, &memory));
    ogpu_next_span all = {memory, 0, desc.size};
    ogpu_next_mapping mapping = {0};
    TRY(ogpu_next_memory_map(all, &mapping));
    ogpu_next_address base = 0;
    TRY(ogpu_next_memory_address(all, &base));
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc, OGPU_NEXT_ARENA_DESC), domain, 2};
    TRY(ogpu_next_arena_create(device, &ad, &arena));
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc, OGPU_NEXT_RECORDING_DESC), OGPU_NEXT_SERIAL_REPLAY, OGPU_NEXT_PRIMARY, NULL};
    ogpu_next_encoder *encoder = NULL;
    ogpu_next_list *list = NULL;
    TRY(ogpu_next_commands_begin(arena, &rd, &encoder));
    ogpu_next_bind_executable(encoder, executable);
    ogpu_next_set_root(encoder, OGPU_NEXT_STAGE_COMPUTE, 0, base + 1024);
    ogpu_next_set_root(encoder, OGPU_NEXT_STAGE_COMPUTE, 1, base + 1040);
    uint32_t multiplier = 3;
    uint32_t control[2] = {multiplier, 0};
    ogpu_next_set_inline(encoder, OGPU_NEXT_STAGE_COMPUTE, 16, sizeof(control), control);
    ogpu_next_launch launch = {{2, 1, 1}, 0, NULL};
    ogpu_next_dispatch(encoder, &launch);
    /* Different destination root and one changed inline word; source root survives. */
    ogpu_next_set_root(encoder, OGPU_NEXT_STAGE_COMPUTE, 1, base + 1056);
    multiplier = 5;
    ogpu_next_set_inline(encoder, OGPU_NEXT_STAGE_COMPUTE, 16, 4, &multiplier);
    multiplier = 999; /* Recorded bytes, not a borrowed host argument block. */
    ogpu_next_fill_memory(encoder, (ogpu_next_span){memory, 1536, 4}, 2);
    ogpu_next_fill_memory(encoder, (ogpu_next_span){memory, 1540, 8}, 1);
    ogpu_next_dependency dep = {0};
    dep.header = (ogpu_next_record)HEADER(ogpu_next_dependency, OGPU_NEXT_DEPENDENCY);
    dep.before = OGPU_NEXT_STAGE_COPY; dep.after = OGPU_NEXT_STAGE_INDIRECT;
    dep.global_before = OGPU_NEXT_ACCESS_COPY_WRITE; dep.global_after = OGPU_NEXT_ACCESS_INDIRECT_READ;
    ogpu_next_barrier(encoder, &dep);
    ogpu_next_dispatch_indirect(encoder, (ogpu_next_span){memory, 1536, 12}, 0);
    dep.before = OGPU_NEXT_STAGE_COMPUTE; dep.after = OGPU_NEXT_STAGE_HOST;
    dep.global_before = OGPU_NEXT_ACCESS_SHADER_WRITE; dep.global_after = OGPU_NEXT_ACCESS_HOST_READ;
    ogpu_next_barrier(encoder, &dep);
    TRY(ogpu_next_commands_end(encoder, &list));
    TRY(ogpu_next_timeline_create(device, 0, &done));
    ogpu_next_host_requirements scratch_req = {0};
    TRY(ogpu_next_submit_scratch_requirements(1, 0, 1, &scratch_req));
    REQUIRE(scratch_req.alignment <= _Alignof(max_align_t));
    scratch = malloc((size_t)scratch_req.size); REQUIRE(scratch != NULL);
    ogpu_next_sync_point signal = {{done, 1}, OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {HEADER(ogpu_next_submit_desc, OGPU_NEXT_SUBMIT_DESC),
        1, 0, 1, &list, NULL, &signal, scratch, scratch_req.size};
    struct Root { uint64_t address; uint32_t count, add; };
    _Static_assert(sizeof(struct Root) == 16, "device root shader layout");
    for (uint32_t replay = 0; replay < 3; ++replay) {
        uint32_t count = 65 - replay;
        memset(mapping.data, 0xa5, (size_t)desc.size);
        for (uint32_t i = 0; i < 65; ++i) ((uint32_t *)mapping.data)[i] = i + replay;
        struct Root roots[3] = {{base, count, replay}, {base + 512, count, 7}, {base + 2048, count, 11}};
        memcpy((unsigned char *)mapping.data + 1024, roots, sizeof(roots));
        TRY(ogpu_next_memory_flush(all));
        signal.point.value = replay + 1;
        pending = 1; TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device, domain, 0), &submit));
        TRY(ogpu_next_timeline_wait(signal.point, UINT64_C(10000000000))); pending = 0;
        TRY(ogpu_next_memory_invalidate(all));
        for (uint32_t i = 0; i < 65; ++i) {
            REQUIRE(((uint32_t *)mapping.data)[128 + i] == (i < count ? (i + replay) * 3 + replay + 7 + 13 : UINT32_C(0xa5a5a5a5)));
            REQUIRE(((uint32_t *)mapping.data)[512 + i] == (i < count ? (i + replay) * 5 + replay + 11 + 13 : UINT32_C(0xa5a5a5a5)));
        }
    }
    TRY(ogpu_next_arena_reset(arena));
    TRY(ogpu_next_commands_begin(arena, &rd, &encoder));
    ogpu_next_dispatch(encoder, &launch); /* No inherited executable after reset. */
    REQUIRE(ogpu_next_commands_end(encoder, &list) == OGPU_NEXT_INVALID && list == NULL);
    TRY(ogpu_next_commands_begin(arena, &rd, &encoder));
    ogpu_next_bind_executable(encoder, executable);
    ogpu_next_set_root(encoder, OGPU_NEXT_STAGE_COMPUTE, 0, base + 1);
    REQUIRE(ogpu_next_commands_end(encoder, &list) == OGPU_NEXT_INVALID && list == NULL);
    printf("Compute passes: named entry, specialization, two device roots, partial inline updates, GPU-written indirect dimensions and changed-data replay.\n");
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr, "Pending compute work after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_executable_destroy(executable);
    ogpu_next_executable_cache_destroy(cache);
    ogpu_next_timeline_destroy(done); ogpu_next_memory_destroy(memory);
    if (file) fclose(file);
    free(code); free(scratch);
    return result;
}

static int descriptors(ogpu_next_device *device, ogpu_next_memory_desc host_desc,
                       uint32_t domain, ogpu_next_view *view) {
    int result = EXIT_FAILURE, pending = 0;
    ogpu_next_memory *data = NULL, *heaps[2] = {NULL, NULL};
    ogpu_next_arena *arena = NULL;
    ogpu_next_timeline *done = NULL, *gate = NULL;
    void *temporary = NULL, *submit_scratch = NULL;
    unsigned char *encoded = NULL;
    ogpu_next_descriptor_limits limits = {0};
    ogpu_next_query q = query(OGPU_NEXT_QUERY_DESCRIPTOR_LIMITS, &limits, 1, sizeof(limits));
    TRY(ogpu_next_device_query(device, &q));
    REQUIRE(q.count == 1 && limits.buffer_size && limits.image_size && limits.sampler_size);
    ogpu_next_host_requirements r = {0}, s = {0};
    TRY(ogpu_next_descriptor_scratch_requirements(OGPU_NEXT_HEAP_RESOURCE, 2, &r));
    TRY(ogpu_next_descriptor_scratch_requirements(OGPU_NEXT_HEAP_SAMPLER, 2, &s));
    REQUIRE(r.alignment <= _Alignof(max_align_t) && s.alignment <= _Alignof(max_align_t));
    uint64_t temporary_size = r.size > s.size ? r.size : s.size;
    temporary = malloc((size_t)temporary_size); REQUIRE(temporary != NULL);
    ogpu_next_host_span scratch = {temporary, temporary_size};
    uint64_t stride = limits.buffer_size > limits.image_size ? limits.buffer_size : limits.image_size;
    if (stride < limits.sampler_size) stride = limits.sampler_size;
    stride += 16; /* Host outputs need no GPU slot alignment. */
    encoded = malloc((size_t)(stride * 2)); REQUIRE(encoded != NULL);
    memset(encoded, 0xa5, (size_t)(stride * 2));
    ogpu_next_host_span outputs[2] = {{encoded, stride}, {encoded + stride, stride}};
    TRY(ogpu_next_memory_create(device, &host_desc, &data));
    ogpu_next_resource_descriptor resources[2] = {
        {OGPU_NEXT_DESCRIPTOR_STORAGE_BUFFER, 0, NULL, {data, 0, 256}},
        {OGPU_NEXT_DESCRIPTOR_SAMPLED_IMAGE, OGPU_NEXT_STATE_SHADER_READ, view, {NULL, 0, 0}}
    };
    resources[1].kind = UINT32_MAX;
    REQUIRE(ogpu_next_write_resource_descriptors(device, 2, resources, outputs, scratch) == OGPU_NEXT_UNSUPPORTED);
    for (uint64_t i = 0; i < stride * 2; ++i) REQUIRE(encoded[i] == 0xa5);
    resources[1].kind = OGPU_NEXT_DESCRIPTOR_SAMPLED_IMAGE;
    REQUIRE(ogpu_next_write_resource_descriptors(device, 2, resources, outputs, (ogpu_next_host_span){temporary, r.size - 1}) == OGPU_NEXT_CAPACITY);
    TRY(ogpu_next_write_resource_descriptors(device, 2, resources, outputs, scratch));
    for (uint64_t i = limits.buffer_size; i < stride; ++i) REQUIRE(encoded[i] == 0xa5);
    for (uint64_t i = limits.image_size; i < stride; ++i) REQUIRE(encoded[stride + i] == 0xa5);
    TRY(ogpu_next_write_resource_descriptors(device, 0, NULL, NULL, (ogpu_next_host_span){NULL, 0}));
    ogpu_next_sampler_desc samplers[2] = {
        {HEADER(ogpu_next_sampler_desc, OGPU_NEXT_SAMPLER_DESC), 0, 0, 0, {2, 2, 2}, 0, 0, 0, 0, 0, 0, 0, 1},
        {HEADER(ogpu_next_sampler_desc, OGPU_NEXT_SAMPLER_DESC), 1, 1, 1, {0, 0, 0}, 0, 0, 0, 0, 0, 8, 0, 1}
    };
    ogpu_next_memory_limits memory_limits = {0};
    q = query(OGPU_NEXT_QUERY_MEMORY_LIMITS, &memory_limits, 1, sizeof(memory_limits));
    TRY(ogpu_next_device_query(device, &q));
    ogpu_next_memory_type_info types[32];
    q = query(OGPU_NEXT_QUERY_MEMORY_TYPES, types, 32, sizeof(types[0]));
    TRY(ogpu_next_device_query(device, &q));
    uint32_t type_count = q.count;
    ogpu_next_heap_binding bindings[2];
    ogpu_next_mapping mappings[2];
    uint64_t slot_sizes[2];
    for (uint32_t i = 0; i < 2; ++i) {
        uint64_t alignment = i ? limits.sampler_heap_alignment : limits.resource_heap_alignment;
        uint64_t reservation_align = i ? limits.sampler_reserved_alignment : limits.resource_reserved_alignment;
        uint64_t reservation_size = i ? limits.sampler_reserved_size : limits.resource_reserved_size;
        uint64_t slot_align = i ? limits.sampler_alignment : limits.buffer_alignment;
        if (!i && slot_align < limits.image_alignment) slot_align = limits.image_alignment;
        if (reservation_align < memory_limits.cache_atom_size) reservation_align = memory_limits.cache_atom_size;
        slot_sizes[i] = aligned(stride, slot_align);
        uint64_t reservation_offset = aligned(slot_sizes[i] * 3, reservation_align);
        ogpu_next_memory_desc md = {
            HEADER(ogpu_next_memory_desc, OGPU_NEXT_MEMORY_DESC),
            reservation_offset + reservation_size, alignment, OGPU_NEXT_USAGE_DESCRIPTOR_HEAP,
            UINT32_MAX, OGPU_NEXT_MEMORY_LINEAR, NULL, 0, 0
        };
        /* Align native backing prefix as well: flushes cannot touch reserved bytes. */
        if (md.alignment < memory_limits.cache_atom_size) md.alignment = memory_limits.cache_atom_size;
        uint32_t compatible[32];
        ogpu_next_requirements req = {0};
        req.compatible_type_capacity = 32; req.compatible_types = compatible;
        TRY(ogpu_next_memory_requirements(device, &md, &req));
        for (uint32_t j = 0; j < req.compatible_type_count && md.memory_type == UINT32_MAX; ++j)
            for (uint32_t k = 0; k < type_count; ++k)
                if (types[k].id == compatible[j] && (types[k].properties & OGPU_NEXT_MEMORY_HOST_VISIBLE)) { md.memory_type = types[k].id; break; }
        REQUIRE(md.memory_type != UINT32_MAX);
        TRY(ogpu_next_memory_create(device, &md, &heaps[i]));
        TRY(ogpu_next_memory_map((ogpu_next_span){heaps[i], 0, md.size}, &mappings[i]));
        bindings[i] = (ogpu_next_heap_binding){HEADER(ogpu_next_heap_binding, OGPU_NEXT_HEAP_BINDING),
            i + 1, 0, {heaps[i], 0, md.size}, reservation_offset, reservation_size};
        if (i) {
            memset(encoded, 0xa5, (size_t)(stride * 2));
            samplers[1].header.version = UINT32_MAX;
            REQUIRE(ogpu_next_write_sampler_descriptors(device, 2, samplers, outputs, scratch) == OGPU_NEXT_UNSUPPORTED);
            for (uint64_t j = 0; j < stride * 2; ++j) REQUIRE(encoded[j] == 0xa5);
            samplers[1].header.version = OGPU_NEXT_VERSION;
            TRY(ogpu_next_write_sampler_descriptors(device, 2, samplers, outputs, scratch));
            for (uint64_t j = limits.sampler_size; j < stride; ++j)
                REQUIRE(encoded[j] == 0xa5 && encoded[stride + j] == 0xa5);
        }
        memcpy(mappings[i].data, encoded, (size_t)(i ? limits.sampler_size : limits.buffer_size));
        memcpy((unsigned char *)mappings[i].data + slot_sizes[i], encoded + stride, (size_t)(i ? limits.sampler_size : limits.image_size));
        TRY(ogpu_next_memory_flush((ogpu_next_span){heaps[i], 0, slot_sizes[i] * 2}));
    }
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc, OGPU_NEXT_ARENA_DESC), domain, 1};
    TRY(ogpu_next_arena_create(device, &ad, &arena));
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc, OGPU_NEXT_RECORDING_DESC), OGPU_NEXT_ONE_SHOT, OGPU_NEXT_PRIMARY, NULL};
    ogpu_next_encoder *encoder = NULL;
    ogpu_next_list *list = NULL;
    TRY(ogpu_next_commands_begin(arena, &rd, &encoder));
    ogpu_next_bind_heap(encoder, &bindings[0]); ogpu_next_bind_heap(encoder, &bindings[1]);
    TRY(ogpu_next_commands_end(encoder, &list));
    TRY(ogpu_next_timeline_create(device, 0, &done));
    TRY(ogpu_next_timeline_create(device, 0, &gate));
    ogpu_next_host_requirements submit_req = {0};
    TRY(ogpu_next_submit_scratch_requirements(1, 1, 1, &submit_req));
    REQUIRE(submit_req.alignment <= _Alignof(max_align_t));
    submit_scratch = malloc((size_t)submit_req.size); REQUIRE(submit_scratch != NULL);
    ogpu_next_sync_point wait = {{gate, 1}, OGPU_NEXT_STAGE_ALL}, signal = {{done, 1}, OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {HEADER(ogpu_next_submit_desc, OGPU_NEXT_SUBMIT_DESC),
        1, 1, 1, &list, &wait, &signal, submit_scratch, submit_req.size};
    pending = 1;
    TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device, domain, 0), &submit));
    REQUIRE(ogpu_next_timeline_wait(signal.point, 0) == OGPU_NEXT_TIMEOUT);
    /* Binding is not whole-heap immutability. An unused slot can be updated while
     * this list is pending; reserved bytes remain untouched until reset. */
    ogpu_next_host_span direct = {(unsigned char *)mappings[1].data + slot_sizes[1] * 2, limits.sampler_size};
    TRY(ogpu_next_write_sampler_descriptors(device, 1, samplers, &direct, scratch));
    TRY(ogpu_next_memory_flush((ogpu_next_span){heaps[1], slot_sizes[1] * 2, limits.sampler_size}));
    TRY(ogpu_next_timeline_signal_host(wait.point));
    TRY(ogpu_next_timeline_wait(signal.point, UINT64_C(10000000000))); pending = 0;
    TRY(ogpu_next_arena_reset(arena));
    /* Bad bindings poison end without issuing a malformed native command. */
    TRY(ogpu_next_commands_begin(arena, &rd, &encoder));
    bindings[0].reserved_offset = bindings[0].storage.size + 1;
    ogpu_next_bind_heap(encoder, &bindings[0]);
    REQUIRE(ogpu_next_commands_end(encoder, &list) == OGPU_NEXT_INVALID && list == NULL);
    printf("Descriptor encoding subtest passes: batched writes, caller placement, heap binding and pending unused-slot update.\n");
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr, "Pending descriptor work after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena);
    ogpu_next_timeline_destroy(gate); ogpu_next_timeline_destroy(done);
    ogpu_next_memory_destroy(heaps[0]); ogpu_next_memory_destroy(heaps[1]); ogpu_next_memory_destroy(data);
    free(temporary); free(submit_scratch); free(encoded);
    return result;
}

static int images(ogpu_next_device *device, ogpu_next_memory_desc host_desc,
                  const ogpu_next_queue_info *queues, uint32_t queue_count) {
    int result = EXIT_FAILURE, pending = 0;
    ogpu_next_image *image[7] = {NULL};
    ogpu_next_memory *multisample_memory[3] = {NULL};
    ogpu_next_memory *backing = NULL, *dedicated = NULL, *depth_memory = NULL, *host = NULL;
    ogpu_next_view *view = NULL;
    ogpu_next_arena *arena = NULL;
    ogpu_next_timeline *done = NULL;
    void *scratch = NULL, *barrier_scratch = NULL;
    uint32_t formats[] = {OGPU_NEXT_RGBA8_UNORM, OGPU_NEXT_RGBA8_SRGB};
    ogpu_next_image_desc desc = {
        HEADER(ogpu_next_image_desc, OGPU_NEXT_IMAGE_DESC),
        OGPU_NEXT_RGBA8_UNORM, OGPU_NEXT_IMAGE_2D, 3, 2, 1, {16, 8, 1},
        OGPU_NEXT_IMAGE_COPY_SRC | OGPU_NEXT_IMAGE_COPY_DST | OGPU_NEXT_IMAGE_SAMPLED,
        OGPU_NEXT_IMAGE_MUTABLE_FORMAT, formats, 2, 0, NULL
    };
    uint32_t types[32];
    ogpu_next_requirements req = {0};
    req.compatible_type_capacity = 32; req.compatible_types = types;
    TRY(ogpu_next_image_requirements(device, &desc, &req));
    REQUIRE(req.compatible_type_count > 0 && req.size > 0 && req.alignment > 0);
    /* This fixture deliberately tests subplacement, not a fallback allocator. */
    REQUIRE(!req.dedicated_required);
    uint64_t stride = (req.size + req.alignment - 1) / req.alignment * req.alignment;
    ogpu_next_memory_desc md = {
        HEADER(ogpu_next_memory_desc, OGPU_NEXT_MEMORY_DESC),
        req.alignment + stride + req.size, 1, 0, types[0], OGPU_NEXT_MEMORY_OPAQUE, NULL, 0, 0
    };
    TRY(ogpu_next_memory_create(device, &md, &backing));
    ogpu_next_span placement = {backing, req.alignment, req.size};
    TRY(ogpu_next_image_create(device, &desc, placement, &image[0]));
    placement.offset += stride;
    TRY(ogpu_next_image_create(device, &desc, placement, &image[1]));
    ogpu_next_image *bad = NULL;
    placement.size = req.size - 1;
    REQUIRE(ogpu_next_image_create(device, &desc, placement, &bad) == OGPU_NEXT_INVALID && bad == NULL);
    placement.size = req.size;
    if (req.alignment > 1) {
        placement.offset = 1;
        REQUIRE(ogpu_next_image_create(device, &desc, placement, &bad) == OGPU_NEXT_INVALID && bad == NULL);
    }
    TRY(ogpu_next_image_create_unbound(device, &desc, &image[2]));
    ogpu_next_view_desc vd = {
        HEADER(ogpu_next_view_desc, OGPU_NEXT_VIEW_DESC),
        OGPU_NEXT_RGBA8_SRGB, OGPU_NEXT_VIEW_2D, (uint32_t)OGPU_NEXT_IMAGE_SAMPLED,
        {0, 0, 0, 0}, {OGPU_NEXT_ASPECT_COLOR, 1, 1, 1, 1}
    };
    REQUIRE(ogpu_next_view_create(image[2], &vd, &view) == OGPU_NEXT_INVALID && view == NULL);
    TRY(ogpu_next_memory_create_dedicated_image(image[2], types[0], &dedicated));
    placement = (ogpu_next_span){dedicated, 0, req.size};
    TRY(ogpu_next_image_bind(image[2], placement));
    REQUIRE(ogpu_next_image_bind(image[2], placement) == OGPU_NEXT_INVALID);
    TRY(ogpu_next_view_create(image[0], &vd, &view));
    ogpu_next_image_desc depth_desc = desc;
    depth_desc.format = OGPU_NEXT_D32_FLOAT;
    depth_desc.mip_count = 1; depth_desc.layer_count = 1; depth_desc.flags = 0;
    depth_desc.view_formats = NULL; depth_desc.view_format_count = 0;
    depth_desc.usage = OGPU_NEXT_IMAGE_COPY_SRC | OGPU_NEXT_IMAGE_COPY_DST | OGPU_NEXT_IMAGE_DEPTH_STENCIL_ATTACHMENT;
    ogpu_next_requirements depth_req = {0};
    depth_req.compatible_type_capacity = 32; depth_req.compatible_types = types;
    TRY(ogpu_next_image_requirements(device, &depth_desc, &depth_req));
    TRY(ogpu_next_image_create_unbound(device, &depth_desc, &image[3]));
    TRY(ogpu_next_memory_create_dedicated_image(image[3], types[0], &depth_memory));
    TRY(ogpu_next_image_bind(image[3], (ogpu_next_span){depth_memory, 0, depth_req.size}));
    _Static_assert(sizeof(ogpu_next_image_transfer) == 88, "image transfer ABI");
    for (uint32_t i = 0; i < 3; ++i) {
        ogpu_next_image_desc ms = {HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),
            OGPU_NEXT_RGBA8_UNORM,OGPU_NEXT_IMAGE_2D,1,2,i == 2 ? 1 : 4,{8,4,1},
            OGPU_NEXT_IMAGE_COPY_SRC | OGPU_NEXT_IMAGE_COPY_DST | OGPU_NEXT_IMAGE_COLOR_ATTACHMENT,0,NULL,0,0,NULL};
        ogpu_next_requirements mr = {0}; mr.compatible_types = types; mr.compatible_type_capacity = 32;
        TRY(ogpu_next_image_requirements(device,&ms,&mr)); REQUIRE(mr.compatible_type_count > 0);
        TRY(ogpu_next_image_create_unbound(device,&ms,&image[4+i]));
        TRY(ogpu_next_memory_create_dedicated_image(image[4+i],types[0],&multisample_memory[i]));
        TRY(ogpu_next_image_bind(image[4+i],(ogpu_next_span){multisample_memory[i],0,mr.size}));
    }
    ogpu_next_view_destroy(view); view = NULL;
    vd.range.first_mip = 3;
    REQUIRE(ogpu_next_view_create(image[0], &vd, &view) == OGPU_NEXT_INVALID && view == NULL);
    vd.range.first_mip = 0; vd.range.mip_count = 3;
    vd.range.first_layer = 0; vd.range.layer_count = 2; vd.dimension = OGPU_NEXT_VIEW_2D_ARRAY;
    TRY(ogpu_next_view_create(image[0], &vd, &view));
    TRY(ogpu_next_memory_create(device, &host_desc, &host));
    ogpu_next_span all = {host, 0, host_desc.size};
    ogpu_next_mapping map = {0};
    TRY(ogpu_next_memory_map(all, &map));
    for (uint32_t i = 0; i < all.size; ++i) ((unsigned char *)map.data)[i] = 0xa5;
    /* Non-tight rows and array-layer slices, with untouched guard bytes. */
    for (uint32_t layer = 0; layer < 2; ++layer)
        for (uint32_t y = 0; y < 2; ++y)
            for (uint32_t x = 0; x < 3; ++x)
                ((uint32_t *)map.data)[(layer * 80 + y * 20) / 4 + x] = UINT32_C(0xff000000) | (layer << 16) | (y << 8) | x;
    TRY(ogpu_next_memory_flush(all));
    uint32_t domain = UINT32_MAX;
    for (uint32_t i = 0; i < queue_count; ++i)
        if (queues[i].count && (queues[i].flags & OGPU_NEXT_QUEUE_GRAPHICS)) { domain = queues[i].domain; break; }
    REQUIRE(domain != UINT32_MAX);
    REQUIRE(descriptors(device, host_desc, domain, view) == EXIT_SUCCESS);
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc, OGPU_NEXT_ARENA_DESC), domain, 1};
    TRY(ogpu_next_arena_create(device, &ad, &arena));
    TRY(ogpu_next_timeline_create(device, 0, &done));
    ogpu_next_host_requirements hr = {0}, br = {0};
    TRY(ogpu_next_submit_scratch_requirements(1, 0, 1, &hr));
    TRY(ogpu_next_barrier_scratch_requirements(0, 4, &br));
    REQUIRE(hr.alignment <= _Alignof(max_align_t) && br.alignment <= _Alignof(max_align_t));
    scratch = malloc((size_t)hr.size); barrier_scratch = malloc((size_t)br.size);
    REQUIRE(scratch && barrier_scratch);
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc, OGPU_NEXT_RECORDING_DESC), OGPU_NEXT_ONE_SHOT, OGPU_NEXT_PRIMARY, NULL};
    ogpu_next_encoder *encoder = NULL;
    ogpu_next_list *list = NULL;
    TRY(ogpu_next_commands_begin(arena, &rd, &encoder));
    ogpu_next_subresources full = {OGPU_NEXT_ASPECT_COLOR, 0, 3, 0, 2};
    ogpu_next_image_barrier barriers[4];
    for (uint32_t i = 0; i < 3; ++i) barriers[i] = (ogpu_next_image_barrier){
        image[i], full, 0, OGPU_NEXT_ACCESS_COPY_WRITE,
        OGPU_NEXT_STATE_UNDEFINED, OGPU_NEXT_STATE_COPY_DST, OGPU_NEXT_DOMAIN_IGNORED, OGPU_NEXT_DOMAIN_IGNORED, 0
    };
    ogpu_next_subresources depth_range = {OGPU_NEXT_ASPECT_DEPTH, 0, 1, 0, 1};
    barriers[3] = (ogpu_next_image_barrier){image[3], depth_range, 0, OGPU_NEXT_ACCESS_COPY_WRITE,
        OGPU_NEXT_STATE_UNDEFINED, OGPU_NEXT_STATE_COPY_DST, OGPU_NEXT_DOMAIN_IGNORED, OGPU_NEXT_DOMAIN_IGNORED, 0};
    ogpu_next_dependency dep = {
        HEADER(ogpu_next_dependency, OGPU_NEXT_DEPENDENCY), 0, OGPU_NEXT_STAGE_COPY, 0, 0,
        0, 4, 0, NULL, barriers, barrier_scratch, br.size
    };
    ogpu_next_barrier(encoder, &dep);
    for (uint32_t i = 0; i < 3; ++i) {
        ogpu_next_clear_value clear = {.f32 = {i == 0 ? 1.f : 0.f, i == 2 ? 1.f : 0.f, i == 1 ? 1.f : 0.f, 1.f}};
        ogpu_next_clear_image(encoder, image[i], OGPU_NEXT_STATE_COPY_DST, &full, &clear);
        barriers[i].before = OGPU_NEXT_ACCESS_COPY_WRITE;
        barriers[i].old_state = OGPU_NEXT_STATE_COPY_DST;
    }
    ogpu_next_clear_value depth_clear = {.depth_stencil = {0.25f, 0}};
    ogpu_next_clear_image(encoder, image[3], OGPU_NEXT_STATE_COPY_DST, &depth_range, &depth_clear);
    barriers[3].before = OGPU_NEXT_ACCESS_COPY_WRITE;
    barriers[3].old_state = OGPU_NEXT_STATE_COPY_DST;
    dep.before = OGPU_NEXT_STAGE_COPY;
    ogpu_next_barrier(encoder, &dep);
    ogpu_next_image_copy copy = {{OGPU_NEXT_ASPECT_COLOR, 1, 0, 2, {2, 1, 0}, {3, 2, 1}}, 20, 80, OGPU_NEXT_STATE_COPY_DST, 0};
    ogpu_next_copy_to_image(encoder, image[0], (ogpu_next_span){host, 0, 112}, &copy);
    for (uint32_t i = 0; i < 4; ++i) { barriers[i].after = OGPU_NEXT_ACCESS_COPY_READ; barriers[i].new_state = OGPU_NEXT_STATE_COPY_SRC; }
    ogpu_next_barrier(encoder, &dep);
    copy = (ogpu_next_image_copy){{OGPU_NEXT_ASPECT_COLOR, 1, 0, 2, {0, 0, 0}, {8, 4, 1}}, 40, 200, OGPU_NEXT_STATE_COPY_SRC, 0};
    ogpu_next_copy_from_image(encoder, (ogpu_next_span){host, 1024, 352}, image[0], &copy);
    copy = (ogpu_next_image_copy){{OGPU_NEXT_ASPECT_COLOR, 0, 0, 1, {0, 0, 0}, {16, 8, 1}}, 0, 0, OGPU_NEXT_STATE_COPY_SRC, 0};
    ogpu_next_copy_from_image(encoder, (ogpu_next_span){host, 2048, 512}, image[1], &copy);
    ogpu_next_copy_from_image(encoder, (ogpu_next_span){host, 3072, 512}, image[2], &copy);
    copy.region.aspect = OGPU_NEXT_ASPECT_DEPTH;
    ogpu_next_copy_from_image(encoder, (ogpu_next_span){host, 3584, 512}, image[3], &copy);
    dep.image_count = 0; dep.images = NULL;
    dep.after = OGPU_NEXT_STAGE_HOST;
    dep.global_before = OGPU_NEXT_ACCESS_COPY_WRITE; dep.global_after = OGPU_NEXT_ACCESS_HOST_READ;
    ogpu_next_barrier(encoder, &dep);
    TRY(ogpu_next_commands_end(encoder, &list));
    ogpu_next_sync_point signal = {{done, 1}, OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {HEADER(ogpu_next_submit_desc, OGPU_NEXT_SUBMIT_DESC), 1, 0, 1, &list, NULL, &signal, scratch, hr.size};
    pending = 1; TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device, domain, 0), &submit));
    TRY(ogpu_next_timeline_wait(signal.point, UINT64_C(10000000000))); pending = 0;
    TRY(ogpu_next_memory_invalidate(all));
    for (uint32_t layer = 0; layer < 2; ++layer)
        for (uint32_t y = 0; y < 4; ++y)
            for (uint32_t x = 0; x < 8; ++x) {
                uint32_t expected = UINT32_C(0xff0000ff);
                if (x >= 2 && x < 5 && y >= 1 && y < 3)
                    expected = UINT32_C(0xff000000) | (layer << 16) | ((y - 1) << 8) | (x - 2);
                REQUIRE(((uint32_t *)map.data)[(1024 + layer * 200 + y * 40) / 4 + x] == expected);
            }
    for (uint32_t i = 0; i < 128; ++i) {
        REQUIRE(((uint32_t *)map.data)[2048 / 4 + i] == UINT32_C(0xffff0000));
        REQUIRE(((uint32_t *)map.data)[3072 / 4 + i] == UINT32_C(0xff00ff00));
        REQUIRE(((float *)map.data)[3584 / 4 + i] == 0.25f);
    }
    /* All output row/slice padding and the unused allocations' host guards stay intact. */
    for (uint32_t i = 112; i < all.size; ++i) {
        int written = (i >= 2048 && i < 2560) || (i >= 3072 && i < 4096);
        for (uint32_t layer = 0; layer < 2; ++layer)
            for (uint32_t y = 0; y < 4; ++y) {
                uint32_t start = 1024 + layer * 200 + y * 40;
                written |= i >= start && i < start + 32;
            }
        if (!written) REQUIRE(((unsigned char *)map.data)[i] == 0xa5);
    }
    TRY(ogpu_next_arena_reset(arena));
    TRY(ogpu_next_commands_begin(arena, &rd, &encoder));
    /* Native image copies: pitched upload -> another mip/layer region, then a
     * disjoint same-image copy in GENERAL. Multisample copy then partial resolve. */
    ogpu_next_subresources ms_range = {OGPU_NEXT_ASPECT_COLOR,0,1,0,2};
    for (uint32_t i = 0; i < 3; ++i) barriers[i] = (ogpu_next_image_barrier){image[4+i],ms_range,0,OGPU_NEXT_ACCESS_COPY_WRITE,
        OGPU_NEXT_STATE_UNDEFINED,OGPU_NEXT_STATE_COPY_DST,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,0};
    barriers[3] = (ogpu_next_image_barrier){image[1],full,OGPU_NEXT_ACCESS_COPY_READ,OGPU_NEXT_ACCESS_COPY_WRITE,
        OGPU_NEXT_STATE_COPY_SRC,OGPU_NEXT_STATE_GENERAL,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,0};
    dep.before = dep.after = OGPU_NEXT_STAGE_COPY; dep.global_before = dep.global_after = 0;
    dep.image_count = 4; dep.images = barriers;
    ogpu_next_barrier(encoder,&dep);
    ogpu_next_clear_value red = {.f32={1,0,0,1}}, black = {.f32={0,0,0,1}};
    ogpu_next_clear_image(encoder,image[4],OGPU_NEXT_STATE_COPY_DST,&ms_range,&red);
    ogpu_next_clear_image(encoder,image[6],OGPU_NEXT_STATE_COPY_DST,&ms_range,&black);
    ogpu_next_image_transfer transfer = {
        {OGPU_NEXT_ASPECT_COLOR,1,0,2,{2,1,0},{3,2,1}}, {OGPU_NEXT_ASPECT_COLOR,1,0,2,{4,0,0},{3,2,1}},
        OGPU_NEXT_STATE_COPY_SRC,OGPU_NEXT_STATE_GENERAL};
    ogpu_next_copy_image(encoder,image[1],image[0],&transfer);
    barriers[0].before = OGPU_NEXT_ACCESS_COPY_WRITE; barriers[0].after = OGPU_NEXT_ACCESS_COPY_READ;
    barriers[0].old_state = OGPU_NEXT_STATE_COPY_DST; barriers[0].new_state = OGPU_NEXT_STATE_COPY_SRC;
    barriers[2].before = OGPU_NEXT_ACCESS_COPY_WRITE; barriers[2].old_state = OGPU_NEXT_STATE_COPY_DST;
    barriers[3].before = OGPU_NEXT_ACCESS_COPY_WRITE; barriers[3].after = OGPU_NEXT_ACCESS_COPY_READ | OGPU_NEXT_ACCESS_COPY_WRITE;
    barriers[3].old_state = OGPU_NEXT_STATE_GENERAL;
    ogpu_next_barrier(encoder,&dep);
    transfer.source = transfer.destination; transfer.destination.offset = (ogpu_next_offset){0,2,0}; transfer.source_state = OGPU_NEXT_STATE_GENERAL;
    ogpu_next_copy_image(encoder,image[1],image[1],&transfer);
    transfer = (ogpu_next_image_transfer){{OGPU_NEXT_ASPECT_COLOR,0,0,2,{0,0,0},{8,4,1}},
        {OGPU_NEXT_ASPECT_COLOR,0,0,2,{0,0,0},{8,4,1}},OGPU_NEXT_STATE_COPY_SRC,OGPU_NEXT_STATE_COPY_DST};
    ogpu_next_copy_image(encoder,image[5],image[4],&transfer);
    barriers[0] = (ogpu_next_image_barrier){image[5],ms_range,OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_COPY_READ,
        OGPU_NEXT_STATE_COPY_DST,OGPU_NEXT_STATE_COPY_SRC,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,0};
    dep.image_count = 1; ogpu_next_barrier(encoder,&dep);
    transfer.source.offset = (ogpu_next_offset){1,1,0}; transfer.destination.offset = (ogpu_next_offset){2,1,0};
    transfer.source.extent = transfer.destination.extent = (ogpu_next_extent){3,2,1};
    ogpu_next_resolve_image(encoder,image[6],image[5],&transfer,OGPU_NEXT_RESOLVE_NATIVE_COLOR);
    barriers[0] = (ogpu_next_image_barrier){image[6],ms_range,OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_COPY_READ,
        OGPU_NEXT_STATE_COPY_DST,OGPU_NEXT_STATE_COPY_SRC,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,0};
    barriers[1] = (ogpu_next_image_barrier){image[1],full,OGPU_NEXT_ACCESS_COPY_READ | OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_COPY_READ,
        OGPU_NEXT_STATE_GENERAL,OGPU_NEXT_STATE_COPY_SRC,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,0};
    dep.image_count = 2; ogpu_next_barrier(encoder,&dep);
    copy = (ogpu_next_image_copy){{OGPU_NEXT_ASPECT_COLOR,1,0,2,{0,0,0},{8,4,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};
    ogpu_next_copy_from_image(encoder,(ogpu_next_span){host,512,256},image[1],&copy);
    copy.region.mip = 0;
    ogpu_next_copy_from_image(encoder,(ogpu_next_span){host,1536,256},image[6],&copy);
    dep.image_count = 0; dep.images = NULL; dep.after = OGPU_NEXT_STAGE_HOST;
    dep.global_before = OGPU_NEXT_ACCESS_COPY_WRITE; dep.global_after = OGPU_NEXT_ACCESS_HOST_READ;
    ogpu_next_barrier(encoder,&dep);
    TRY(ogpu_next_commands_end(encoder,&list)); signal.point.value = 2;
    pending = 1; TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),&submit));
    TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0;
    TRY(ogpu_next_memory_invalidate(all));
    for (uint32_t layer = 0; layer < 2; ++layer) for (uint32_t y = 0; y < 4; ++y) for (uint32_t x = 0; x < 8; ++x) {
        uint32_t expected = UINT32_C(0xffff0000);
        if (x >= 4 && x < 7 && y < 2) expected = UINT32_C(0xff000000) | (layer << 16) | (y << 8) | (x-4);
        if (x < 3 && y >= 2) expected = UINT32_C(0xff000000) | (layer << 16) | ((y-2) << 8) | x;
        REQUIRE(((uint32_t *)map.data)[128+layer*32+y*8+x] == expected);
        expected = (x >= 2 && x < 5 && y >= 1 && y < 3) ? UINT32_C(0xff0000ff) : UINT32_C(0xff000000);
        REQUIRE(((uint32_t *)map.data)[384+layer*32+y*8+x] == expected);
    }
    for (uint32_t i = 768; i < 1024; ++i) REQUIRE(((unsigned char *)map.data)[i] == 0xa5);
    for (uint32_t i = 1792; i < 2048; ++i) REQUIRE(((unsigned char *)map.data)[i] == 0xa5);
    TRY(ogpu_next_arena_reset(arena));
    TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    ogpu_next_copy_image(encoder,image[6],image[5],&transfer); /* Sample mismatch. */
    REQUIRE(ogpu_next_commands_end(encoder,&list) == OGPU_NEXT_INVALID && list == NULL);
    for (uint32_t bad_transfer = 0; bad_transfer < 5; ++bad_transfer) {
        TRY(ogpu_next_arena_reset(arena));
        TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
        ogpu_next_image_transfer bad_t = transfer;
        if (bad_transfer == 0) ogpu_next_resolve_image(encoder,image[4],image[5],&bad_t,OGPU_NEXT_RESOLVE_NATIVE_COLOR);
        if (bad_transfer == 1) ogpu_next_resolve_image(encoder,image[6],image[5],&bad_t,2);
        if (bad_transfer == 2) { bad_t.source.offset.x = INT32_MAX; ogpu_next_resolve_image(encoder,image[6],image[5],&bad_t,OGPU_NEXT_RESOLVE_NATIVE_COLOR); }
        if (bad_transfer == 3) { bad_t.destination.extent.x = 0; ogpu_next_resolve_image(encoder,image[6],image[5],&bad_t,OGPU_NEXT_RESOLVE_NATIVE_COLOR); }
        if (bad_transfer == 4) { bad_t.destination_state = OGPU_NEXT_STATE_UNDEFINED; ogpu_next_resolve_image(encoder,image[6],image[5],&bad_t,OGPU_NEXT_RESOLVE_NATIVE_COLOR); }
        REQUIRE(ogpu_next_commands_end(encoder,&list) == (bad_transfer == 1 ? OGPU_NEXT_UNSUPPORTED : OGPU_NEXT_INVALID) && list == NULL);
    }
    TRY(ogpu_next_arena_reset(arena));
    TRY(ogpu_next_commands_begin(arena, &rd, &encoder));
    copy.region.aspect = OGPU_NEXT_ASPECT_COLOR; copy.row_pitch = 1;
    ogpu_next_copy_from_image(encoder, (ogpu_next_span){host, 1024, 512}, image[0], &copy);
    REQUIRE(ogpu_next_commands_end(encoder, &list) == OGPU_NEXT_INVALID && list == NULL);
    TRY(ogpu_next_arena_reset(arena));
    TRY(ogpu_next_commands_begin(arena, &rd, &encoder));
    barriers[0].new_state = OGPU_NEXT_STATE_UNDEFINED;
    dep.image_count = 1; dep.images = barriers;
    dep.after = OGPU_NEXT_STAGE_COPY;
    dep.global_before = 0; dep.global_after = 0;
    ogpu_next_barrier(encoder, &dep);
    REQUIRE(ogpu_next_commands_end(encoder, &list) == OGPU_NEXT_INVALID && list == NULL);
    printf("Image path passes: placement/views, explicit transitions, pitched transfers, mip/layer and disjoint same-image copies, multisample copy and partial color resolve.\n");
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr, "Pending image work after failure; avoiding unsafe teardown.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_timeline_destroy(done);
    ogpu_next_view_destroy(view);
    for (uint32_t i = 0; i < 7; ++i) ogpu_next_image_destroy(image[i]);
    for (uint32_t i = 0; i < 3; ++i) ogpu_next_memory_destroy(multisample_memory[i]);
    ogpu_next_memory_destroy(backing); ogpu_next_memory_destroy(dedicated); ogpu_next_memory_destroy(host);
    ogpu_next_memory_destroy(depth_memory);
    free(scratch); free(barrier_scratch);
    return result;
}

static int commands(ogpu_next_device *device, ogpu_next_memory_desc desc,
                    const ogpu_next_queue_info *queues, uint32_t queue_count,
                    const ogpu_next_memory_type_info *types, uint32_t type_count,
                    const uint32_t *compatible, uint32_t compatible_count) {
    int result = EXIT_FAILURE, pending = 0;
    ogpu_next_arena *arena = NULL;
    ogpu_next_arena *other_arena = NULL;
    ogpu_next_memory *upload = NULL, *work = NULL, *readback = NULL;
    ogpu_next_memory *other_readback = NULL;
    ogpu_next_timeline *done = NULL, *gate = NULL;
    void *scratch = NULL, *barrier_scratch = NULL;
    ogpu_next_host_requirements req = {0}, barrier_req = {0};
    uint32_t domain = UINT32_MAX;
    uint32_t host_type = desc.memory_type;
    for (uint32_t i = 0; i < queue_count; ++i) {
        if (queues[i].count && (queues[i].flags & (OGPU_NEXT_QUEUE_GRAPHICS | OGPU_NEXT_QUEUE_COMPUTE | OGPU_NEXT_QUEUE_TRANSFER))) {
            domain = queues[i].domain; break;
        }
    }
    REQUIRE(domain != UINT32_MAX);
    ogpu_next_queue *queue = ogpu_next_device_queue(device, domain, 0);
    REQUIRE(queue != NULL);
    TRY(ogpu_next_memory_create(device, &desc, &upload));
    TRY(ogpu_next_memory_create(device, &desc, &readback));
    /* Choose a compatible local working allocation, not a runtime placement hint. */
    desc.memory_type = UINT32_MAX;
    for (uint32_t i = 0; i < compatible_count; ++i)
        for (uint32_t j = 0; j < type_count; ++j)
            if (types[j].id == compatible[i] && (types[j].properties & OGPU_NEXT_MEMORY_LOCAL))
                desc.memory_type = types[j].id;
    REQUIRE(desc.memory_type != UINT32_MAX);
    TRY(ogpu_next_memory_create(device, &desc, &work));
    ogpu_next_span u = {upload, 0, 4096}, w = {work, 0, 4096}, r = {readback, 0, 4096};
    ogpu_next_mapping um = {0}, rm = {0};
    TRY(ogpu_next_memory_map(u, &um)); TRY(ogpu_next_memory_map(r, &rm));
    TRY(ogpu_next_timeline_create(device, 0, &done));
    TRY(ogpu_next_timeline_create(device, 0, &gate));
    ogpu_next_arena_desc ad = { HEADER(ogpu_next_arena_desc, OGPU_NEXT_ARENA_DESC), domain, 1 };
    TRY(ogpu_next_arena_create(device, &ad, &arena));
    TRY(ogpu_next_submit_scratch_requirements(1, 1, 1, &req));
    REQUIRE(req.alignment <= _Alignof(max_align_t));
    scratch = malloc((size_t)req.size); REQUIRE(scratch != NULL);
    TRY(ogpu_next_barrier_scratch_requirements(1, 0, &barrier_req));
    REQUIRE(barrier_req.alignment <= _Alignof(max_align_t));
    barrier_scratch = malloc((size_t)barrier_req.size); REQUIRE(barrier_scratch != NULL);
    ogpu_next_recording_desc rd = { HEADER(ogpu_next_recording_desc, OGPU_NEXT_RECORDING_DESC), OGPU_NEXT_SERIAL_REPLAY, OGPU_NEXT_PRIMARY, NULL };
    ogpu_next_encoder *encoder = NULL;
    ogpu_next_list *list = NULL;
    TRY(ogpu_next_commands_begin(arena, &rd, &encoder));
    ogpu_next_copy_memory(encoder, w, u);
    ogpu_next_memory_barrier range = {w, OGPU_NEXT_ACCESS_COPY_WRITE, OGPU_NEXT_ACCESS_COPY_WRITE, OGPU_NEXT_DOMAIN_IGNORED, OGPU_NEXT_DOMAIN_IGNORED};
    ogpu_next_dependency dep = {
        HEADER(ogpu_next_dependency, OGPU_NEXT_DEPENDENCY),
        OGPU_NEXT_STAGE_COPY, OGPU_NEXT_STAGE_COPY, 0, 0,
        1, 0, 0, &range, NULL, barrier_scratch, barrier_req.size
    };
    ogpu_next_barrier(encoder, &dep);
    ogpu_next_fill_memory(encoder, (ogpu_next_span){work, 1024, 1024}, UINT32_C(0x12345678));
    range.after = OGPU_NEXT_ACCESS_COPY_READ;
    ogpu_next_barrier(encoder, &dep);
    ogpu_next_copy_memory(encoder, r, w);
    dep.memory_count = 0; dep.memory = NULL;
    dep.after = OGPU_NEXT_STAGE_HOST;
    dep.global_before = OGPU_NEXT_ACCESS_COPY_WRITE;
    dep.global_after = OGPU_NEXT_ACCESS_HOST_READ;
    ogpu_next_barrier(encoder, &dep);
    TRY(ogpu_next_commands_end(encoder, &list));
    encoder = NULL;
    REQUIRE(ogpu_next_commands_begin(arena, &rd, &encoder) == OGPU_NEXT_CAPACITY && encoder == NULL);
    /* Reserve growth preserves the already recorded list handle. */
    TRY(ogpu_next_arena_reserve(arena, 3));
    ogpu_next_sync_point signal = {{done, 1}, OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {
        HEADER(ogpu_next_submit_desc, OGPU_NEXT_SUBMIT_DESC),
        1, 0, 1, &list, NULL, &signal, scratch, req.size
    };
    submit.scratch_size = 0;
    REQUIRE(ogpu_next_queue_submit(queue, &submit) == OGPU_NEXT_CAPACITY);
    submit.scratch_size = req.size;
    for (uint32_t replay = 0; replay < 3; ++replay) {
        for (uint32_t i = 0; i < 1024; ++i) ((uint32_t *)um.data)[i] = i ^ (replay * 12345);
        TRY(ogpu_next_memory_flush(u));
        signal.point.value = replay + 1;
        pending = 1; TRY(ogpu_next_queue_submit(queue, &submit));
        TRY(ogpu_next_timeline_wait(signal.point, UINT64_C(10000000000))); pending = 0;
        TRY(ogpu_next_memory_invalidate(r));
        for (uint32_t i = 0; i < 1024; ++i)
            REQUIRE(((uint32_t *)rm.data)[i] == (i >= 256 && i < 512 ? UINT32_C(0x12345678) : i ^ (replay * 12345)));
    }
    /* A malformed command poisons end; later valid commands cannot rescue it. */
    TRY(ogpu_next_commands_begin(arena, &rd, &encoder));
    ogpu_next_fill_memory(encoder, (ogpu_next_span){work, 1, 4}, 0);
    ogpu_next_fill_memory(encoder, w, 0);
    list = (ogpu_next_list *)(uintptr_t)1;
    REQUIRE(ogpu_next_commands_end(encoder, &list) == OGPU_NEXT_INVALID && list == NULL);
    TRY(ogpu_next_commands_begin(arena, &rd, &encoder));
    ogpu_next_commands_cancel(encoder);
    REQUIRE(ogpu_next_commands_begin(arena, &rd, &encoder) == OGPU_NEXT_CAPACITY);
    TRY(ogpu_next_arena_reset(arena));
    rd.replay_mode = OGPU_NEXT_ONE_SHOT;
    TRY(ogpu_next_commands_begin(arena, &rd, &encoder));
    TRY(ogpu_next_commands_end(encoder, &list));
    signal.point.value = 4;
    pending = 1; TRY(ogpu_next_queue_submit(queue, &submit));
    REQUIRE(ogpu_next_queue_submit(queue, &submit) == OGPU_NEXT_INVALID);
    TRY(ogpu_next_timeline_wait(signal.point, UINT64_C(10000000000))); pending = 0;
    /* Two truly pending executions of one simultaneous list, with a host gate.
     * Empty work avoids introducing an unrelated resource hazard between them. */
    rd.replay_mode = OGPU_NEXT_SIMULTANEOUS_REPLAY;
    TRY(ogpu_next_commands_begin(arena, &rd, &encoder));
    TRY(ogpu_next_commands_end(encoder, &list));
    ogpu_next_sync_point wait = {{gate, 1}, OGPU_NEXT_STAGE_ALL};
    submit.wait_count = 1; submit.waits = &wait;
    signal.point.value = 5;
    pending = 1; TRY(ogpu_next_queue_submit(queue, &submit));
    signal.point.value = 6;
    TRY(ogpu_next_queue_submit(queue, &submit));
    REQUIRE(ogpu_next_timeline_wait(signal.point, 0) == OGPU_NEXT_TIMEOUT);
    TRY(ogpu_next_timeline_signal_host(wait.point));
    TRY(ogpu_next_timeline_wait(signal.point, UINT64_C(10000000000))); pending = 0;
    /* An actual ownership transfer when a second created family is available.
     * Release/acquire, timeline edge and final completion are all explicit. */
    uint32_t other_domain = UINT32_MAX;
    for (uint32_t i = 0; i < queue_count; ++i)
        if (queues[i].domain != domain && queues[i].count &&
            (queues[i].flags & (OGPU_NEXT_QUEUE_GRAPHICS | OGPU_NEXT_QUEUE_COMPUTE | OGPU_NEXT_QUEUE_TRANSFER))) {
            other_domain = queues[i].domain; break;
        }
    if (other_domain != UINT32_MAX) {
        TRY(ogpu_next_arena_reset(arena));
        rd.replay_mode = OGPU_NEXT_ONE_SHOT;
        TRY(ogpu_next_commands_begin(arena, &rd, &encoder));
        range = (ogpu_next_memory_barrier){w, OGPU_NEXT_ACCESS_COPY_READ | OGPU_NEXT_ACCESS_COPY_WRITE, 0, domain, other_domain};
        dep.before = OGPU_NEXT_STAGE_COPY; dep.after = 0;
        dep.global_before = 0; dep.global_after = 0;
        dep.memory_count = 1; dep.memory = &range;
        ogpu_next_barrier(encoder, &dep);
        TRY(ogpu_next_commands_end(encoder, &list));
        submit.wait_count = 0; submit.waits = NULL; signal.point.value = 7;
        pending = 1; TRY(ogpu_next_queue_submit(queue, &submit));
        ad.domain = other_domain;
        TRY(ogpu_next_arena_create(device, &ad, &other_arena));
        desc.memory_type = host_type;
        TRY(ogpu_next_memory_create(device, &desc, &other_readback));
        ogpu_next_span other_span = {other_readback, 0, 4096};
        ogpu_next_mapping other_map = {0};
        TRY(ogpu_next_memory_map(other_span, &other_map));
        TRY(ogpu_next_commands_begin(other_arena, &rd, &encoder));
        dep.before = 0; dep.after = OGPU_NEXT_STAGE_COPY;
        range.before = 0; range.after = OGPU_NEXT_ACCESS_COPY_READ;
        ogpu_next_barrier(encoder, &dep);
        ogpu_next_copy_memory(encoder, other_span, w);
        dep.memory_count = 0; dep.memory = NULL;
        dep.before = OGPU_NEXT_STAGE_COPY; dep.after = OGPU_NEXT_STAGE_HOST;
        dep.global_before = OGPU_NEXT_ACCESS_COPY_WRITE; dep.global_after = OGPU_NEXT_ACCESS_HOST_READ;
        ogpu_next_barrier(encoder, &dep);
        TRY(ogpu_next_commands_end(encoder, &list));
        wait = (ogpu_next_sync_point){{done, 7}, OGPU_NEXT_STAGE_ALL};
        submit.wait_count = 1; submit.waits = &wait; signal.point.value = 8;
        TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device, other_domain, 0), &submit));
        TRY(ogpu_next_timeline_wait(signal.point, UINT64_C(10000000000))); pending = 0;
        TRY(ogpu_next_memory_invalidate(other_span));
        for (uint32_t i = 0; i < 1024; ++i)
            REQUIRE(((uint32_t *)other_map.data)[i] == ((uint32_t *)rm.data)[i]);
        printf("Cross-family ownership transfer passes: domain %u -> %u.\n", domain, other_domain);
    } else {
        printf("Cross-family ownership transfer not exercised: only one transfer-capable family.\n");
    }
    TRY(ogpu_next_arena_trim(arena, 0));
    REQUIRE(ogpu_next_commands_begin(arena, &rd, &encoder) == OGPU_NEXT_CAPACITY);
    TRY(ogpu_next_arena_reserve(arena, 1));
    TRY(ogpu_next_commands_begin(arena, &rd, &encoder));
    ogpu_next_commands_cancel(encoder);
    printf("Command path passes: local backing, copy/fill, ranged/global barriers, replay, pending gate, reserve/reset/trim.\n");
    result = EXIT_SUCCESS;
cleanup:
    /* Failure must not destroy resources with unknown pending use. Let process
     * teardown reclaim them instead of adding a hidden infinite cleanup wait. */
    if (pending) { fprintf(stderr, "Pending GPU work after failure; exiting without unsafe teardown.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena);
    ogpu_next_arena_destroy(other_arena);
    ogpu_next_timeline_destroy(gate); ogpu_next_timeline_destroy(done);
    ogpu_next_memory_destroy(upload); ogpu_next_memory_destroy(work); ogpu_next_memory_destroy(readback);
    ogpu_next_memory_destroy(other_readback);
    free(scratch); free(barrier_scratch);
    return result;
}
int main(int argc, char **argv) {
    int result = EXIT_FAILURE;
    ogpu_next_discovery *discovery = NULL;
    ogpu_next_device *device = NULL;
    ogpu_next_timeline *timeline = NULL;
    ogpu_next_memory *memory = NULL;
    ogpu_next_memory_type_info *types = NULL;
    uint32_t *compatible = NULL;
    ogpu_next_queue_info *queues = NULL;
    ogpu_next_queue_request *requests = NULL;
    ogpu_next_adapter_info info = {0};
    uint32_t queue_count = 0;
    TRY(ogpu_next_discovery_create(OGPU_NEXT_VERSION, &discovery));
    for (uint32_t i = 0; i < ogpu_next_discovery_count(discovery); ++i) {
        const ogpu_next_adapter *adapter = ogpu_next_discovery_adapter(discovery, i);
        ogpu_next_feature_info features = {0};
        ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES, &features, 1, sizeof(features));
        TRY(ogpu_next_adapter_query(adapter, &q));
        REQUIRE(features.enabled == 0 && features.device_scope == 0);
        if (!features.baseline_supported || !(features.available & OGPU_NEXT_FEATURE_RASTER)) continue;
        q = query(OGPU_NEXT_QUERY_INFO, &info, 1, sizeof(info));
        TRY(ogpu_next_adapter_query(adapter, &q));
        q = query(OGPU_NEXT_QUERY_QUEUES, NULL, 0, sizeof(*queues));
        TRY(ogpu_next_adapter_query(adapter, &q));
        REQUIRE(q.count > 0);
        queues = calloc(q.count, sizeof(*queues));
        requests = calloc(q.count, sizeof(*requests));
        REQUIRE(queues && requests);
        q.data = queues; q.capacity = q.count;
        TRY(ogpu_next_adapter_query(adapter, &q));
        uint32_t count = 0;
        queue_count = q.count;
        for (uint32_t j = 0; j < q.count; ++j) {
            if (!queues[j].count) continue;
            requests[count++] = (ogpu_next_queue_request){queues[j].domain, 1, 0.5f};
        }
        /* One real queue from every available domain, no implicit substitution. */
        ogpu_next_device_desc desc = { HEADER(ogpu_next_device_desc, OGPU_NEXT_DEVICE_DESC), requests, count, 0,
            OGPU_NEXT_FEATURE_RASTER | (features.available & OGPU_NEXT_FEATURE_CACHE_CONTROL) };
        TRY(ogpu_next_device_create(adapter, &desc, &device));
        for (uint32_t j = 0; j < count; ++j) {
            REQUIRE(ogpu_next_device_queue(device, requests[j].domain, 0) != NULL);
            REQUIRE(ogpu_next_device_queue(device, requests[j].domain, 1) == NULL);
        }
        break;
    }
    REQUIRE(device != NULL); /* No suitable adapter is a failure, not a passing skip. */
    ogpu_next_discovery_destroy(discovery); discovery = NULL;
    ogpu_next_feature_info features = {0};
    ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES, &features, 1, sizeof(features));
    TRY(ogpu_next_device_query(device, &q));
    REQUIRE(features.device_scope == 1 && features.enabled == (OGPU_NEXT_FEATURE_RASTER | (features.available & OGPU_NEXT_FEATURE_CACHE_CONTROL)));
    q = query(OGPU_NEXT_QUERY_MEMORY_TYPES, NULL, 0, sizeof(*types));
    TRY(ogpu_next_device_query(device, &q));
    types = calloc(q.count, sizeof(*types)); REQUIRE(types != NULL);
    q.data = types; q.capacity = q.count;
    TRY(ogpu_next_device_query(device, &q));
    uint32_t type_count = q.count;
    ogpu_next_memory_desc memory_desc = {
        HEADER(ogpu_next_memory_desc, OGPU_NEXT_MEMORY_DESC),
        4096, 65536, OGPU_NEXT_USAGE_COPY_SRC | OGPU_NEXT_USAGE_COPY_DST | OGPU_NEXT_USAGE_STORAGE | OGPU_NEXT_USAGE_INDIRECT | OGPU_NEXT_USAGE_INDEX,
        0, OGPU_NEXT_MEMORY_LINEAR, NULL, 0, 0
    };
    ogpu_next_requirements requirements = {0};
    TRY(ogpu_next_memory_requirements(device, &memory_desc, &requirements));
    REQUIRE(requirements.compatible_type_count > 0 && requirements.size >= memory_desc.size);
    compatible = calloc(requirements.compatible_type_count, sizeof(*compatible));
    REQUIRE(compatible != NULL);
    requirements.compatible_type_capacity = requirements.compatible_type_count;
    requirements.compatible_types = compatible;
    TRY(ogpu_next_memory_requirements(device, &memory_desc, &requirements));
    memory_desc.memory_type = UINT32_MAX;
    /* Placement policy belongs here in the application, not in the runtime. */
    for (uint32_t i = 0; i < requirements.compatible_type_count; ++i) {
        for (uint32_t j = 0; j < type_count; ++j) {
            if (types[j].id == compatible[i] && (types[j].properties & OGPU_NEXT_MEMORY_HOST_VISIBLE)) {
                memory_desc.memory_type = types[j].id;
                break;
            }
        }
        if (memory_desc.memory_type != UINT32_MAX) break;
    }
    REQUIRE(memory_desc.memory_type != UINT32_MAX);
    TRY(ogpu_next_memory_create(device, &memory_desc, &memory));
    ogpu_next_span all = {memory, 0, memory_desc.size}, part = {memory, 257, 17};
    uint64_t base = 0, address = 0;
    TRY(ogpu_next_memory_address(all, &base)); REQUIRE(base % memory_desc.alignment == 0);
    TRY(ogpu_next_memory_address(part, &address)); REQUIRE(address == base + part.offset);
    ogpu_next_mapping whole = {0}, view = {0};
    TRY(ogpu_next_memory_map(all, &whole));
    TRY(ogpu_next_memory_map(part, &view));
    REQUIRE(view.data == (unsigned char *)whole.data + part.offset);
    REQUIRE(view.cache_atom_size > 0 && view.cache_offset < view.cache_atom_size);
    for (uint32_t i = 0; i < part.size; ++i) ((unsigned char *)view.data)[i] = (unsigned char)(i * 3);
    TRY(ogpu_next_memory_flush(part));
    TRY(ogpu_next_memory_invalidate(part));
    for (uint32_t i = 0; i < part.size; ++i) REQUIRE(((unsigned char *)whole.data)[257 + i] == (unsigned char)(i * 3));
    ogpu_next_memory_unmap(memory);
    REQUIRE(ogpu_next_memory_flush(part) == OGPU_NEXT_INVALID);
    TRY(ogpu_next_memory_map(part, &view));
    for (uint32_t i = 0; i < part.size; ++i) REQUIRE(((unsigned char *)view.data)[i] == (unsigned char)(i * 3));
    /* Device remains live after discovery; only requested optional bits enabled. */
    TRY(ogpu_next_timeline_create(device, 3, &timeline));
    uint64_t value = 0;
    TRY(ogpu_next_timeline_poll(timeline, &value)); REQUIRE(value == 3);
    ogpu_next_point point = {timeline, 4};
    REQUIRE(ogpu_next_timeline_wait(point, 0) == OGPU_NEXT_TIMEOUT);
    TRY(ogpu_next_timeline_signal_host(point));
    TRY(ogpu_next_timeline_wait(point, UINT64_C(1000000000)));
    TRY(ogpu_next_timeline_poll(timeline, &value)); REQUIRE(value == 4);
    REQUIRE(commands(device, memory_desc, queues, queue_count, types, type_count, compatible, requirements.compatible_type_count) == EXIT_SUCCESS);
    REQUIRE(images(device, memory_desc, queues, queue_count) == EXIT_SUCCESS);
    REQUIRE(argc == 5);
    uint32_t compute_domain = UINT32_MAX;
    for (uint32_t i = 0; i < queue_count; ++i)
        if (queues[i].count && (queues[i].flags & OGPU_NEXT_QUEUE_COMPUTE)) { compute_domain = queues[i].domain; break; }
    REQUIRE(compute_domain != UINT32_MAX);
    REQUIRE(compute(device, memory_desc, compute_domain, argv[1]) == EXIT_SUCCESS);
    REQUIRE(heap_execution(device, memory_desc, compute_domain, argv[2]) == EXIT_SUCCESS);
    uint32_t graphics_domain = UINT32_MAX;
    for (uint32_t i = 0; i < queue_count; ++i)
        if (queues[i].count && (queues[i].flags & OGPU_NEXT_QUEUE_GRAPHICS)) { graphics_domain = queues[i].domain; break; }
    REQUIRE(graphics_domain != UINT32_MAX);
    REQUIRE(graphics_execution(device, memory_desc, graphics_domain, argv[3], argv[4], 1, 0) == EXIT_SUCCESS);
    REQUIRE(graphics_execution(device, memory_desc, graphics_domain, argv[3], argv[4], 4, 0) == EXIT_SUCCESS);
    REQUIRE(graphics_execution(device, memory_desc, graphics_domain, argv[3], argv[4], 1, 1) == EXIT_SUCCESS);
    printf("Foundation passes on %s: explicit queues, independent timeline, aligned memory and persistent ranges.\n", info.name);
    result = EXIT_SUCCESS;
cleanup:
    ogpu_next_timeline_destroy(timeline);
    ogpu_next_memory_destroy(memory);
    ogpu_next_device_destroy(device);
    ogpu_next_discovery_destroy(discovery);
    free(requests); free(queues);
    free(types); free(compatible);
    return result;
}
