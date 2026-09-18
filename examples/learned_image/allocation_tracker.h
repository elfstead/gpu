#ifndef LEARNED_IMAGE_ALLOCATION_TRACKER_H
#define LEARNED_IMAGE_ALLOCATION_TRACKER_H
#include <stdint.h>
#include <stddef.h>

// Diagnostic-only: one serialized device, no placement changes or hidden frees.
typedef struct {
    struct { uint64_t handle, bytes; } slots[1024];
    uint64_t live_bytes, peak_bytes, live_count, peak_count, allocations, frees;
} AllocationTracker;

static inline int track_allocate(AllocationTracker *t, uint64_t handle, uint64_t bytes) {
    if (!handle || !bytes || bytes > UINT64_MAX - t->live_bytes || t->allocations == UINT64_MAX) return 0;
    size_t free_slot = 1024;
    for (size_t i = 0; i < 1024; ++i) {
        if (t->slots[i].handle == handle) return 0;
        if (!t->slots[i].handle && free_slot == 1024) free_slot = i;
    }
    if (free_slot == 1024) return 0;
    t->slots[free_slot].handle = handle; t->slots[free_slot].bytes = bytes;
    t->live_bytes += bytes; ++t->live_count; ++t->allocations;
    if (t->live_bytes > t->peak_bytes) t->peak_bytes = t->live_bytes;
    if (t->live_count > t->peak_count) t->peak_count = t->live_count;
    return 1;
}

static inline int track_free(AllocationTracker *t, uint64_t handle) {
    for (size_t i = 0; i < 1024; ++i) if (handle && t->slots[i].handle == handle) {
        t->live_bytes -= t->slots[i].bytes; --t->live_count; ++t->frees;
        t->slots[i].handle = 0; t->slots[i].bytes = 0;
        return 1;
    }
    return 0;
}
#endif
