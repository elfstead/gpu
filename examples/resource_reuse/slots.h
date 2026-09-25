#ifndef OGPU_EXAMPLE_REUSE_SLOTS_H
#define OGPU_EXAMPLE_REUSE_SLOTS_H
#include <stdint.h>

/* Caller-asserted transitions, not a runtime fence or race detector. Keep actual
 * batch/list/completion/backing owners elsewhere. Zero initialization is IDLE.
 * A ticket is local to one slot; it must never be used with a different slot.
 * All operations are externally serialized; no automatic waits/cancellation.
 */
typedef enum ReuseState {
    REUSE_IDLE, REUSE_RESERVED, REUSE_RECORDED, REUSE_PENDING, REUSE_READY,
    REUSE_FAILED_PENDING, REUSE_FAILED_DRAINED
} ReuseState;
typedef struct ReuseSlot {
    uint64_t generation;
    ReuseState state;
} ReuseSlot;

static inline int reuse_acquire(ReuseSlot *slot, uint64_t *ticket) {
    if (!slot || !ticket || slot->state != REUSE_IDLE || slot->generation == UINT64_MAX) return 0;
    ++slot->generation;
    slot->state = REUSE_RESERVED;
    *ticket = slot->generation;
    return 1;
}
static inline int reuse_transition(ReuseSlot *slot, uint64_t ticket, ReuseState from, ReuseState to) {
    if (!slot || !ticket || slot->generation != ticket || slot->state != from) return 0;
    slot->state = to;
    return 1;
}
/* New commands encoded, or an existing immutable list selected for this range.
 * Compiled-list backing lifetime remains independent of per-execution tickets. */
static inline int reuse_recorded(ReuseSlot *slot, uint64_t ticket) {
    return reuse_transition(slot, ticket, REUSE_RESERVED, REUSE_RECORDED);
}
/* Only after submit ACCEPTS the execution, never before calling the API. */
static inline int reuse_submitted(ReuseSlot *slot, uint64_t ticket) {
    return reuse_transition(slot, ticket, REUSE_RECORDED, REUSE_PENDING);
}
/* Only after successful completion observation. A timeout, incomplete poll or
 * transient error changes nothing; do not call this function for those cases. */
static inline int reuse_completed(ReuseSlot *slot, uint64_t ticket) {
    return reuse_transition(slot, ticket, REUSE_PENDING, REUSE_READY);
}
/* CPU consumption/cache handling must finish before releasing READY storage. */
static inline int reuse_release(ReuseSlot *slot, uint64_t ticket) {
    return reuse_transition(slot, ticket, REUSE_READY, REUSE_IDLE);
}
/* Caller has destroyed/abandoned one-shot recording as appropriate and knows NO
 * execution was accepted. This is NOT how unknown submit outcomes are handled. */
static inline int reuse_abort_unsubmitted(ReuseSlot *slot, uint64_t ticket) {
    if (!slot || !ticket || slot->generation != ticket
        || (slot->state != REUSE_RESERVED && slot->state != REUSE_RECORDED)) return 0;
    slot->state = REUSE_IDLE;
    return 1;
}
/* Unknown submit outcome or terminal execution error: quarantine, retaining all
 * actual owners until the caller proves drain/retirement. Never recycle it. */
static inline int reuse_quarantine(ReuseSlot *slot, uint64_t ticket) {
    if (!slot || !ticket || slot->generation != ticket
        || (slot->state != REUSE_RECORDED && slot->state != REUSE_PENDING)) return 0;
    slot->state = REUSE_FAILED_PENDING;
    return 1;
}
static inline int reuse_failed_drained(ReuseSlot *slot, uint64_t ticket) {
    return reuse_transition(slot, ticket, REUSE_FAILED_PENDING, REUSE_FAILED_DRAINED);
}
/* This execution no longer prevents teardown. Retained compiled-list or other
 * resource ownership may STILL prevent freeing the backing allocation. */
static inline int reuse_execution_drained(const ReuseSlot *slot) {
    return slot && (slot->state == REUSE_IDLE || slot->state == REUSE_FAILED_DRAINED);
}

#endif
