#ifndef OGPU_NEXT_H
#define OGPU_NEXT_H
/* Experimental foundation revision 1, not ABI 20 and not installed by the SDK.
 * Only declarations in THIS header are implemented (Linux Vulkan for now).
 * The remaining design lives in docs/drafts/ogpu_next.h.
 *
 * All pointers are trusted: live, aligned, correctly typed, and readable/writable
 * for the specified extent. Outputs must not overlap inputs or each other.
 * Destruction is immediate and NEVER waits. Children and pending operations must
 * be finished/destroyed before their device. No resource retention is implied.
 * Immutable queries, distinct creations and timeline waits may run concurrently.
 * Device destruction excludes all access. Timeline signal values must obey native
 * monotonic ordering; the application synchronizes conflicting host/GPU signals.
 */
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
#define OGPU_NEXT_VERSION 1u
typedef struct ogpu_next_discovery ogpu_next_discovery;
typedef struct ogpu_next_adapter ogpu_next_adapter;
typedef struct ogpu_next_device ogpu_next_device;
typedef struct ogpu_next_queue ogpu_next_queue;
typedef struct ogpu_next_timeline ogpu_next_timeline;
typedef struct ogpu_next_memory ogpu_next_memory;
typedef struct ogpu_next_image ogpu_next_image;
typedef struct ogpu_next_view ogpu_next_view;
typedef struct ogpu_next_arena ogpu_next_arena;
typedef struct ogpu_next_encoder ogpu_next_encoder;
typedef struct ogpu_next_list ogpu_next_list;
typedef struct ogpu_next_image_barrier ogpu_next_image_barrier;
typedef int32_t ogpu_next_status;
enum {
    OGPU_NEXT_OK = 0, OGPU_NEXT_NOT_READY = 1, OGPU_NEXT_TIMEOUT = 2,
    OGPU_NEXT_INVALID = -1, OGPU_NEXT_UNSUPPORTED = -2,
    OGPU_NEXT_OUT_OF_MEMORY = -3, OGPU_NEXT_DEVICE_LOST = -4,
    OGPU_NEXT_CAPACITY = -5, OGPU_NEXT_BACKEND_ERROR = -6,
    OGPU_NEXT_VERSION_MISMATCH = -7, OGPU_NEXT_INTERNAL_ERROR = -8
};
typedef uint64_t ogpu_next_address;
typedef uint64_t ogpu_next_stages;
typedef uint64_t ogpu_next_access;
typedef uint64_t ogpu_next_features;
typedef uint32_t ogpu_next_format;
typedef uint32_t ogpu_next_image_state;
typedef uint32_t ogpu_next_queue_domain;
typedef struct ogpu_next_record {
    uint32_t kind, version, byte_size, flags;
    const struct ogpu_next_record *next;
} ogpu_next_record;
enum {
    OGPU_NEXT_QUERY_INFO = 1, OGPU_NEXT_QUERY_QUEUES = 2,
    OGPU_NEXT_QUERY_MEMORY_TYPES = 3, OGPU_NEXT_QUERY_MEMORY_HEAPS = 4,
    OGPU_NEXT_QUERY_FEATURES = 5, OGPU_NEXT_QUERY_MEMORY_LIMITS = 6,
    OGPU_NEXT_DEVICE_DESC = 100, OGPU_NEXT_MEMORY_DESC = 101,
    OGPU_NEXT_ARENA_DESC = 102, OGPU_NEXT_RECORDING_DESC = 103,
    OGPU_NEXT_SUBMIT_DESC = 104, OGPU_NEXT_DEPENDENCY = 105,
    OGPU_NEXT_IMAGE_DESC = 106, OGPU_NEXT_VIEW_DESC = 107
};
/* Version 1 records require exact byte_size, flags=0 and next=NULL. Unknown
 * kinds/versions are UNSUPPORTED, not ignored. Later versions may add chains.
 * Query capacity=0 obtains count with OK and no data access. Otherwise capacity
 * must fit the entire result: CAPACITY writes count but no array elements.
 * element_size must exactly match the record type, even for count-only queries.
 * Except OK/CAPACITY, queries leave all outputs unchanged. No query allocates.
 */
