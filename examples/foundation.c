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

static ogpu_next_query query(uint32_t kind, void *data, uint32_t capacity, uint32_t size) {
    ogpu_next_query q = { HEADER(ogpu_next_query, kind), data, capacity, 0, size, 0 };
    return q;
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
        4096, 65536, OGPU_NEXT_USAGE_COPY_SRC | OGPU_NEXT_USAGE_COPY_DST,
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
