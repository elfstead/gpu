#ifndef OGPU_LEARNED_FOUNDATION_TIMING_H
#define OGPU_LEARNED_FOUNDATION_TIMING_H
#include <math.h>
#include <stdint.h>
/* Consumer policy: a >10-second wrap interval, not an API capability minimum.
 * Every measured submission must finish inside one full counter period. */
static double timestamp_wrap_ms(uint32_t bits, double period_ns) {
    return bits && bits <= 64 && isfinite(period_ns) && period_ns > 0 ? ldexp(period_ns,(int)bits)/1e6 : 0;
}
static int timestamp_delta_ms(uint64_t first, uint64_t last, uint32_t bits, double period_ns,
    double host_interval_ms, double *result) {
    double wrap = timestamp_wrap_ms(bits,period_ns);
    if (wrap <= 10000 || !isfinite(wrap) || !isfinite(host_interval_ms) || host_interval_ms < 0 || host_interval_ms >= wrap) return 0;
    uint64_t ticks = (last-first) & (UINT64_MAX >> (64-bits));
    *result = (double)ticks*period_ns/1e6;
    return isfinite(*result) && *result > 0;
}
#endif
