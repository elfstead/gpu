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
typedef struct ogpu_next_executable ogpu_next_executable;
typedef struct ogpu_next_query_pool ogpu_next_query_pool;
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
    OGPU_NEXT_QUERY_DESCRIPTOR_LIMITS = 7,
    OGPU_NEXT_QUERY_EXECUTION_LIMITS = 8,
    OGPU_NEXT_QUERY_GRAPHICS_LIMITS = 9,
    OGPU_NEXT_QUERY_LIMITS = 10,
    OGPU_NEXT_DEVICE_DESC = 100, OGPU_NEXT_MEMORY_DESC = 101,
    OGPU_NEXT_ARENA_DESC = 102, OGPU_NEXT_RECORDING_DESC = 103,
    OGPU_NEXT_SUBMIT_DESC = 104, OGPU_NEXT_DEPENDENCY = 105,
    OGPU_NEXT_IMAGE_DESC = 106, OGPU_NEXT_VIEW_DESC = 107,
    OGPU_NEXT_SAMPLER_DESC = 108, OGPU_NEXT_HEAP_BINDING = 109,
    OGPU_NEXT_EXECUTABLE_DESC = 110, OGPU_NEXT_ARGUMENT_INTERFACE = 111,
    OGPU_NEXT_SHADER_REQUIREMENTS = 112, OGPU_NEXT_SPECIALIZATION = 113,
    OGPU_NEXT_GRAPHICS_STATE = 114, OGPU_NEXT_RENDER_DESC = 115,
    OGPU_NEXT_VIEWPORT_STATE = 116, OGPU_NEXT_QUERY_POOL_DESC = 117
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
#define OGPU_NEXT_FEATURE_SAMPLER_ANISOTROPY UINT64_C(8)
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
#define OGPU_NEXT_USAGE_DESCRIPTOR_HEAP UINT64_C(128)
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
#define OGPU_NEXT_ACCESS_RESOURCE_HEAP_READ UINT64_C(65536)
#define OGPU_NEXT_ACCESS_SAMPLER_HEAP_READ UINT64_C(131072)
typedef struct ogpu_next_host_requirements { uint64_t size, alignment; } ogpu_next_host_requirements;
/* Descriptor bytes are opaque and device-specific, not a serializable ABI.
 * No owned heap/slot table: encode into host storage, place/copy into linear
 * DESCRIPTOR_HEAP memory, and bind a caller-selected range. Resource heaps may
 * mix descriptor types and sizes; shader metadata must match offsets/strides.
 * Size fields bound each encoding's output. Descriptor alignments apply to GPU
 * placement, not host output pointers. Heap base and reservation alignments are
 * separate constraints. For native SPIR-V heap arrays using OpConstantSizeOfEXT,
 * stride is the descriptor size rounded up to its corresponding alignment, NOT
 * necessarily the raw encoding size or an application's mixed-slot stride.
 * Limits describe this device, not portable constants. */
