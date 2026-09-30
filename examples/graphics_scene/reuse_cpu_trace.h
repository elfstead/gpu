/* Compile-time diagnostic for allocations owned by this C consumer, not the
   runtime, C library or driver. Include before the scene consumer sources. */
#ifndef SCENE_REUSE_CPU_TRACE_H
#define SCENE_REUSE_CPU_TRACE_H
#include <stdlib.h>
#include <stdint.h>
#include <inttypes.h>
#include <stdio.h>
#include "../learned_image/allocation_tracker.h"
static AllocationTracker cpu_tracker;
static void *scene_cpu_malloc(size_t bytes) {
    void *p=malloc(bytes);
    if(p && !track_allocate(&cpu_tracker,(uint64_t)(uintptr_t)p,bytes)) abort();
    return p;
}
static void scene_cpu_free(void *p) {
    if(p && !track_free(&cpu_tracker,(uint64_t)(uintptr_t)p)) abort();
    free(p);
}
static void scene_cpu_summary(void) {
    fprintf(stderr,"CPU_MEMORY_SUMMARY {\"allocations\":%" PRIu64 ",\"frees\":%" PRIu64
        ",\"live_count\":%" PRIu64 ",\"live_bytes\":%" PRIu64 "}\n",
        cpu_tracker.allocations,cpu_tracker.frees,cpu_tracker.live_count,cpu_tracker.live_bytes);
    if(cpu_tracker.live_count || cpu_tracker.live_bytes) abort();
}
#define malloc scene_cpu_malloc
#define free scene_cpu_free
#endif
