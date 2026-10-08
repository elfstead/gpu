#include "ogpu_next.h"
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>

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

static ogpu_next_query query(uint32_t kind, void *data, uint32_t capacity, uint32_t size) {
    ogpu_next_query q = { HEADER(ogpu_next_query, kind), data, capacity, 0, size, 0 };
    return q;
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
    TRY(ogpu_next_barrier_scratch_requirements(1, &barrier_req));
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
int main(void) {
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
        if (!features.baseline_supported) continue;
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
        ogpu_next_device_desc desc = { HEADER(ogpu_next_device_desc, OGPU_NEXT_DEVICE_DESC), requests, count, 0, 0 };
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
    REQUIRE(features.device_scope == 1 && features.enabled == 0);
    q = query(OGPU_NEXT_QUERY_MEMORY_TYPES, NULL, 0, sizeof(*types));
    TRY(ogpu_next_device_query(device, &q));
    types = calloc(q.count, sizeof(*types)); REQUIRE(types != NULL);
    q.data = types; q.capacity = q.count;
    TRY(ogpu_next_device_query(device, &q));
    uint32_t type_count = q.count;
    ogpu_next_memory_desc memory_desc = {
        HEADER(ogpu_next_memory_desc, OGPU_NEXT_MEMORY_DESC),
        4096, 65536, OGPU_NEXT_USAGE_COPY_SRC | OGPU_NEXT_USAGE_COPY_DST | OGPU_NEXT_USAGE_STORAGE,
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
