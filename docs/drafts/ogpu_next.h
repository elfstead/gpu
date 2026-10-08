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

typedef struct ogpu_next_image ogpu_next_image;
typedef struct ogpu_next_view ogpu_next_view;
typedef struct ogpu_next_heap ogpu_next_heap;
typedef struct ogpu_next_executable ogpu_next_executable;
typedef struct ogpu_next_query_pool ogpu_next_query_pool;

typedef struct ogpu_next_bytes { const void *data; size_t size; } ogpu_next_bytes;
typedef struct ogpu_next_extent { uint32_t x, y, z; } ogpu_next_extent;
typedef struct ogpu_next_offset { int32_t x, y, z; } ogpu_next_offset;

/* Discovery, queue/memory topology, requested feature enabling and independent
 * timeline objects: implemented header above. Budget, numerical tuples and
 * exact executable/format/state query records remain to be defined. */

/* Explicit linear/opaque backing, requirements, ranges, mapping and cache
 * visibility are now in the implemented header above. Placement follows below. */

/* Images are placed interpretations of backing. A view can select compatible
 * format/aspects/mips/layers for sampled, storage or attachment use. Native
 * dedicated-allocation associations use explicit creation extension records. */
typedef struct ogpu_next_image_desc {
    ogpu_next_record header;
    ogpu_next_format format;
    uint32_t dimension, mip_count, layer_count, sample_count;
    ogpu_next_extent extent;
    uint64_t usage;
    uint32_t flags;
    const ogpu_next_format *view_formats;
    uint32_t view_format_count;
} ogpu_next_image_desc;
typedef struct ogpu_next_subresources {
    uint32_t aspects, first_mip, mip_count, first_layer, layer_count;
} ogpu_next_subresources;
typedef struct ogpu_next_view_desc {
    ogpu_next_record header;
    ogpu_next_format format;
    uint32_t dimension, usage, component_mapping[4];
    ogpu_next_subresources range;
} ogpu_next_view_desc;
ogpu_next_status ogpu_next_image_requirements(ogpu_next_device *, const ogpu_next_image_desc *, ogpu_next_requirements *);
ogpu_next_status ogpu_next_image_create(ogpu_next_device *, const ogpu_next_image_desc *, ogpu_next_span placement, ogpu_next_image **);
void ogpu_next_image_destroy(ogpu_next_image *);
ogpu_next_status ogpu_next_view_create(ogpu_next_image *, const ogpu_next_view_desc *, ogpu_next_view **);
void ogpu_next_view_destroy(ogpu_next_view *);

/* Heap mutation is range-local. Caller owns slot/resource lifetimes. Sampler
 * state is encoded into a slot, not necessarily a separately owned object. */
typedef struct ogpu_next_sampler_desc {
    ogpu_next_record header;
    uint32_t min_filter, mag_filter, mip_filter, address_mode[3];
    uint32_t compare_enable, compare_op, border_color;
    float min_lod, max_lod, lod_bias, max_anisotropy;
} ogpu_next_sampler_desc;
typedef struct ogpu_next_heap_desc {
    ogpu_next_record header;
    uint32_t kind, capacity, flags;
} ogpu_next_heap_desc;
ogpu_next_status ogpu_next_heap_create(ogpu_next_device *, const ogpu_next_heap_desc *, ogpu_next_heap **);
void ogpu_next_heap_destroy(ogpu_next_heap *);
ogpu_next_status ogpu_next_heap_write_images(ogpu_next_heap *, uint32_t first, uint32_t count, ogpu_next_view *const *);
ogpu_next_status ogpu_next_heap_write_samplers(ogpu_next_heap *, uint32_t first, uint32_t count, const ogpu_next_sampler_desc *);
ogpu_next_status ogpu_next_heap_copy(ogpu_next_heap *dst, uint32_t dst_first, ogpu_next_heap *src, uint32_t src_first, uint32_t count);

/* Preparation owns compilation/linking/specialization. Interface metadata
 * defines inline bytes, device-root slots, stage visibility, numeric requirements
 * and compatibility. Native artifacts are not forcibly translated at runtime.
 * Full schema for generated interface, specialization and cache records pending. */
