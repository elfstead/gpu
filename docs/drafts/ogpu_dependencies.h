/* C1 dependencies/submission, 2026-11-03. DESIGN ONLY, not installed/linkable.
 * Semantics: ../minimal-core-dependencies.md. Imported operations not replaced
 * here retain their existing contracts, notably split create/destroy/reset.
 */
#ifndef OGPU_DRAFT_DEPENDENCIES_H
#define OGPU_DRAFT_DEPENDENCIES_H
#include "../../include/ogpu_next.h"
#ifdef __cplusplus
extern "C" {
#endif
enum { OGPU_DRAFT_QUERY_DEPENDENCY_LIMITS = 19 };
enum {
    OGPU_DRAFT_DEPENDENCY_SPLIT = 1,
    OGPU_DRAFT_DEPENDENCY_OWNERSHIP = 2,
    OGPU_DRAFT_DEPENDENCY_BINARY = 4
};
enum {
    OGPU_DRAFT_MEMORY_SCOPE_NONE = 0,
    OGPU_DRAFT_MEMORY_SCOPE_OBJECT = 1,
    OGPU_DRAFT_MEMORY_SCOPE_BYTES = 2
};
enum {
    OGPU_DRAFT_SUBMIT_REJECTED = 0,
    OGPU_DRAFT_SUBMIT_ACCEPTED = 1,
    OGPU_DRAFT_SUBMIT_UNKNOWN = 2
};
/* Device query: one immutable record per enabled domain. Stage masks report
 * representable submission scopes, not measured physical overlap. ALL-only
 * means narrower waits/signals must reject. Other native restrictions still apply.
 */
typedef struct ogpu_draft_dependency_limits {
    uint32_t domain, flags, memory_scope, reserved;
    ogpu_next_stages wait_stages, signal_stages, split_stages;
} ogpu_draft_dependency_limits;

typedef struct ogpu_draft_barrier_scope {
    ogpu_next_stages stages_before, stages_after;
    ogpu_next_access access_before, access_after;
} ogpu_draft_barrier_scope;
typedef struct ogpu_draft_memory_barrier {
    ogpu_draft_barrier_scope scope;
    ogpu_next_span range;
    ogpu_next_queue_domain source_domain, destination_domain;
} ogpu_draft_memory_barrier;
typedef struct ogpu_draft_image_barrier {
    ogpu_draft_barrier_scope scope;
    ogpu_next_image *image;
    ogpu_next_subresources range;
    ogpu_next_image_state old_state, new_state;
    ogpu_next_queue_domain source_domain, destination_domain;
    uint32_t discard, reserved;
} ogpu_draft_image_barrier;
typedef struct ogpu_draft_dependency {
    uint32_t global_count, memory_count, image_count, flags;
    const ogpu_draft_barrier_scope *global;
    const ogpu_draft_memory_barrier *memory;
    const ogpu_draft_image_barrier *images;
} ogpu_draft_dependency;
typedef struct ogpu_draft_split_dependency {
    const ogpu_next_split *split;
    const ogpu_draft_dependency *dependency;
} ogpu_draft_split_dependency;

ogpu_next_status ogpu_draft_barrier_scratch_requirements(
    const ogpu_next_device *, uint32_t globals, uint32_t memory, uint32_t images,
    ogpu_next_host_requirements *);
void ogpu_draft_barrier(ogpu_next_encoder *, const ogpu_draft_dependency *,
    ogpu_next_host_span scratch);
void ogpu_draft_split_release(ogpu_next_encoder *, const ogpu_next_split *,
    const ogpu_draft_dependency *, ogpu_next_host_span scratch);
ogpu_next_status ogpu_draft_split_scratch_requirements(
    const ogpu_next_device *, uint32_t count, const ogpu_draft_split_dependency *,
    ogpu_next_host_requirements *);
void ogpu_draft_split_acquire(ogpu_next_encoder *, uint32_t count,
    const ogpu_draft_split_dependency *, ogpu_next_host_span scratch);

/* All host arrays borrowed only during submit. No linked binary extension or
 * per-batch scratch: one reusable translation span covers the complete call.
 */
typedef struct ogpu_draft_submit_desc {
    uint32_t list_count, wait_count, signal_count;
    uint32_t binary_wait_count, binary_signal_count, flags;
    ogpu_next_list *const *lists;
    const ogpu_next_sync_point *waits, *signals;
    const ogpu_next_binary_point *binary_waits, *binary_signals;
} ogpu_draft_submit_desc;
typedef struct ogpu_draft_submit_outcome {
    ogpu_next_status status;
    uint32_t acceptance;
} ogpu_draft_submit_outcome;
/* Requirements depend only on counts/flags; nested object arrays are not read. */
ogpu_next_status ogpu_draft_submit_scratch_requirements(
    const ogpu_next_device *, uint32_t count, const ogpu_draft_submit_desc *,
    ogpu_next_host_requirements *);
/* count=1 is ordinary submit. Outcomes has count entries, written on every call
 * with trusted valid inputs/outputs. All local preflight precedes native effects.
 * UNKNOWN is not retryable and does not prove completion of any referenced use.
 */
ogpu_next_status ogpu_draft_queue_submit(ogpu_next_queue *, uint32_t count,
    const ogpu_draft_submit_desc *, ogpu_draft_submit_outcome *, ogpu_next_host_span scratch);
#ifdef __cplusplus
}
#endif
#endif