typedef struct ogpu_next_descriptor_limits {
    uint64_t buffer_size, buffer_alignment, image_size, image_alignment;
    uint64_t sampler_size, sampler_alignment;
    uint64_t resource_heap_alignment, resource_heap_max_size;
    uint64_t resource_reserved_size, resource_reserved_alignment;
    uint64_t sampler_heap_alignment, sampler_heap_max_size;
    uint64_t sampler_reserved_size, sampler_reserved_alignment;
    uint64_t uniform_address_alignment, storage_address_alignment;
    uint64_t max_uniform_range, max_storage_range;
    float max_sampler_lod_bias, max_sampler_anisotropy;
} ogpu_next_descriptor_limits;
typedef struct ogpu_next_host_span { void *data; uint64_t size; } ogpu_next_host_span;
enum { OGPU_NEXT_HEAP_RESOURCE = 1, OGPU_NEXT_HEAP_SAMPLER = 2 };
enum {
    OGPU_NEXT_DESCRIPTOR_SAMPLED_IMAGE = 1, OGPU_NEXT_DESCRIPTOR_STORAGE_IMAGE = 2,
    OGPU_NEXT_DESCRIPTOR_STORAGE_BUFFER = 3, OGPU_NEXT_DESCRIPTOR_UNIFORM_BUFFER = 4,
    OGPU_NEXT_DESCRIPTOR_INPUT_ATTACHMENT = 5
};
typedef struct ogpu_next_resource_descriptor {
    uint32_t kind;
    ogpu_next_image_state image_state;
    ogpu_next_view *view;
    ogpu_next_span buffer;
} ogpu_next_resource_descriptor;
enum { OGPU_NEXT_FILTER_NEAREST = 0, OGPU_NEXT_FILTER_LINEAR = 1 };
enum {
    OGPU_NEXT_ADDRESS_REPEAT = 0, OGPU_NEXT_ADDRESS_MIRRORED_REPEAT = 1,
    OGPU_NEXT_ADDRESS_CLAMP_EDGE = 2, OGPU_NEXT_ADDRESS_CLAMP_BORDER = 3
};
enum {
    OGPU_NEXT_COMPARE_NEVER = 0, OGPU_NEXT_COMPARE_LESS = 1,
    OGPU_NEXT_COMPARE_EQUAL = 2, OGPU_NEXT_COMPARE_LESS_EQUAL = 3,
    OGPU_NEXT_COMPARE_GREATER = 4, OGPU_NEXT_COMPARE_NOT_EQUAL = 5,
    OGPU_NEXT_COMPARE_GREATER_EQUAL = 6, OGPU_NEXT_COMPARE_ALWAYS = 7
};
enum {
    OGPU_NEXT_BORDER_FLOAT_TRANSPARENT_BLACK = 0, OGPU_NEXT_BORDER_INT_TRANSPARENT_BLACK = 1,
    OGPU_NEXT_BORDER_FLOAT_OPAQUE_BLACK = 2, OGPU_NEXT_BORDER_INT_OPAQUE_BLACK = 3,
    OGPU_NEXT_BORDER_FLOAT_OPAQUE_WHITE = 4, OGPU_NEXT_BORDER_INT_OPAQUE_WHITE = 5
};
typedef struct ogpu_next_sampler_desc {
    ogpu_next_record header;
    uint32_t min_filter, mag_filter, mip_filter, address_mode[3];
    uint32_t compare_enable, compare_op, border_color, unnormalized_coordinates;
    float min_lod, max_lod, lod_bias, max_anisotropy;
} ogpu_next_sampler_desc;
/* Resource descriptors: image kinds require buffer={NULL,0,0}; buffer kinds
 * require view=NULL, image_state=0 and a nonempty UNIFORM/STORAGE span. Image
 * views require matching usage and one aspect. Storage uses GENERAL; sampled/
 * input uses GENERAL, SHADER_READ or DEPTH_STENCIL_READ. Encoding does not
 * transition images. Input-attachment execution/local reads remain unimplemented.
 * Samplers: booleans are 0/1, all floats finite, min_lod<=max_lod, anisotropy=1
 * disables it; >1 needs explicitly enabled SAMPLER_ANISOTROPY. Unnormalized
 * requires equal min/mag filters, nearest mip, LODs=0, compare off, anisotropy=1,
 * and U/V clamp-edge or clamp-border. Unsupported modes never fall back.
 *
 * Writers preflight the whole batch before one native call. Local errors leave
 * destinations unchanged; native failure may partially write them. Scratch is
 * unspecified on failure. Outputs and scratch are mutually disjoint and disjoint
 * from all inputs. Scratch is caller-owned, aligned per query, borrowed only for
 * this call. count=0 permits NULL arrays and empty scratch. No OGPU allocation,
 * flush, copy, resource retention or implicit synchronization. Concurrent writes
 * to disjoint host ranges are allowed. Padding bytes need not be deterministic.
 * Ordinary host/GPU copies may relocate bytes within the same live device;
 * referenced resources must remain valid through all descriptor consumption.
 */