typedef struct ogpu_next_query {
    ogpu_next_record header;
    void *data;
    uint32_t capacity, count, element_size, reserved;
} ogpu_next_query;
typedef struct ogpu_next_adapter_info {
    char name[256];
    uint32_t backend, vendor_id, device_id, device_type, native_api_version;
} ogpu_next_adapter_info;
enum {
    OGPU_NEXT_QUEUE_GRAPHICS = 1u, OGPU_NEXT_QUEUE_COMPUTE = 2u,
    OGPU_NEXT_QUEUE_TRANSFER = 4u, OGPU_NEXT_QUEUE_SPARSE = 8u,
    OGPU_NEXT_QUEUE_PROTECTED = 16u
};
typedef struct ogpu_next_queue_info {
    uint32_t domain, flags, count, timestamp_bits;
    uint32_t copy_granularity[3], reserved;
} ogpu_next_queue_info;
enum {
    OGPU_NEXT_MEMORY_LOCAL = 1u, OGPU_NEXT_MEMORY_HOST_VISIBLE = 2u,
    OGPU_NEXT_MEMORY_COHERENT = 4u, OGPU_NEXT_MEMORY_CACHED = 8u,
    OGPU_NEXT_MEMORY_LAZY = 16u, OGPU_NEXT_MEMORY_PROTECTED = 32u,
    OGPU_NEXT_MEMORY_DEVICE_COHERENT = 64u, OGPU_NEXT_MEMORY_DEVICE_UNCACHED = 128u
};
typedef struct ogpu_next_memory_type_info {
    uint32_t id, heap, properties, reserved;
} ogpu_next_memory_type_info;
typedef struct ogpu_next_memory_heap_info {
    uint64_t size; /* Physical heap size, NOT currently available budget. */
    uint32_t id, device_local;
} ogpu_next_memory_heap_info;
typedef struct ogpu_next_memory_limits {
    uint64_t max_buffer_size, max_allocation_size, cache_atom_size, map_alignment;
} ogpu_next_memory_limits;
#define OGPU_NEXT_FEATURE_RASTER UINT64_C(1)
#define OGPU_NEXT_FEATURE_FLOAT16 UINT64_C(2)
#define OGPU_NEXT_FEATURE_UNIFIED_IMAGES UINT64_C(4)
typedef struct ogpu_next_feature_info {
    uint64_t available, enabled, max_timeline_difference;
    uint32_t baseline_supported, device_scope;
} ogpu_next_feature_info;
/* Feature records describe native enabling, not availability of future OGPU
 * draw/dispatch commands. Adapter queries: enabled=0, device_scope=0. Device
 * queries: only explicitly requested optional bits enabled, device_scope=1.
 * Queue flags and memory properties describe physical facilities, not enabled
 * sparse/protected functionality. Domain/type IDs belong to this adapter only.
 */
typedef struct ogpu_next_queue_request {
    ogpu_next_queue_domain domain;
    uint32_t count;
    float priority;
} ogpu_next_queue_request;
typedef struct ogpu_next_device_desc {
    ogpu_next_record header;
    const ogpu_next_queue_request *queues;
    uint32_t queue_request_count, reserved;
    ogpu_next_features required_features;
} ogpu_next_device_desc;
typedef struct ogpu_next_point {
    ogpu_next_timeline *timeline;
    uint64_t value;
} ogpu_next_point;

/* Discovery owns a stable immutable snapshot and its borrowed adapter handles.
 * Destroying discovery invalidates those handles, but not already created
 * devices, which retain only the native connection they need.
 * Create functions clear their handle output on failure. Version is checked
 * before interpreting other versioned records. No process-global current device.
 */