typedef struct ogpu_next_shader {
    uint32_t stage, format;
    ogpu_next_bytes code;
    const char *entry;
    const ogpu_next_record *interface_metadata;
    const ogpu_next_record *specialization;
} ogpu_next_shader;
typedef struct ogpu_next_executable_desc {
    ogpu_next_record header;
    uint32_t kind, shader_count;
    const ogpu_next_shader *shaders;
    const ogpu_next_record *static_state;
    uint64_t dynamic_state;
    const ogpu_next_record *requirements;
    ogpu_next_bytes native_cache;
} ogpu_next_executable_desc;
ogpu_next_status ogpu_next_executable_create(ogpu_next_device *, const ogpu_next_executable_desc *, ogpu_next_executable **);
ogpu_next_status ogpu_next_executable_cache(ogpu_next_executable *, size_t *size, void *data);
void ogpu_next_executable_destroy(ogpu_next_executable *);

/* Primary arenas, replay, explicit host scratch, submission and memory barriers
 * are implemented in the imported header. Nested/secondary execution follows. */
void ogpu_next_execute_lists(ogpu_next_encoder *, uint32_t count, ogpu_next_list *const *);

/* Barriers separate execution scopes, memory access and optional ranges/state.
 * Zero resource ranges means a global memory dependency, not no dependency.
 * Ownership transfer endpoints use matching release/acquire records and a queue
 * timeline edge. Split tokens are arena-local and capability-scoped; not generic
 * GPU-address semaphores. Exact reset/replay rules remain to be specified. */
struct ogpu_next_image_barrier {
    ogpu_next_image *image;
    ogpu_next_subresources range;
    ogpu_next_access before, after;
    ogpu_next_image_state old_state, new_state;
    ogpu_next_queue_domain source_domain, destination_domain;
    uint32_t discard;
};
typedef struct ogpu_next_split { uint64_t value; } ogpu_next_split;
void ogpu_next_split_release(ogpu_next_encoder *, const ogpu_next_dependency *, ogpu_next_split *);
void ogpu_next_split_acquire(ogpu_next_encoder *, ogpu_next_split, const ogpu_next_dependency *);
void ogpu_next_alias_activate(ogpu_next_encoder *, ogpu_next_image *, const ogpu_next_dependency *);

/* Ordinary commands copy immediate parameter records into command storage;
 * resource data and device roots are never copied implicitly. */
typedef struct ogpu_next_image_region {
    uint32_t aspect, mip, first_layer, layer_count;
    ogpu_next_offset offset;
    ogpu_next_extent extent;
} ogpu_next_image_region;
typedef struct ogpu_next_image_copy {
    ogpu_next_image_region region;
    uint64_t row_pitch, slice_pitch;
} ogpu_next_image_copy;
typedef union ogpu_next_clear_value {
    float f32[4]; uint32_t u32[4]; int32_t i32[4];
    struct { float depth; uint32_t stencil; } depth_stencil;
} ogpu_next_clear_value;
void ogpu_next_copy_to_image(ogpu_next_encoder *, ogpu_next_image *dst, ogpu_next_span src, const ogpu_next_image_copy *);
void ogpu_next_copy_from_image(ogpu_next_encoder *, ogpu_next_span dst, ogpu_next_image *src, const ogpu_next_image_copy *);
void ogpu_next_copy_image(ogpu_next_encoder *, ogpu_next_image *dst, const ogpu_next_image_region *, ogpu_next_image *src, const ogpu_next_image_region *);
void ogpu_next_clear_image(ogpu_next_encoder *, ogpu_next_image *, const ogpu_next_subresources *, const ogpu_next_clear_value *);
void ogpu_next_resolve_image(ogpu_next_encoder *, ogpu_next_image *dst, const ogpu_next_image_region *, ogpu_next_image *src, const ogpu_next_image_region *, uint32_t mode);

