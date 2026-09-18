#include "allocation_tracker.h"
#include <assert.h>
#include <stdio.h>
int main(void) {
    AllocationTracker t = {0};
    assert(!track_allocate(&t, 0, 4));
    assert(!track_allocate(&t, 1, 0));
    assert(track_allocate(&t, 1, 64));
    assert(!track_allocate(&t, 1, 64));
    assert(!track_allocate(&t, 2, UINT64_MAX));
    assert(t.live_bytes == 64 && t.allocations == 1);
    assert(track_allocate(&t, 2, 32));
    assert(track_free(&t, 1));
    assert(!track_free(&t, 1) && !track_free(&t, 0));
    assert(track_allocate(&t, 1, 128)); // handle reuse, no stale size
    assert(t.peak_bytes == 160 && t.peak_count == 2);
    assert(track_free(&t, 1) && track_free(&t, 2));
    assert(t.live_bytes == 0 && t.live_count == 0 && t.allocations == t.frees);
    for (uint64_t i = 1; i <= 1024; ++i) assert(track_allocate(&t, i, 1));
    assert(!track_allocate(&t, 1025, 1));
    for (uint64_t i = 1; i <= 1024; ++i) assert(track_free(&t, i));
    assert(t.allocations == t.frees && t.live_bytes == 0);
    puts("Allocation tracking: overflow, duplicates, exhaustion, reuse and cleanup PASS");
    return 0;
}