ogpu_next_status ogpu_next_descriptor_scratch_requirements(uint32_t kind, uint32_t count, ogpu_next_host_requirements *);
ogpu_next_status ogpu_next_write_resource_descriptors(const ogpu_next_device *, uint32_t count, const ogpu_next_resource_descriptor *, const ogpu_next_host_span *outputs, ogpu_next_host_span scratch);
ogpu_next_status ogpu_next_write_sampler_descriptors(const ogpu_next_device *, uint32_t count, const ogpu_next_sampler_desc *, const ogpu_next_host_span *outputs, ogpu_next_host_span scratch);
typedef struct ogpu_next_heap_binding {
    ogpu_next_record header;
    uint32_t kind, flags;
    ogpu_next_span storage;
    uint64_t reserved_offset, reserved_size;
} ogpu_next_heap_binding;
/* Bind a GPU range without allocating, copying or retaining anything. flags=0.
 * Reservation is relative to storage, inside it, and at least the queried size.
 * Do not access reserved bytes from recording until ALL referencing command
 * buffers are reset/destroyed, even after GPU completion. Reservations must not
 * overlap unless exactly identical and of the same heap kind. Caller proves
 * these rules; no global registry or heap-wide lock. Nonreserved descriptors
 * can be updated range-locally, with correct host/GPU hazards and cache-atom
 * isolation from reserved/in-use bytes. Explicit flush and COPY->heap-read
 * dependencies remain the caller's job. Shader access must remain in bounds and
 * avoid reservations. Bind requires a graphics/compute queue domain. */
void ogpu_next_bind_heap(ogpu_next_encoder *, const ogpu_next_heap_binding *);

/* Prepared executable state. This implementation supports compute or paired
 * vertex/fragment SPIR-V with native heap/pointer access. Native artifacts, descriptor
 * set-to-heap mappings, cache import/export and launch extensions follow; these
 * are unimplemented profiles, not excluded designs. No runtime shader translation.
 * Preparation may allocate/compile. Binding, arguments and dispatch never allocate
 * OGPU storage, compile, retain resources, upload pointees or insert dependencies.
 */