ogpu_next_status ogpu_next_discovery_create(uint32_t version, ogpu_next_discovery **);
uint32_t ogpu_next_discovery_count(const ogpu_next_discovery *);
const ogpu_next_adapter *ogpu_next_discovery_adapter(const ogpu_next_discovery *, uint32_t index);
void ogpu_next_discovery_destroy(ogpu_next_discovery *);
ogpu_next_status ogpu_next_adapter_query(const ogpu_next_adapter *, ogpu_next_query *);
ogpu_next_status ogpu_next_device_create(const ogpu_next_adapter *, const ogpu_next_device_desc *, ogpu_next_device **);
ogpu_next_status ogpu_next_device_query(const ogpu_next_device *, ogpu_next_query *);
/* Exact domain/count selection: no clamping, queue aliasing, family substitution
 * or device-wide host lock. Duplicate domain requests are INVALID; unavailable
 * domain/count/features are UNSUPPORTED. Priorities must be finite in [0,1].
 * Device creation requires >=1 request. Queues remain borrowed until device
 * destruction. Out-of-request domain/index returns NULL. Separate queues can be
 * used concurrently; the application serializes use of each individual queue.
 */
ogpu_next_queue *ogpu_next_device_queue(const ogpu_next_device *, ogpu_next_queue_domain, uint32_t index);
void ogpu_next_device_destroy(ogpu_next_device *);
ogpu_next_status ogpu_next_timeline_create(ogpu_next_device *, uint64_t initial, ogpu_next_timeline **);
void ogpu_next_timeline_destroy(ogpu_next_timeline *);
/* Poll writes completed only on OK; loss is NOT successful completion. Wait
 * returns OK/TIMEOUT/DEVICE_LOST/error; UINT64_MAX requests an infinite timeout.
 * A point does not retain its timeline/device or own a completion receipt.
 */
ogpu_next_status ogpu_next_timeline_poll(const ogpu_next_timeline *, uint64_t *completed);
ogpu_next_status ogpu_next_timeline_wait(ogpu_next_point, uint64_t timeout_ns);
ogpu_next_status ogpu_next_timeline_signal_host(ogpu_next_point);

/* Linear memory is one native addressable backing, partitioned by the caller;
 * spans have no resource object or allocation of their own. Opaque backing is
 * reserved for placed resources: no GPU address, usage must be zero. Mapping
 * either kind requires a host-visible type. No automatic placement/staging.
 */
enum { OGPU_NEXT_MEMORY_LINEAR = 1, OGPU_NEXT_MEMORY_OPAQUE = 2 };
#define OGPU_NEXT_USAGE_COPY_SRC UINT64_C(1)
#define OGPU_NEXT_USAGE_COPY_DST UINT64_C(2)
#define OGPU_NEXT_USAGE_INDIRECT UINT64_C(4)
#define OGPU_NEXT_USAGE_INDEX UINT64_C(8)
#define OGPU_NEXT_USAGE_VERTEX UINT64_C(16)
#define OGPU_NEXT_USAGE_UNIFORM UINT64_C(32)
#define OGPU_NEXT_USAGE_STORAGE UINT64_C(64)
typedef struct ogpu_next_memory_desc {
    ogpu_next_record header;
    uint64_t size, alignment, usage;
    uint32_t memory_type, kind;
    const uint32_t *concurrent_domains;
    uint32_t concurrent_domain_count, flags;
} ogpu_next_memory_desc;
typedef struct ogpu_next_span {
    ogpu_next_memory *memory;
    uint64_t offset, size;
} ogpu_next_span;
typedef struct ogpu_next_requirements {
    uint64_t size, alignment;
    uint32_t dedicated_required, dedicated_preferred;
    uint32_t compatible_type_count, compatible_type_capacity;
    uint32_t *compatible_types;
} ogpu_next_requirements;
typedef struct ogpu_next_mapping {
    void *data;
    uint64_t size, cache_atom_size, cache_offset;
    uint32_t coherent, reserved;
} ogpu_next_mapping;
/* size>0; alignment is a power of two (opaque requires 1). Linear alignment is
 * a GPU-address guarantee, implemented with bounded leading backing padding.
 * Requirements include this overhead. memory_type is ignored by requirements,
 * but creation selects exactly that compatible type. No first-fit substitution.
 * Requirements fill size/alignment/dedicated metadata/count even on CAPACITY;
 * capacity=0 is a successful count query, otherwise no partial type-array write.
 * Other errors leave output unchanged. requirements never allocates GPU storage.
 * Concurrent domains: 0 means exclusive sharing; >=2 unique valid adapter domains
 * means explicit concurrent sharing. One domain is INVALID, not silently ignored.
 * Opaque backing has no sharing/usage flags; placed resources specify their own.
 * Alignment/usage/sharing records are checked before native calls. flags=0.
 */
