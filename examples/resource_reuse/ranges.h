#ifndef OGPU_EXAMPLE_REUSE_RANGES_H
#define OGPU_EXAMPLE_REUSE_RANGES_H
#include <stdint.h>

/* Consumer bookkeeping, not a GPU allocator. Backing storage is caller-owned.
 * Base address must meet payload alignment; offsets alone cannot establish it.
 * All operations are externally serialized. Outputs and cursor stay unchanged
 * on rejection. No automatic growth, freeing, aliasing or cache operations.
 */
typedef struct ReuseArena {
    uint64_t capacity, used;
} ReuseArena;
typedef struct ReuseRange {
    uint64_t begin, payload, size, end; /* [begin,end), including guards/padding */
} ReuseRange;

static inline int reuse_add(uint64_t a, uint64_t b, uint64_t *out) {
    if (!out || a > UINT64_MAX-b) return 0;
    *out = a+b;
    return 1;
}
static inline int reuse_align(uint64_t value, uint64_t alignment, uint64_t *out) {
    if (!alignment || !out) return 0;
    uint64_t remainder = value%alignment;
    return reuse_add(value, remainder ? alignment-remainder : 0, out);
}
static inline int reuse_quantum(uint64_t alignment, uint64_t granularity, uint64_t *out) {
    if (!alignment || !granularity || !out) return 0;
    uint64_t a = alignment, b = granularity;
    while (b) { uint64_t remainder = a%b; a = b; b = remainder; }
    uint64_t factor = alignment/a;
    if (factor > UINT64_MAX/granularity) return 0;
    *out = factor*granularity;
    return 1;
}

/* Atom-isolated spans: begin/end are cache-granularity multiples; payload is
 * alignment-aligned. Prefix/suffix are AT LEAST guard bytes. Padding belongs to
 * this range; the caller initializes/checks it, including any inter-range gap.
 * For DEVICE ranges without host cache operations, pass granularity=1.
 */
static inline int reuse_reserve(ReuseArena *arena, uint64_t size, uint64_t alignment,
                                uint64_t granularity, uint64_t guard, ReuseRange *out) {
    if (!arena || !out || !size || arena->used > arena->capacity) return 0;
    ReuseRange range = {0}; uint64_t quantum, cursor;
    if (!reuse_quantum(alignment, granularity, &quantum)
        || !reuse_align(arena->used, quantum, &range.begin)
        || !reuse_add(range.begin, guard, &cursor)
        || !reuse_align(cursor, alignment, &range.payload)
        || !reuse_add(range.payload, size, &cursor)
        || !reuse_add(cursor, guard, &cursor)
        || !reuse_align(cursor, granularity, &range.end)
        || range.end > arena->capacity) return 0;
    range.size = size;
    *out = range; arena->used = range.end;
    return 1;
}

#endif
