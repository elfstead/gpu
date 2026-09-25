#include "ranges.h"
#include "slots.h"
#include <stdio.h>
#include <string.h>

/* Checks stay active under NDEBUG; no GPU or runtime linkage. */
#define CHECK(condition) do { if (!(condition)) { \
    fprintf(stderr, "Reuse test line %d: %s\n", __LINE__, #condition); return 0; \
} } while (0)

static int rejected(ReuseArena arena, uint64_t size, uint64_t alignment, uint64_t atom, uint64_t guard) {
    ReuseArena before = arena;
    ReuseRange range = {11, 22, 33, 44}, original = range;
    CHECK(!reuse_reserve(&arena, size, alignment, atom, guard, &range));
    CHECK(arena.capacity == before.capacity && arena.used == before.used);
    CHECK(!memcmp(&range, &original, sizeof(range)));
    return 1;
}

static int ranges(void) {
    ReuseArena arena = {.capacity=1024}; ReuseRange first, second;
    CHECK(reuse_reserve(&arena, 257, 16, 256, 64, &first));
    CHECK(first.begin == 0 && first.payload == 64 && first.size == 257 && first.end == 512);
    CHECK(reuse_reserve(&arena, 257, 16, 256, 64, &second));
    CHECK(second.begin == 512 && second.payload == 576 && second.end == 1024 && arena.used == 1024);
    CHECK(rejected(arena, 1, 1, 1, 0));
    CHECK(rejected((ReuseArena){.capacity=1023}, 1024, 1, 1, 0));
    CHECK(rejected((ReuseArena){.capacity=100, .used=101}, 1, 1, 1, 0));
    CHECK(rejected((ReuseArena){.capacity=100}, 0, 1, 1, 0));
    CHECK(rejected((ReuseArena){.capacity=100}, 1, 0, 1, 0));
    CHECK(rejected((ReuseArena){.capacity=100}, 1, 1, 0, 0));
    CHECK(rejected((ReuseArena){.capacity=UINT64_MAX}, 1, UINT64_MAX, 2, 0));
    CHECK(rejected((ReuseArena){.capacity=UINT64_MAX, .used=UINT64_MAX-1}, 1, 4, 1, 0));
    CHECK(rejected((ReuseArena){.capacity=UINT64_MAX, .used=1}, UINT64_MAX, 1, 1, 0));
    CHECK(rejected((ReuseArena){.capacity=UINT64_MAX}, 1, 1, 1, UINT64_MAX));
    CHECK(rejected((ReuseArena){.capacity=UINT64_MAX}, UINT64_MAX, 1, 2, 0));
    arena = (ReuseArena){.capacity=UINT64_MAX};
    CHECK(reuse_reserve(&arena, UINT64_MAX, 1, 1, 0, &first) && first.end == UINT64_MAX);
    CHECK(!reuse_reserve(NULL, 1, 1, 1, 0, &first));
    CHECK(!reuse_reserve(&arena, 1, 1, 1, 0, NULL));
    uint64_t value = 987;
    CHECK(!reuse_align(UINT64_MAX, 2, &value) && value == 987);
    CHECK(reuse_quantum(12, 18, &value) && value == 36);
    CHECK(!reuse_quantum(UINT64_MAX, 2, &value) && value == 36);
    /* Small exhaustive arithmetic oracle, including non-power-of-two quanta.
     * Validate disjoint complete atoms, guards, payload alignment and capacity. */
    for (uint64_t align = 1; align <= 24; ++align) {
        for (uint64_t atom = 1; atom <= 32; ++atom) {
            arena = (ReuseArena){.capacity=65536};
            uint64_t last = 0;
            for (uint64_t n = 1; n <= 31; ++n) {
                CHECK(reuse_reserve(&arena, n, align, atom, 7, &first));
                CHECK(first.begin >= last && first.begin%atom == 0 && first.payload%align == 0);
                CHECK(first.payload-first.begin >= 7 && first.size == n);
                CHECK(first.end-first.payload-n >= 7 && first.end%atom == 0);
                CHECK(arena.used == first.end && arena.used <= arena.capacity);
                last = first.end;
            }
        }
    }
    return 1;
}

