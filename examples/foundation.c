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

static ogpu_next_query query(uint32_t kind, void *data, uint32_t capacity, uint32_t size) {
    ogpu_next_query q = { HEADER(ogpu_next_query, kind), data, capacity, 0, size, 0 };
    return q;
}
int main(void) {
    int result = EXIT_FAILURE;
    ogpu_next_discovery *discovery = NULL;
    ogpu_next_device *device = NULL;
    ogpu_next_timeline *timeline = NULL;
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
    /* Device remains live after discovery; only requested optional bits enabled. */
    TRY(ogpu_next_timeline_create(device, 3, &timeline));
    uint64_t value = 0;
    TRY(ogpu_next_timeline_poll(timeline, &value)); REQUIRE(value == 3);
    ogpu_next_point point = {timeline, 4};
    REQUIRE(ogpu_next_timeline_wait(point, 0) == OGPU_NEXT_TIMEOUT);
    TRY(ogpu_next_timeline_signal_host(point));
    TRY(ogpu_next_timeline_wait(point, UINT64_C(1000000000)));
    TRY(ogpu_next_timeline_poll(timeline, &value)); REQUIRE(value == 4);
    printf("Foundation setup passes on %s: explicit queues, optional features off, independent timeline.\n", info.name);
    result = EXIT_SUCCESS;
cleanup:
    ogpu_next_timeline_destroy(timeline);
    ogpu_next_device_destroy(device);
    ogpu_next_discovery_destroy(discovery);
    free(requests); free(queues);
    return result;
}