enum { OGPU_NEXT_EXECUTABLE_COMPUTE = 1, OGPU_NEXT_EXECUTABLE_GRAPHICS = 2 };
enum { OGPU_NEXT_SHADER_SPIRV = 0 };
enum { OGPU_NEXT_ARGUMENT_SHARED_BYTES = 1 };
typedef struct ogpu_next_execution_limits {
    uint64_t max_inline_size;
    uint32_t max_groups[3], max_local_size[3], max_local_invocations;
    uint32_t max_shared_memory, argument_flags;
} ogpu_next_execution_limits;
typedef struct ogpu_next_bytes { const void *data; size_t size; } ogpu_next_bytes;
typedef struct ogpu_next_root_slot {
    ogpu_next_stages stages;
    uint32_t offset, alignment;
} ogpu_next_root_slot;
typedef struct ogpu_next_argument_interface {
    ogpu_next_record header;
    uint32_t byte_size, root_count;
    const ogpu_next_root_slot *roots;
} ogpu_next_argument_interface;
typedef struct ogpu_next_shader_requirements {
    ogpu_next_record header;
    ogpu_next_features features;
    uint32_t local_size[3], shared_memory;
} ogpu_next_shader_requirements;
typedef struct ogpu_next_specialization_entry { uint32_t id, offset; uint64_t size; } ogpu_next_specialization_entry;
typedef struct ogpu_next_specialization {
    ogpu_next_record header;
    uint32_t count, reserved;
    const ogpu_next_specialization_entry *entries;
    ogpu_next_bytes data;
} ogpu_next_specialization;
typedef struct ogpu_next_shader {
    uint32_t stage, format;
    ogpu_next_bytes code;
    const char *entry;
    const ogpu_next_record *interface_metadata, *specialization;
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
/* stage=COMPUTE; kind=COMPUTE; shader_count=1; static_state=NULL, dynamic_state=0,
 * native_cache={NULL,0}. Explicit nonempty UTF-8 entry name (not forced to main).
 * code is aligned SPIR-V words, valid for this device and its enabled features.
 * interface_metadata names ARGUMENT_INTERFACE; requirements names
 * SHADER_REQUIREMENTS. Both are mandatory even when no argument bytes are used.
 * Shader semantics and metadata agreement are trusted: callers supply the actual
 * post-specialization local size and shared-memory use. This is not a SPIR-V
 * validator/reflection engine. Capabilities, memory safety, uniformity promises
 * and every reachable GPU access remain the artifact producer/caller's contract.
 * Set/binding resource variables are not supported yet; direct heap access is.
 * Optional SPECIALIZATION entries have unique IDs, nonempty bounded byte spans,
 * and sizes/types matching the shader (including 4-byte native bools); reserved=0.
 * Inputs are borrowed during creation only; argument metadata is copied. Device
 * is borrowed through destruction; no current context or shared pipeline cache.
 */
ogpu_next_status ogpu_next_executable_create(ogpu_next_device *, const ogpu_next_executable_desc *, ogpu_next_executable **);
void ogpu_next_executable_destroy(ogpu_next_executable *);
/* Bind borrows executable through all recorded future/pending use. COMPUTE only
 * and requires a compute domain. Inline/root commands require a bound executable.
 * byte_size is a multiple of four <= queried maximum. Root slots are array IDs,
 * each occupying 8 bytes at an 8-aligned offset within byte_size, with nonoverlap,
 * COMPUTE visibility and power-of-two pointee alignment. No fixed single root.
 * SHARED_BYTES means inline and roots address one byte namespace, not separate
 * stage banks. set_root changes only its 8 bytes; inline may intentionally change
 * those same bytes. The caller initializes every byte read by each dispatch.
 * Binding preserves bytes at identical offsets, even between different executables;
 * only compatible meanings may be reused. Begin has undefined argument contents.
 * offset/size must be multiples of four and in bounds; empty inline is a no-op.
 * Input host bytes are copied during recording, address pointees are never copied.
 * Null addresses can be encoded but must not be dereferenced. Address alignment,
 * lifetime, bounds and hazards apply at execution, including every replay.
 * Stage-isolated banks are NOT emulated with hidden copies on this profile.
 * Graphics extends this below: VERTEX/FRAGMENT share the same native bytes,
 * including with compute. A stage mask does not isolate an update from other
 * stages reading the same offsets. Use distinct offsets for independent values.
 */
void ogpu_next_bind_executable(ogpu_next_encoder *, ogpu_next_executable *);
void ogpu_next_set_inline(ogpu_next_encoder *, ogpu_next_stages, uint32_t offset, uint32_t size, const void *);
void ogpu_next_set_root(ogpu_next_encoder *, ogpu_next_stages, uint32_t slot, ogpu_next_address);
typedef struct ogpu_next_launch {
    ogpu_next_extent groups;
    uint32_t dynamic_shared_bytes;
    const ogpu_next_record *extensions;
} ogpu_next_launch;
/* Group counts are direct native dimensions, including zero-work dimensions.
 * dynamic_shared_bytes=0 and extensions=NULL in this profile. Other strategies
 * are UNSUPPORTED, never specialized/JIT-compiled at command time. Indirect uses
 * the first three u32 dimensions at a 4-aligned INDIRECT span of >=12 bytes;
 * the caller proves execution-time dimensions obey limits. No CPU readback/check,
 * no hidden barrier, and no second allocation for arguments. */
void ogpu_next_dispatch(ogpu_next_encoder *, const ogpu_next_launch *);
void ogpu_next_dispatch_indirect(ogpu_next_encoder *, ogpu_next_span args, uint32_t dynamic_shared_bytes);

typedef struct ogpu_next_graphics_limits {
    uint32_t max_colors, max_width, max_height, max_layers;
    uint32_t max_viewport[2]; float viewport_bounds[2];
    uint32_t color_samples, depth_samples, no_attachment_samples, max_indirect_count;
} ogpu_next_graphics_limits;
typedef struct ogpu_next_query_limits {
    float timestamp_period_ns;
    uint32_t timestamp_compute_graphics;
} ogpu_next_query_limits;
enum { OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR = 1 };
enum {
    OGPU_NEXT_POINTS = 0, OGPU_NEXT_LINES = 1, OGPU_NEXT_LINE_STRIP = 2,
    OGPU_NEXT_TRIANGLES = 3, OGPU_NEXT_TRIANGLE_STRIP = 4
};
enum { OGPU_NEXT_CULL_NONE = 0, OGPU_NEXT_CULL_FRONT = 1, OGPU_NEXT_CULL_BACK = 2, OGPU_NEXT_CULL_BOTH = 3 };
enum { OGPU_NEXT_FRONT_CCW = 0, OGPU_NEXT_FRONT_CW = 1 };
enum {
    OGPU_NEXT_BLEND_ZERO = 0, OGPU_NEXT_BLEND_ONE = 1,
    OGPU_NEXT_BLEND_SRC_COLOR = 2, OGPU_NEXT_BLEND_ONE_MINUS_SRC_COLOR = 3,
    OGPU_NEXT_BLEND_DST_COLOR = 4, OGPU_NEXT_BLEND_ONE_MINUS_DST_COLOR = 5,
    OGPU_NEXT_BLEND_SRC_ALPHA = 6, OGPU_NEXT_BLEND_ONE_MINUS_SRC_ALPHA = 7,
    OGPU_NEXT_BLEND_DST_ALPHA = 8, OGPU_NEXT_BLEND_ONE_MINUS_DST_ALPHA = 9,
    OGPU_NEXT_BLEND_CONSTANT_COLOR = 10, OGPU_NEXT_BLEND_ONE_MINUS_CONSTANT_COLOR = 11,
    OGPU_NEXT_BLEND_CONSTANT_ALPHA = 12, OGPU_NEXT_BLEND_ONE_MINUS_CONSTANT_ALPHA = 13,
    OGPU_NEXT_BLEND_SRC_ALPHA_SATURATE = 14
};
enum { OGPU_NEXT_BLEND_ADD = 0, OGPU_NEXT_BLEND_SUBTRACT = 1, OGPU_NEXT_BLEND_REVERSE_SUBTRACT = 2, OGPU_NEXT_BLEND_MIN = 3, OGPU_NEXT_BLEND_MAX = 4 };
typedef struct ogpu_next_color_state {
    ogpu_next_format format;
    uint32_t write_mask, blend;
    uint32_t src_color, dst_color, color_op, src_alpha, dst_alpha, alpha_op;
} ogpu_next_color_state;
typedef struct ogpu_next_graphics_state {
    ogpu_next_record header;
    uint32_t topology, cull, front_face, samples, color_count;
    const ogpu_next_color_state *colors;
    ogpu_next_format depth_format;
    uint32_t depth_test, depth_write, depth_compare;
    float blend_constants[4];
} ogpu_next_graphics_state;
/* Graphics preparation: RASTER must be enabled. kind=GRAPHICS, exactly two
 * shaders ordered VERTEX then FRAGMENT, static_state=GRAPHICS_STATE,
 * dynamic_state=DYNAMIC_VIEWPORT_SCISSOR. requirements local_size/shared_memory=0.
 * Both shaders provide identical full argument interfaces; root visibility is
 * VERTEX, FRAGMENT or both. Independent roots use distinct offsets. No reflected
 * resource list, automatic per-stage remapping or hidden upload/PSO compilation.
 * Format count may be zero or up to max_colors; format=0 means no depth, otherwise
 * D16/D32. samples=1; sample bitmasks describe hardware, not enabled profile breadth.
 * Color mask bits are R=1,G=2,B=4,A=8. Booleans 0/1, compare uses COMPARE_*.
 * All color entries need identical blend/write state (independentBlend not enabled).
 * Fixed profile: fill, depth clip on, no depth bias/bounds/stencil/primitive restart,
 * no sample shading, logic op or fixed-function vertex bindings. Vertex pulling
 * remains available. Points require shader PointSize=1; vertex/fragment storage
 * writes need future feature enabling. Missing profiles are UNSUPPORTED, not emulated.
 */
enum { OGPU_NEXT_LOAD = 0, OGPU_NEXT_CLEAR = 1, OGPU_NEXT_DONT_CARE = 2 };
enum { OGPU_NEXT_STORE = 0, OGPU_NEXT_DISCARD = 1 };
typedef union ogpu_next_clear_value {
    float f32[4]; uint32_t u32[4]; int32_t i32[4];
    struct { float depth; uint32_t stencil; } depth_stencil;
} ogpu_next_clear_value;
typedef struct ogpu_next_attachment {
    ogpu_next_view *view, *resolve_view;
    ogpu_next_image_state state, resolve_state;
    uint32_t load_op, store_op, resolve_mode;
    ogpu_next_clear_value clear;
} ogpu_next_attachment;
typedef struct ogpu_next_render_desc {
    ogpu_next_record header;
    int32_t x, y;
    uint32_t width, height, layers, view_mask, samples, flags, color_count;
    const ogpu_next_attachment *colors;
    const ogpu_next_attachment *depth, *stencil;
    ogpu_next_host_span scratch;
} ogpu_next_render_desc;
typedef struct ogpu_next_viewport_state {
    ogpu_next_record header;
    float x, y, width, height, min_depth, max_depth;
    int32_t scissor_x, scissor_y;
    uint32_t scissor_width, scissor_height;
} ogpu_next_viewport_state;
typedef struct ogpu_next_draw_desc {
    uint32_t count, instances, first, first_instance;
    int32_t vertex_offset;
} ogpu_next_draw_desc;
/* Render begin does not need an executable and permits clear-only or attachmentless
 * scopes. Area is explicit and nonempty; layers>0; view_mask=flags=0, samples=1,
 * stencil=NULL, resolve_view=NULL, resolve_state=resolve_mode=0. Views are 2D/array,
 * one mip, with sufficient extent/layers and attachment usage. Null color/depth
 * views (state=UNDEFINED) are unused; null color slots require matching undefined
 * output format at draw, which the current executable profile does not yet expose.
 * Color states GENERAL/COLOR_ATTACHMENT; depth GENERAL/DEPTH_STENCIL_ATTACHMENT/
 * DEPTH_STENCIL_READ. Read-only depth forbids CLEAR and depth writes. Clear union
 * member matches the format; depth clear is finite [0,1]. No inferred transition,
 * content-preservation tracking, initialization or implicit viewport/scissor.
 * Scratch is sized per color count, aligned, disjoint and borrowed only during
 * begin. Attachments are borrowed through every recorded future/pending use.
 * All local attachment validation precedes the native begin call.
 *
 * Each draw TRUSTS matching pipeline/attachment formats, counts, samples and depth
 * write permission; shader output types and accesses must match. No per-draw
 * attachment scan/resource registry. Begin/end nesting, missing pipeline/viewport
 * and invalid command scope poison the list. Transfers, dispatch and generic
 * barriers are outside rendering only in this profile; local dependencies follow.
 * Graphics/compute bind points are independent. Viewport/scissor and byte state
 * persist across compatible binds/scopes, reset to undefined by command begin.
 * Dynamic viewport allows negative height and reversed depth endpoints within
 * [0,1]. set_graphics_state currently accepts VIEWPORT_STATE only. Draw requires
 * vertex_offset=0 and passes all count/first/instance values directly to native.
 */
ogpu_next_status ogpu_next_render_scratch_requirements(uint32_t color_count, ogpu_next_host_requirements *);
void ogpu_next_render_begin(ogpu_next_encoder *, const ogpu_next_render_desc *);
void ogpu_next_render_end(ogpu_next_encoder *);
void ogpu_next_set_graphics_state(ogpu_next_encoder *, const ogpu_next_record *);
void ogpu_next_draw(ogpu_next_encoder *, const ogpu_next_draw_desc *);
enum { OGPU_NEXT_INDEX_U16 = 16, OGPU_NEXT_INDEX_U32 = 32 };
typedef struct ogpu_next_indirect {
    ogpu_next_span arguments;
    uint32_t stride, maximum_count;
    ogpu_next_span count; /* Null memory with offset=size=0 means fixed maximum_count. */
} ogpu_next_indirect;
/* GPU wire records, NOT ogpu_next_draw_desc (indexed field order differs). */
typedef struct ogpu_next_draw_arguments {
    uint32_t count, instances, first, first_instance;
} ogpu_next_draw_arguments;
typedef struct ogpu_next_draw_indexed_arguments {
    uint32_t count, instances, first;
    int32_t vertex_offset;
    uint32_t first_instance;
} ogpu_next_draw_indexed_arguments;
/* Index binding borrows INDEX memory, aligned to its element width; persists
 * across render scopes/binds, undefined after command begin. Direct indexed draw
 * validates first+count against the bound span and preserves signed vertex_offset.
 * Indirect arguments/count borrow INDIRECT memory, addresses aligned to 4 bytes.
 * Stride is zero for at most one draw, or a multiple of 4 >= wire record size.
 * arguments covers (maximum_count-1)*stride + record size when count>0; no scan,
 * repack or CPU readback. maximum_count <= graphics_limits.max_indirect_count.
 * GPU count reads one uint32_t; draws min(count,maximum_count). GPU count itself
 * must also be <= max_indirect_count. GPU first_instance must be zero in this
 * profile (drawIndirectFirstInstance not enabled). Other GPU draw fields/index
 * bounds and resulting shader accesses are TRUSTED valid at every execution.
 * Index/argument/count memory requires caller synchronization and recorded/pending
 * lifetime. No resource retention, hidden allocation or implicit barrier.
 */
void ogpu_next_bind_indices(ogpu_next_encoder *, ogpu_next_span, uint32_t index_type);
void ogpu_next_draw_indexed(ogpu_next_encoder *, const ogpu_next_draw_desc *);
void ogpu_next_draw_indirect(ogpu_next_encoder *, const ogpu_next_indirect *);
void ogpu_next_draw_indexed_indirect(ogpu_next_encoder *, const ogpu_next_indirect *);
enum { OGPU_NEXT_QUERY_TIMESTAMP = 1, OGPU_NEXT_QUERY_OCCLUSION = 2 };
enum {
    OGPU_NEXT_QUERY_RESULT_64 = 1, OGPU_NEXT_QUERY_RESULT_AVAILABILITY = 2,
    OGPU_NEXT_QUERY_RESULT_WAIT = 4, OGPU_NEXT_QUERY_RESULT_PARTIAL = 8
};
typedef struct ogpu_next_query_pool_desc {
    ogpu_next_record header;
    uint32_t type, count;
    uint64_t statistics;
} ogpu_next_query_pool_desc;
/* Pools explicitly allocate native query storage, borrow their device and are
 * UNINITIALIZED until a recorded reset executes. count>0; statistics=0 in this
 * timestamp/occlusion profile. No implicit creation/reset/recycling, CPU readback,
 * retained list reference or wait-on-destroy. Caller owns reset/use/resolve order
 * and all recorded/pending lifetimes, including independent or simultaneous lists.
 * Replaying a reset/write list concurrently against the same slots is invalid;
 * independent pools/ranges remain usable independently. Host recording is local
 * to command arenas; there is no device-wide or query-pool recording mutex.
 *
 * Occlusion requires RASTER. One query may be active per list; begin/end must
 * match pool/index and render scope. An outside-scope query may enclose complete
 * scopes. An inside-scope query must end before render_end. No precise-count flag
 * yet: zero/nonzero visibility is meaningful, exact sample count is not promised.
 * Timestamp requires nonzero queue_info.timestamp_bits and one native stage bit:
 * ALL, COPY, COMPUTE, FRAGMENT, INDIRECT or COLOR in this profile; combined masks,
 * HOST and broad VERTEX/DEPTH groups are rejected. The requested stage can measure
 * a later native stage; it is not a whole-list completion or calibrated host clock.
 * Timestamp period is nanoseconds per tick. Mask to the queue's valid low bits and
 * account for wrap; compare only within a known compatible device/queue clock
 * domain. timestamp_compute_graphics is native timestampComputeAndGraphics, not
 * an extra clock-synchronization guarantee. No CPU/GPU clock calibration API yet.
 *
 * Reset/resolve are outside rendering on graphics/compute queues; first/count
 * names a nonempty in-bounds range with no active overlapping query. Resolve
 * writes COPY_DST memory: one unsigned 32-bit result, or 64-bit with RESULT_64;
 * optional availability follows at the same width (0 unavailable, nonzero ready).
 * Stride is bytes between records; when count>1, nonzero and aligned to result
 * width. Destination must fit every record and be width-aligned; overlapping
 * records are not repacked or given an OGPU write-order guarantee. WAIT is
 * an EXPLICIT GPU-side wait, never a host wait. Without WAIT, result data must not
 * be interpreted before available; PARTIAL permits interim occlusion values only.
 * Queried slots must have been reset and issued by prior execution; do not resolve
 * never-issued slots. Timestamp PARTIAL is invalid. Writes need a subsequent
 * COPY/WRITE dependency before host/shader use. Completion and cache invalidation
 * remain explicit; result availability does not prove unrelated work complete.
 */
ogpu_next_status ogpu_next_query_pool_create(ogpu_next_device *, const ogpu_next_query_pool_desc *, ogpu_next_query_pool **);
void ogpu_next_query_pool_destroy(ogpu_next_query_pool *);
void ogpu_next_queries_reset(ogpu_next_encoder *, ogpu_next_query_pool *, uint32_t first, uint32_t count);
void ogpu_next_query_begin(ogpu_next_encoder *, ogpu_next_query_pool *, uint32_t index);
void ogpu_next_query_end(ogpu_next_encoder *, ogpu_next_query_pool *, uint32_t index);
void ogpu_next_timestamp(ogpu_next_encoder *, ogpu_next_query_pool *, uint32_t index, ogpu_next_stages);
void ogpu_next_queries_resolve(ogpu_next_encoder *, ogpu_next_query_pool *, uint32_t first, uint32_t count, ogpu_next_span dst, uint64_t stride, uint32_t flags);
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