static int states(void) {
    ReuseSlot slot = {0}; uint64_t ticket = 0, untouched = 999;
    CHECK(reuse_execution_drained(&slot));
    CHECK(!reuse_recorded(&slot, 0) && !reuse_submitted(&slot, 0));
    CHECK(!reuse_acquire(NULL, &ticket) && !reuse_acquire(&slot, NULL));
    for (unsigned frame = 0; frame < 1000; ++frame) {
        uint64_t stale = ticket;
        CHECK(reuse_acquire(&slot, &ticket) && ticket == (uint64_t)frame+1);
        CHECK(!reuse_execution_drained(&slot));
        CHECK(!reuse_acquire(&slot, &untouched) && untouched == 999);
        CHECK(!reuse_recorded(&slot, stale) && !reuse_submitted(&slot, ticket));
        CHECK(reuse_recorded(&slot, ticket));
        CHECK(!reuse_recorded(&slot, ticket) && !reuse_release(&slot, ticket));
        CHECK(reuse_submitted(&slot, ticket));
        CHECK(!reuse_abort_unsubmitted(&slot, ticket));
        CHECK(!reuse_acquire(&slot, &untouched) && untouched == 999);
        CHECK(!reuse_release(&slot, ticket) && !reuse_completed(&slot, stale));
        /* An incomplete poll/transient error invokes no completion transition. */
        CHECK(slot.state == REUSE_PENDING && !reuse_execution_drained(&slot));
        CHECK(reuse_completed(&slot, ticket));
        CHECK(!reuse_completed(&slot, ticket) && !reuse_acquire(&slot, &untouched));
        CHECK(!reuse_execution_drained(&slot)); /* CPU result not consumed yet. */
        CHECK(reuse_release(&slot, ticket) && reuse_execution_drained(&slot));
        CHECK(!reuse_release(&slot, ticket));
    }
    CHECK(reuse_acquire(&slot, &ticket));
    CHECK(reuse_abort_unsubmitted(&slot, ticket) && reuse_execution_drained(&slot));
    CHECK(reuse_acquire(&slot, &ticket) && reuse_recorded(&slot, ticket));
    CHECK(reuse_abort_unsubmitted(&slot, ticket)); /* Known rejected submit. */
    for (unsigned accepted = 0; accepted < 2; ++accepted) {
        ReuseSlot failed = {0};
        CHECK(reuse_acquire(&failed, &ticket) && reuse_recorded(&failed, ticket));
        if (accepted) CHECK(reuse_submitted(&failed, ticket));
        CHECK(reuse_quarantine(&failed, ticket));
        CHECK(!reuse_execution_drained(&failed) && !reuse_acquire(&failed, &untouched));
        CHECK(!reuse_abort_unsubmitted(&failed, ticket) && !reuse_completed(&failed, ticket));
        CHECK(!reuse_release(&failed, ticket) && !reuse_failed_drained(&failed, ticket+1));
        CHECK(reuse_failed_drained(&failed, ticket) && reuse_execution_drained(&failed));
        CHECK(!reuse_acquire(&failed, &untouched) && untouched == 999); /* Terminal, never recycled. */
        CHECK(!reuse_failed_drained(&failed, ticket));
    }
    slot = (ReuseSlot){.generation=UINT64_MAX-1};
    CHECK(reuse_acquire(&slot, &ticket) && ticket == UINT64_MAX);
    CHECK(reuse_abort_unsubmitted(&slot, ticket));
    CHECK(!reuse_acquire(&slot, &untouched) && untouched == 999 && slot.state == REUSE_IDLE);
    CHECK(!reuse_execution_drained(NULL));
    return 1;
}

int main(void) {
    if (!ranges() || !states()) return 1;
    puts("Consumer ranges/state PASS: 23808 mixed-alignment ranges, 1000 generations, overflow/reuse/drain rejection");
    return 0;
}
