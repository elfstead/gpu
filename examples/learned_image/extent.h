#ifndef LEARNED_IMAGE_EXTENT_H
#define LEARNED_IMAGE_EXTENT_H
#include <errno.h>
#include <limits.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct { uint32_t x, y, stride; } Launch;

static inline uint32_t extent(const char *text) {
    if (!*text) return 0;
    for (const char *p = text; *p; ++p) if (*p < '0' || *p > '9') return 0;
    char *end;
    errno = 0;
    unsigned long value = strtoul(text, &end, 10);
    // Shader coordinates use signed ints, including a +1 neighbor.
    return errno || *end || !value || value >= INT32_MAX ? 0 : (uint32_t)value;
}

static inline int image_count(uint32_t w, uint32_t h, uint32_t channels, uint32_t *count) {
    if (!w || !h || w >= INT32_MAX || h >= INT32_MAX || !channels) return 0;
    uint64_t pixels = (uint64_t)w * h;
    // Diagnostic poison counts include two 64-byte guards. All shader indices
    // and the guarded word count must fit uint32, all host bytes size_t.
    if (pixels > (UINT32_MAX - 32u) / channels
        || pixels > (SIZE_MAX - 128u) / sizeof(float) / channels) return 0;
    *count = (uint32_t)pixels;
    return 1;
}

static inline int add_size(size_t a, size_t b, size_t *out) {
    if (b > SIZE_MAX - a) return 0;
    *out = a + b;
    return 1;
}

static inline int resize_axis(uint32_t input, uint32_t output) {
    // Shader numerator = (2*output_pixel + 1)*input, denominator = 2*output.
    return input && output && output < INT32_MAX
        && (2 * (uint64_t)output - 1) * input <= UINT32_MAX;
}

static inline int launch(uint32_t count, const uint32_t local[3],
                         const uint32_t maximum[3], Launch *out) {
    if (!count || !local[0] || local[1] != 1 || local[2] != 1
        || !maximum[0] || !maximum[1] || !maximum[2]) return 0;
    uint64_t groups = ((uint64_t)count + local[0] - 1) / local[0];
    uint64_t x = groups < maximum[0] ? groups : maximum[0];
    // Application row policy, not a device requirement or tuning claim. Some
    // devices permit huge X grids; bounded rows exercise real Y/tail addressing.
    if (x > 1024) x = 1024;
    uint64_t y = (groups + x - 1) / x;
    uint64_t stride = x * local[0];
    // Includes tail invocations: even rejected lanes must not wrap their index.
    if (y > maximum[1] || stride > UINT32_MAX || stride * y > UINT32_MAX) return 0;
    *out = (Launch){(uint32_t)x, (uint32_t)y, (uint32_t)stride};
    return 1;
}
#endif