void ogpu_next_bind_executable(ogpu_next_encoder *, ogpu_next_executable *);
void ogpu_next_bind_heap(ogpu_next_encoder *, uint32_t domain, ogpu_next_heap *);
void ogpu_next_set_inline(ogpu_next_encoder *, ogpu_next_stages, uint32_t offset, uint32_t size, const void *);
void ogpu_next_set_root(ogpu_next_encoder *, ogpu_next_stages, uint32_t slot, ogpu_next_address);
typedef struct ogpu_next_launch {
    ogpu_next_extent groups;
    uint32_t dynamic_shared_bytes;
    const ogpu_next_record *extensions;
} ogpu_next_launch;
void ogpu_next_dispatch(ogpu_next_encoder *, const ogpu_next_launch *);
void ogpu_next_dispatch_indirect(ogpu_next_encoder *, ogpu_next_span args, uint32_t dynamic_shared_bytes);

/* Graphics. Render area is explicit, not inferred from a required attachment.
 * Static/dynamic forms use the same versioned state record schemas. Raster,
 * blend, depth/stencil, viewport/scissor, sample, vertex-input and tile-local
 * state record schemas remain to be finalized; no hidden dynamic-to-PSO JIT. */
typedef struct ogpu_next_attachment {
    ogpu_next_view *view, *resolve_view;
    uint32_t load_op, store_op, resolve_mode;
    ogpu_next_clear_value clear;
} ogpu_next_attachment;
typedef struct ogpu_next_render_desc {
    ogpu_next_record header;
    int32_t x, y;
    uint32_t width, height, layers, view_mask, samples, flags;
    uint32_t color_count;
    const ogpu_next_attachment *colors;
    const ogpu_next_attachment *depth, *stencil;
} ogpu_next_render_desc;
typedef struct ogpu_next_draw_desc {
    uint32_t count, instances, first, first_instance;
    int32_t vertex_offset; /* Indexed only; non-indexed must use zero. */
} ogpu_next_draw_desc;
typedef struct ogpu_next_indirect {
    ogpu_next_span arguments;
    uint32_t stride, maximum_count;
    ogpu_next_span count; /* Null memory means fixed maximum_count. */
} ogpu_next_indirect;
void ogpu_next_render_begin(ogpu_next_encoder *, const ogpu_next_render_desc *);
void ogpu_next_render_end(ogpu_next_encoder *);
void ogpu_next_set_graphics_state(ogpu_next_encoder *, const ogpu_next_record *state);
void ogpu_next_bind_indices(ogpu_next_encoder *, ogpu_next_span, uint32_t index_type);
void ogpu_next_draw(ogpu_next_encoder *, const ogpu_next_draw_desc *);
void ogpu_next_draw_indexed(ogpu_next_encoder *, const ogpu_next_draw_desc *);
void ogpu_next_draw_indirect(ogpu_next_encoder *, const ogpu_next_indirect *);
void ogpu_next_draw_indexed_indirect(ogpu_next_encoder *, const ogpu_next_indirect *);
void ogpu_next_draw_mesh(ogpu_next_encoder *, ogpu_next_extent groups);
void ogpu_next_draw_mesh_indirect(ogpu_next_encoder *, const ogpu_next_indirect *);

/* Queries resolve to application storage without an implicit CPU readback.
 * Query type, counter widths/availability, timestamp domains/wrap and result
 * layout are described by capabilities and pool metadata. */
typedef struct ogpu_next_query_pool_desc {
    ogpu_next_record header;
    uint32_t type, count;
    uint64_t statistics;
} ogpu_next_query_pool_desc;
ogpu_next_status ogpu_next_query_pool_create(ogpu_next_device *, const ogpu_next_query_pool_desc *, ogpu_next_query_pool **);
void ogpu_next_query_pool_destroy(ogpu_next_query_pool *);
void ogpu_next_queries_reset(ogpu_next_encoder *, ogpu_next_query_pool *, uint32_t first, uint32_t count);
void ogpu_next_query_begin(ogpu_next_encoder *, ogpu_next_query_pool *, uint32_t index);
void ogpu_next_query_end(ogpu_next_encoder *, ogpu_next_query_pool *, uint32_t index);
void ogpu_next_timestamp(ogpu_next_encoder *, ogpu_next_query_pool *, uint32_t index, ogpu_next_stages);
void ogpu_next_queries_resolve(ogpu_next_encoder *, ogpu_next_query_pool *, uint32_t first, uint32_t count, ogpu_next_span dst, uint64_t stride, uint32_t flags);
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