ogpu_next_status ogpu_next_memory_requirements(const ogpu_next_device *, const ogpu_next_memory_desc *, ogpu_next_requirements *);
ogpu_next_status ogpu_next_memory_create(ogpu_next_device *, const ogpu_next_memory_desc *, ogpu_next_memory **);
void ogpu_next_memory_destroy(ogpu_next_memory *);
ogpu_next_status ogpu_next_memory_address(ogpu_next_span, ogpu_next_address *);
/* Mapping is persistent and borrowed. Repeated maps share one whole-allocation
 * native mapping; unmap invalidates ALL views. Map/unmap/destroy on one memory
 * object are externally serialized against each other and all view/cache users.
 * Independent view ranges may be accessed concurrently, subject to hazards.
 * Coherent => atom=1, offset=0. Otherwise cache_offset is this view's first-byte
 * offset within a native cache atom. Flush/invalidate round out to atoms; callers
 * must synchronize neighboring bytes in those atoms too. No implicit waits.
 * Address/range/cache outputs are unchanged on error. Address permits a bounded
 * empty range; map requires nonempty range. GPU sizes aren't limited by size_t;
 * an allocation too large for a host view returns UNSUPPORTED when mapped.
 */
ogpu_next_status ogpu_next_memory_map(ogpu_next_span, ogpu_next_mapping *);
void ogpu_next_memory_unmap(ogpu_next_memory *);
ogpu_next_status ogpu_next_memory_flush(ogpu_next_span);
ogpu_next_status ogpu_next_memory_invalidate(ogpu_next_span);

/* Image format IDs are OGPU values, not backend enum passthrough. This initial
 * table is a supported subset, not the full format/capability surface. */
