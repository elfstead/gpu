/*
 * OGPU whole-surface C sketch, 2026-10-08. NOT a fully implemented/installed ABI.
 * Companion semantics: ../whole-api-design.md
 *
 * The ogpu_next_ prefix prevents confusion with include/ogpu.h (ABI 20).
 * Setup declarations now come from include/ogpu_next.h and are implemented on
 * Linux. The declarations added HERE remain draft and are not linkable. Remaining
 * record layouts/extension schemas are unfinished, not implemented coverage.
 *
 * Foundation rules:
 * - No implicit resource retention, host wait, staging or per-command root copy.
 * - Destruction requires no pending/recorded future use; it does not wait.
 * - Each mutable arena/encoder/queue is externally synchronized; independent
 *   objects may be used concurrently. Resource lifetime changes exclude its use.
 * - Recording preconditions are trusted; optional validation diagnoses misuse.
 *   Allocation/encoding failure poisons the list and is reported by commands_end.
 * - All requested strategies are capability checked, never silently emulated.
 */
#ifndef OGPU_NEXT_DESIGN_DRAFT_H
#define OGPU_NEXT_DESIGN_DRAFT_H
#include "../../include/ogpu_next.h"
#ifdef __cplusplus
extern "C" {
#endif

/* Discovery, queue/memory topology, requested feature enabling and independent
 * timeline objects and scalar arithmetic/storage enabling: implemented header above.
 * Subgroup queries and per-stage size/operation requirements are also implemented.
 * Budget, broader atomic/matrix numerical tuples and
 * exact executable/format/state query records remain to be defined. */

/* Backing, placed/dedicated images, views and explicit image transitions/copies
 * are now in the implemented header above. Broader format/feature profiles follow. */

/* Descriptor byte encoding and borrowed GPU heap-range binding are implemented
 * in the imported header. No owned heap object or implicit allocation policy. */

/* Compute/graphics preparation, inline/device-root ABI and direct/indirect launch
 * come from the implemented header, together with independent preparation caches,
 * import/export, merge and explicit fail-if-compilation-required policy.
 * Wider graphics/native artifact profiles remain to
 * implement; no implicit translation or command-time JIT. */

/* Primary arenas, replay, explicit host scratch, single/batched submission and memory barriers
 * are implemented in the imported header. Nested/secondary execution follows. */
void ogpu_next_execute_lists(ogpu_next_encoder *, uint32_t count, ogpu_next_list *const *);

/* Barriers separate execution scopes, memory access and optional ranges/state.
 * Zero resource ranges means a global memory dependency, not no dependency.
 * Ownership transfer endpoints use matching release/acquire records and a queue
 * timeline edge. Split tokens are arena-local and capability-scoped; not generic
 * GPU-address semaphores. Exact reset/replay rules remain to be specified. */
typedef struct ogpu_next_split { uint64_t value; } ogpu_next_split;
void ogpu_next_split_release(ogpu_next_encoder *, const ogpu_next_dependency *, ogpu_next_split *);
void ogpu_next_split_acquire(ogpu_next_encoder *, ogpu_next_split, const ogpu_next_dependency *);
void ogpu_next_alias_activate(ogpu_next_encoder *, ogpu_next_image *, const ogpu_next_dependency *);

/* Ordinary commands copy immediate parameter records into command storage;
 * resource data and device roots are never copied implicitly. */
/* Image-to-image copies and native-color resolves are imported above with explicit
 * source/destination regions AND layouts. Other resolve profiles remain draft work. */

/* Graphics preparation, attachment scopes, explicit viewport/scissor and direct/
 * indexed/indirect/count draws, multisampling and color attachment resolves are imported
 * above, together with static vertex input and explicit address-span binding.
 * Broader sample/dynamic-stencil/dynamic-vertex-input/tile-local and
 * dynamic-state profiles remain to implement; no hidden dynamic-to-PSO JIT. */
void ogpu_next_draw_mesh(ogpu_next_encoder *, ogpu_next_extent groups);
void ogpu_next_draw_mesh_indirect(ogpu_next_encoder *, const ogpu_next_indirect *);

/* Timestamp/occlusion/statistics pools and explicit GPU result copies are imported
 * above, including precise occlusion. Performance-counter profiles follow. */
void ogpu_next_label_begin(ogpu_next_encoder *, const char *name);
void ogpu_next_label_end(ogpu_next_encoder *);

/* Versioned optional function tables, negotiated by exact schema/size. An
 * unavailable table is UNSUPPORTED; its presence is not inferred from backend.
 * Never use untyped void* as a portable C function-pointer representation. */
ogpu_next_status ogpu_next_extension(ogpu_next_device *, uint32_t id, uint32_t version, size_t table_size, void *table);

/* Planned extension tables, NOT specified or implemented in this sketch:
 *
 * Residency: create/update sets, commit residency, attach to queue/submission,
 *            optional explicit eviction. No mandatory command resource walk.
 * Virtual memory: reserve/release, backing create/free, map/unmap/access;
 *                sparse image bindings and peer mappings with explicit ordering.
 * Generated commands: supported tokens, executable tables, layout create,
 *                     storage requirements, prepare, execute with GPU count.
 * Native interop: borrow device/queue, import/export memory/images/timelines;
 *                typed platform handles, owned/borrowed modes, initial/final
 *                states and native encoding sections with invalidated bindings.
 * Address-native commands: raw-address copy/index/indirect endpoints where the
 *                          backend supports them without backing registration.
 * Presentation: surface/swapchain create, acquire token, present dependency,
 *               resize/format negotiation. Not part of device construction.
 * Advanced raster: tile-local dependencies, multiview, shading rate,
 *                  pass continuation, vertex fetch and additional stages.
 * Accelerated compute: exact matrix/tensor descriptors and launch profiles,
 *                      cooperative/cluster/device-enqueue operations.
 * Ray tracing: acceleration structure requirements/build/copy/query,
 *              ray executable/shader binding table, direct/indirect trace.
 * Video: codec/session capabilities, plane views, parameter/reference records,
 *        bitstream ranges, explicit decode/encode and status queries.
 *
 * Shader-only subgroup/atomic/numeric features use artifact requirement records;
 * they do not need an extra host function merely to count as supported.
 */

#ifdef __cplusplus
}
#endif
#endif
