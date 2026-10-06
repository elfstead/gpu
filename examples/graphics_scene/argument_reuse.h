/* Compile-time experiment only; no inferred immutability in the public API. */
#ifndef SCENE_ARGUMENT_REUSE_H
#define SCENE_ARGUMENT_REUSE_H
#include <stdint.h>
#include <string.h>
#ifdef SCENE_ARGUMENT_PROFILE
#include <time.h>
static double argument_record_ms,argument_submit_ms;
static double argument_clock(void) {
    struct timespec now;if(clock_gettime(CLOCK_MONOTONIC,&now)) abort();
    return now.tv_sec*1000.0+now.tv_nsec/1000000.0;
}
#endif
static unsigned argument_reverse;
#ifdef SCENE_ARGUMENT_DIAGNOSTICS
static uint64_t argument_calls,argument_bytes;
#define ARGUMENT_SUPPLIED() (++argument_calls,argument_bytes+=SCENE_RASTER_BYTES)
#else
#define ARGUMENT_SUPPLIED() ((void)0)
#endif
static void argument_prepare(uint64_t *root,uint64_t address) {
    memcpy(root,&address,8);
    for(unsigned i=0;i<(SCENE_RASTER_BYTES-8)/4;++i) {
        uint32_t value=i+1;memcpy((unsigned char *)root+8+4*i,&value,4);
    }
}
static unsigned argument_index(unsigned i,unsigned capacity) {
    return argument_reverse ? capacity-1-i : i;
}
#endif