enum {
    OGPU_NEXT_R8_UNORM = 1, OGPU_NEXT_RG8_UNORM = 2,
    OGPU_NEXT_RGBA8_UNORM = 3, OGPU_NEXT_RGBA8_SRGB = 4,
    OGPU_NEXT_BGRA8_UNORM = 5, OGPU_NEXT_BGRA8_SRGB = 6,
    OGPU_NEXT_R16_FLOAT = 7, OGPU_NEXT_RG16_FLOAT = 8, OGPU_NEXT_RGBA16_FLOAT = 9,
    OGPU_NEXT_R32_FLOAT = 10, OGPU_NEXT_R32_UINT = 11, OGPU_NEXT_RGBA32_FLOAT = 12,
    OGPU_NEXT_D16_UNORM = 13, OGPU_NEXT_D32_FLOAT = 14,
    OGPU_NEXT_D24_UNORM_S8_UINT = 15, OGPU_NEXT_D32_FLOAT_S8_UINT = 16
};
enum { OGPU_NEXT_IMAGE_1D = 1, OGPU_NEXT_IMAGE_2D = 2, OGPU_NEXT_IMAGE_3D = 3 };
enum { OGPU_NEXT_ASPECT_COLOR = 1, OGPU_NEXT_ASPECT_DEPTH = 2, OGPU_NEXT_ASPECT_STENCIL = 4 };
#define OGPU_NEXT_IMAGE_COPY_SRC UINT64_C(1)
#define OGPU_NEXT_IMAGE_COPY_DST UINT64_C(2)
#define OGPU_NEXT_IMAGE_SAMPLED UINT64_C(4)
#define OGPU_NEXT_IMAGE_STORAGE UINT64_C(8)
#define OGPU_NEXT_IMAGE_COLOR_ATTACHMENT UINT64_C(16)
#define OGPU_NEXT_IMAGE_DEPTH_STENCIL_ATTACHMENT UINT64_C(32)
#define OGPU_NEXT_IMAGE_TRANSIENT UINT64_C(64)
#define OGPU_NEXT_IMAGE_INPUT_ATTACHMENT UINT64_C(128)
enum { OGPU_NEXT_IMAGE_MUTABLE_FORMAT = 1, OGPU_NEXT_IMAGE_CUBE_COMPATIBLE = 2, OGPU_NEXT_IMAGE_ALIAS = 4 };
typedef struct ogpu_next_extent { uint32_t x, y, z; } ogpu_next_extent;
typedef struct ogpu_next_image_desc {
    ogpu_next_record header;
    ogpu_next_format format;
    uint32_t dimension, mip_count, layer_count, sample_count;
    ogpu_next_extent extent;
    uint64_t usage;
    uint32_t flags;
    const ogpu_next_format *view_formats;
    uint32_t view_format_count, concurrent_domain_count;
    const ogpu_next_queue_domain *concurrent_domains;
} ogpu_next_image_desc;
typedef struct ogpu_next_subresources {
    uint32_t aspects, first_mip, mip_count, first_layer, layer_count;
} ogpu_next_subresources;
enum {
    OGPU_NEXT_VIEW_1D = 1, OGPU_NEXT_VIEW_2D = 2, OGPU_NEXT_VIEW_3D = 3,
    OGPU_NEXT_VIEW_CUBE = 4, OGPU_NEXT_VIEW_1D_ARRAY = 5, OGPU_NEXT_VIEW_2D_ARRAY = 6,
    OGPU_NEXT_VIEW_CUBE_ARRAY = 7
};
enum {
    OGPU_NEXT_COMPONENT_IDENTITY = 0, OGPU_NEXT_COMPONENT_ZERO = 1,
    OGPU_NEXT_COMPONENT_ONE = 2, OGPU_NEXT_COMPONENT_R = 3,
    OGPU_NEXT_COMPONENT_G = 4, OGPU_NEXT_COMPONENT_B = 5, OGPU_NEXT_COMPONENT_A = 6
};
typedef struct ogpu_next_view_desc {
    ogpu_next_record header;
    ogpu_next_format format;
    uint32_t dimension, usage, component_mapping[4];
    ogpu_next_subresources range;
} ogpu_next_view_desc;
/* Images use optimal GPU tiling and start UNDEFINED: binding never initializes
 * texels or performs a transition. Requirements queries create no GPU object or
 * allocation; they follow memory_requirements output/capacity rules. They check
 * the exact format/usage/dimensions/sample/flag tuple; no silent fallback.
 * Placement uses opaque memory of a compatible type with offset aligned to the
 * image's native requirement and size >= required size. Multiple optimal images
 * can occupy disjoint spans of one allocation. The caller owns nonoverlap, alias
 * activation/hazards and resource lifetimes; ALIAS explicitly permits alias use.
 * Concurrent domains follow memory_desc rules, independently of backing.
 * View-format lists restrict permitted interpretations. Reinterpretation needs
 * MUTABLE_FORMAT and a compatible format class; depth/stencil keeps its format.
 * This revision excludes linear tiling, sparse/external/disjoint images, extended
 * usage, cube-array views, 2D views of 3D slices and multisampled storage images.
 * Such strategies are UNSUPPORTED, never emulated; broader profiles follow.
 */
ogpu_next_status ogpu_next_image_requirements(const ogpu_next_device *, const ogpu_next_image_desc *, ogpu_next_requirements *);
ogpu_next_status ogpu_next_image_create(ogpu_next_device *, const ogpu_next_image_desc *, ogpu_next_span placement, ogpu_next_image **);
/* Explicit two-phase creation supports native dedicated-allocation association.
 * Unbound images cannot create views or execute GPU work. Binding is one-shot.
 * Dedicated allocation names the unbound image and exact memory type, returns
 * a separate memory owner, and does NOT bind. It can only bind that image, at
 * offset zero. Use this path when dedicated_required, or by caller policy.
 * No image or view retains its backing/parents, and destruction never waits.
 */
ogpu_next_status ogpu_next_image_create_unbound(ogpu_next_device *, const ogpu_next_image_desc *, ogpu_next_image **);
ogpu_next_status ogpu_next_memory_create_dedicated_image(ogpu_next_image *, uint32_t memory_type, ogpu_next_memory **);
ogpu_next_status ogpu_next_image_bind(ogpu_next_image *, ogpu_next_span placement);
void ogpu_next_image_destroy(ogpu_next_image *);
/* usage is an explicit nonzero subset of the image's sampled/storage/attachment
 * usages. Dimensions/aspects/mips/layers are checked. Storage/attachment views
 * require identity component mappings. Binding and lifetime changes exclude all
 * uses; independent view creation on a bound image may proceed concurrently. */
ogpu_next_status ogpu_next_view_create(ogpu_next_image *, const ogpu_next_view_desc *, ogpu_next_view **);
void ogpu_next_view_destroy(ogpu_next_view *);

/* Arena capacity counts primary list slots, NOT native command bytes. Reserve
 * grows explicitly; begin never grows OGPU storage. Native recording/submission
 * can still allocate internally. All host use of a pool and its encoders is
 * externally serialized; use distinct arenas for parallel recording. Immutable
 * executable lists may be submitted independently of other slots' recording.
 * Reset/trim/destroy require no pending uses and invalidate ALL list/encoder
 * handles, even on a failed reset; failed reset requires successful reset before
 * recording again. Reset retains slots; trim resets then frees slots above retained_count
 * and asks the driver to release unused pool storage (no byte-budget guarantee).
 * Cancel and failed begin/end consume their slot until reset; cancel does not
 * wait. An encoder error is sticky and reported by end with a NULL list.
 */
typedef struct ogpu_next_arena_desc {
    ogpu_next_record header;
    ogpu_next_queue_domain domain;
    uint32_t list_capacity;
} ogpu_next_arena_desc;
enum { OGPU_NEXT_ONE_SHOT = 1, OGPU_NEXT_SERIAL_REPLAY = 2, OGPU_NEXT_SIMULTANEOUS_REPLAY = 3 };
enum { OGPU_NEXT_PRIMARY = 0 };
typedef struct ogpu_next_recording_desc {
    ogpu_next_record header;
    uint32_t replay_mode, level;
    const ogpu_next_record *inheritance;
} ogpu_next_recording_desc;
/* Only primary, inheritance=NULL is implemented. One-shot accepts one successful
 * submit. Serial replay requires prior executions complete before resubmission;
 * simultaneous replay permits pending executions but does NOT resolve hazards.
 * The caller tracks pending use and keeps referenced resources alive, including
 * between replays. No resource lists, pending scans or implicit timeline values.
 */
ogpu_next_status ogpu_next_arena_create(ogpu_next_device *, const ogpu_next_arena_desc *, ogpu_next_arena **);
ogpu_next_status ogpu_next_arena_reserve(ogpu_next_arena *, uint32_t list_capacity);
ogpu_next_status ogpu_next_arena_reset(ogpu_next_arena *);
ogpu_next_status ogpu_next_arena_trim(ogpu_next_arena *, uint32_t retained_count);
void ogpu_next_arena_destroy(ogpu_next_arena *);
ogpu_next_status ogpu_next_commands_begin(ogpu_next_arena *, const ogpu_next_recording_desc *, ogpu_next_encoder **);
ogpu_next_status ogpu_next_commands_end(ogpu_next_encoder *, ogpu_next_list **);
void ogpu_next_commands_cancel(ogpu_next_encoder *);

#define OGPU_NEXT_STAGE_ALL UINT64_C(1)
/* COPY covers all native transfer operations (copy/fill/clear/resolve); VERTEX
 * covers vertex input and vertex shader; DEPTH covers early and late tests.
 * Finer graphics scopes and optional-stage capabilities accompany that surface. */
#define OGPU_NEXT_STAGE_COPY UINT64_C(2)
#define OGPU_NEXT_STAGE_COMPUTE UINT64_C(4)
#define OGPU_NEXT_STAGE_VERTEX UINT64_C(8)
#define OGPU_NEXT_STAGE_FRAGMENT UINT64_C(16)
#define OGPU_NEXT_STAGE_INDIRECT UINT64_C(32)
#define OGPU_NEXT_STAGE_COLOR UINT64_C(64)
#define OGPU_NEXT_STAGE_DEPTH UINT64_C(128)
#define OGPU_NEXT_STAGE_HOST UINT64_C(256)
#define OGPU_NEXT_ACCESS_READ UINT64_C(1)
#define OGPU_NEXT_ACCESS_WRITE UINT64_C(2)
#define OGPU_NEXT_ACCESS_COPY_READ UINT64_C(4)
#define OGPU_NEXT_ACCESS_COPY_WRITE UINT64_C(8)
#define OGPU_NEXT_ACCESS_SHADER_READ UINT64_C(16)
#define OGPU_NEXT_ACCESS_SHADER_WRITE UINT64_C(32)
#define OGPU_NEXT_ACCESS_HOST_READ UINT64_C(64)
#define OGPU_NEXT_ACCESS_HOST_WRITE UINT64_C(128)
#define OGPU_NEXT_ACCESS_INDIRECT_READ UINT64_C(256)
#define OGPU_NEXT_ACCESS_INDEX_READ UINT64_C(512)
#define OGPU_NEXT_ACCESS_VERTEX_READ UINT64_C(1024)
#define OGPU_NEXT_ACCESS_UNIFORM_READ UINT64_C(2048)
#define OGPU_NEXT_ACCESS_COLOR_READ UINT64_C(4096)
#define OGPU_NEXT_ACCESS_COLOR_WRITE UINT64_C(8192)
#define OGPU_NEXT_ACCESS_DEPTH_READ UINT64_C(16384)
#define OGPU_NEXT_ACCESS_DEPTH_WRITE UINT64_C(32768)
typedef struct ogpu_next_host_requirements { uint64_t size, alignment; } ogpu_next_host_requirements;
typedef struct ogpu_next_sync_point { ogpu_next_point point; ogpu_next_stages stages; } ogpu_next_sync_point;
typedef struct ogpu_next_submit_desc {
    ogpu_next_record header;
    uint32_t list_count, wait_count, signal_count;
    ogpu_next_list *const *lists;
    const ogpu_next_sync_point *waits, *signals;
    void *scratch;
    uint64_t scratch_size;
} ogpu_next_submit_desc;
/* Scratch is writable aligned host storage, borrowed only during the call and
 * disjoint from all inputs/objects. Query size/alignment once and reuse it; no
 * OGPU hot-path heap allocation. Zero-size queries return alignment=1. Too-small
 * scratch => CAPACITY; null/misaligned nonempty storage => INVALID. Queries leave
 * output unchanged on error. Lists/timelines must belong to the queue's device;
 * lists must match its domain. Each queue is externally serialized. The caller
 * guarantees duplicate lists use simultaneous mode, no duplicate wait or signal
 * timelines, and signals exceed same-timeline waits. No quadratic duplicate scan.
 * One-shot submission excludes every other host access to that list.
 * Wait/signal stage masks are preserved, HOST is invalid here. Only an ALL signal
 * after the relevant work proves whole-list completion for reuse/destruction.
 * No automatic signal, wait or submission receipt. OOM is retryable without
 * consuming one-shot lists; loss is terminal. Other native submission errors
 * conservatively poison the device (return BACKEND_ERROR, then DEVICE_LOST).
 */
ogpu_next_status ogpu_next_submit_scratch_requirements(uint32_t lists, uint32_t waits, uint32_t signals, ogpu_next_host_requirements *);
ogpu_next_status ogpu_next_queue_submit(ogpu_next_queue *, const ogpu_next_submit_desc *);

#define OGPU_NEXT_DOMAIN_IGNORED UINT32_MAX
typedef struct ogpu_next_memory_barrier {
    ogpu_next_span range;
    ogpu_next_access before, after;
    ogpu_next_queue_domain source_domain, destination_domain;
} ogpu_next_memory_barrier;
enum {
    OGPU_NEXT_STATE_UNDEFINED = 0, OGPU_NEXT_STATE_GENERAL = 1,
    OGPU_NEXT_STATE_COPY_SRC = 2, OGPU_NEXT_STATE_COPY_DST = 3,
    OGPU_NEXT_STATE_SHADER_READ = 4, OGPU_NEXT_STATE_COLOR_ATTACHMENT = 5,
    OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT = 6, OGPU_NEXT_STATE_DEPTH_STENCIL_READ = 7
};
struct ogpu_next_image_barrier {
    ogpu_next_image *image;
    ogpu_next_subresources range;
    ogpu_next_access before, after;
    ogpu_next_image_state old_state, new_state;
    ogpu_next_queue_domain source_domain, destination_domain;
    uint32_t discard;
};
typedef struct ogpu_next_dependency {
    ogpu_next_record header;
    ogpu_next_stages before, after;
    ogpu_next_access global_before, global_after;
    uint32_t memory_count, image_count, flags;
    const ogpu_next_memory_barrier *memory;
    const ogpu_next_image_barrier *images;
    void *scratch;
    uint64_t scratch_size;
} ogpu_next_dependency;
/* One native batched dependency: global scopes plus optional backing ranges.
 * Source/destination both IGNORED => no ownership transfer. Otherwise explicit
 * exclusive-family release/acquire endpoints require matching ranges/domains and
 * a queue timeline edge. No implicit owner tracking. Empty memory ranges invalid.
 * Stage NONE (0) requires access NONE; ALL includes GPU stages, not HOST.
 * Image states are explicit native layouts, never a tracked current state. The
 * caller proves old_state and consistent state at execution/replay. discard=1
 * discards contents via UNDEFINED; it does NOT remove synchronization obligations.
 * new_state cannot be UNDEFINED. Depth/stencil combined formats currently require
 * both aspects in transitions (separate-layout enabling follows). flags=0.
 */
ogpu_next_status ogpu_next_barrier_scratch_requirements(uint32_t memory_count, uint32_t image_count, ogpu_next_host_requirements *);
void ogpu_next_barrier(ogpu_next_encoder *, const ogpu_next_dependency *);
/* Copies require equal sizes, COPY_SRC/DST usage, no overlapping byte ranges.
 * Fill requires COPY_DST and offset/size multiples of four. Empty valid ranges
 * are no-ops. Bounds/device/usage mistakes poison the encoder, not partial work.
 * Commands borrow resources; they never stage, allocate, retain or wait.
 */
void ogpu_next_copy_memory(ogpu_next_encoder *, ogpu_next_span dst, ogpu_next_span src);
void ogpu_next_fill_memory(ogpu_next_encoder *, ogpu_next_span dst, uint32_t pattern);
typedef struct ogpu_next_offset { int32_t x, y, z; } ogpu_next_offset;
typedef struct ogpu_next_image_region {
    uint32_t aspect, mip, first_layer, layer_count;
    ogpu_next_offset offset;
    ogpu_next_extent extent;
} ogpu_next_image_region;
typedef struct ogpu_next_image_copy {
    ogpu_next_image_region region;
    uint64_t row_pitch, slice_pitch;
    ogpu_next_image_state state;
    uint32_t reserved;
} ogpu_next_image_copy;
typedef union ogpu_next_clear_value {
    float f32[4]; uint32_t u32[4]; int32_t i32[4];
    struct { float depth; uint32_t stencil; } depth_stencil;
} ogpu_next_clear_value;
/* Commands never insert transitions or hazard barriers. Clear requires GENERAL
 * or COPY_DST; copies require GENERAL or the corresponding COPY_SRC/COPY_DST.
 * Single-sample copies support one aspect and explicit mip/layer/subregion bounds.
 * Pitches are bytes: zero means tight, row is a texel multiple, slice is a row
 * multiple. Native queue granularity and address alignment are checked. Row/slice
 * texel counts must fit native uint32 fields; row bytes must fit INT32_MAX.
 * reserved=0. No implicit staging.
 * Color clear needs graphics/compute; depth/stencil clear/copy currently needs a
 * graphics family. Select the clear-value member matching the image's numeric
 * format (float for normalized/float, uint for unsigned, depth_stencil for depth).
 * Depth clear values must be finite and in [0,1], including stencil-only clears.
 */
void ogpu_next_copy_to_image(ogpu_next_encoder *, ogpu_next_image *dst, ogpu_next_span src, const ogpu_next_image_copy *);
void ogpu_next_copy_from_image(ogpu_next_encoder *, ogpu_next_span dst, ogpu_next_image *src, const ogpu_next_image_copy *);
void ogpu_next_clear_image(ogpu_next_encoder *, ogpu_next_image *, ogpu_next_image_state, const ogpu_next_subresources *, const ogpu_next_clear_value *);
#ifdef __cplusplus
}
#endif
#endif
