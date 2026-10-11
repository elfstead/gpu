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
typedef struct ogpu_next_executable_cache ogpu_next_executable_cache;
typedef struct ogpu_next_query_pool ogpu_next_query_pool;
typedef struct ogpu_next_split ogpu_next_split;
typedef struct ogpu_next_image_barrier ogpu_next_image_barrier;
typedef int32_t ogpu_next_status;
enum {
    OGPU_NEXT_OK = 0, OGPU_NEXT_NOT_READY = 1, OGPU_NEXT_TIMEOUT = 2,
    OGPU_NEXT_COMPILE_REQUIRED = 3,
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
    OGPU_NEXT_QUERY_SUBGROUP_LIMITS = 11,
    OGPU_NEXT_QUERY_FORMATS = 12,
    OGPU_NEXT_QUERY_FLOAT_CONTROLS = 13,
    OGPU_NEXT_QUERY_MATRIX_TUPLES = 14,
    OGPU_NEXT_QUERY_INTEGER_DOT_INFO = 15,
    OGPU_NEXT_DEVICE_DESC = 100, OGPU_NEXT_MEMORY_DESC = 101,
    OGPU_NEXT_ARENA_DESC = 102, OGPU_NEXT_RECORDING_DESC = 103,
    OGPU_NEXT_SUBMIT_DESC = 104, OGPU_NEXT_DEPENDENCY = 105,
    OGPU_NEXT_IMAGE_DESC = 106, OGPU_NEXT_VIEW_DESC = 107,
    OGPU_NEXT_SAMPLER_DESC = 108, OGPU_NEXT_HEAP_BINDING = 109,
    OGPU_NEXT_EXECUTABLE_DESC = 110, OGPU_NEXT_ARGUMENT_INTERFACE = 111,
    OGPU_NEXT_SHADER_REQUIREMENTS = 112, OGPU_NEXT_SPECIALIZATION = 113,
    OGPU_NEXT_GRAPHICS_STATE = 114, OGPU_NEXT_RENDER_DESC = 115,
    OGPU_NEXT_VIEWPORT_STATE = 116, OGPU_NEXT_QUERY_POOL_DESC = 117,
    OGPU_NEXT_EXECUTABLE_CACHE_DESC = 118, OGPU_NEXT_VERTEX_INPUT = 119,
    OGPU_NEXT_SUBGROUP_STATE = 120, OGPU_NEXT_INHERITANCE = 121,
    OGPU_NEXT_RENDER_INHERITANCE = 122, OGPU_NEXT_IMAGE_PROFILE = 123,
    OGPU_NEXT_RASTER_OPTIONS = 124
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
/* QUERY_FORMATS lists every format known to this revision, including records with
 * zero capabilities. IDs/order are not array indices or a fixed-count promise.
 * Immutable physical capabilities, identical on adapter/device: not optional
 * feature enabling, throughput, supported combinations or allocation guarantees.
 * optimal/linear masks concern image tiling; linear image creation is not yet
 * exposed. buffer_features concern formatted buffers (including vertex fetch),
 * not arbitrary address-buffer arithmetic. Texel buffers remain a future profile.
 * Use image_limits for dimension/usage/flag tuples, then image_requirements for
 * concrete extents, placement and dedicated allocation constraints.
 */
#define OGPU_NEXT_FORMAT_SAMPLED UINT64_C(1)
#define OGPU_NEXT_FORMAT_STORAGE UINT64_C(2)
#define OGPU_NEXT_FORMAT_STORAGE_ATOMIC UINT64_C(4)
#define OGPU_NEXT_FORMAT_COLOR UINT64_C(8)
#define OGPU_NEXT_FORMAT_BLEND UINT64_C(16)
#define OGPU_NEXT_FORMAT_DEPTH_STENCIL UINT64_C(32)
#define OGPU_NEXT_FORMAT_BLIT_SRC UINT64_C(64)
#define OGPU_NEXT_FORMAT_BLIT_DST UINT64_C(128)
#define OGPU_NEXT_FORMAT_FILTER_LINEAR UINT64_C(256)
#define OGPU_NEXT_FORMAT_COPY_SRC UINT64_C(512)
#define OGPU_NEXT_FORMAT_COPY_DST UINT64_C(1024)
#define OGPU_NEXT_FORMAT_BUFFER_VERTEX UINT64_C(1)
#define OGPU_NEXT_FORMAT_BUFFER_UNIFORM_TEXEL UINT64_C(2)
#define OGPU_NEXT_FORMAT_BUFFER_STORAGE_TEXEL UINT64_C(4)
#define OGPU_NEXT_FORMAT_BUFFER_ATOMIC_TEXEL UINT64_C(8)
typedef struct ogpu_next_format_info {
    ogpu_next_format format;
    uint32_t aspects;
    uint64_t optimal_features, linear_features, buffer_features;
} ogpu_next_format_info;
typedef struct ogpu_next_image_profile {
    ogpu_next_record header;
    ogpu_next_format format;
    uint32_t dimension;
    uint64_t usage;
    uint32_t image_flags, reserved;
} ogpu_next_image_profile;
typedef struct ogpu_next_image_limits {
    uint32_t max_extent[3], max_mips, max_layers, sample_counts;
    uint64_t max_resource_size;
} ogpu_next_image_limits;
/* Optimal-image profile query: IMAGE_PROFILE, dimension/usage/flags use the image
 * description enums; reserved=0. Output written only on OK. No native resource,
 * memory allocation or OGPU heap allocation. Reports limits for this tuple under
 * the current implemented image profile (e.g. storage sample counts restricted to
 * one until multisampled-storage enabling exists). Unknown profiles fail; no
 * format substitution. Limits are not a promise that every maximal value can be
 * used simultaneously, or that memory of that size is currently available.
 */
ogpu_next_status ogpu_next_image_limits_query(const ogpu_next_device *, const ogpu_next_image_profile *, ogpu_next_image_limits *);
#define OGPU_NEXT_FEATURE_RASTER UINT64_C(1)
#define OGPU_NEXT_FEATURE_FLOAT16 UINT64_C(2)
#define OGPU_NEXT_FEATURE_UNIFIED_IMAGES UINT64_C(4)
#define OGPU_NEXT_FEATURE_SAMPLER_ANISOTROPY UINT64_C(8)
#define OGPU_NEXT_FEATURE_CACHE_CONTROL UINT64_C(16)
#define OGPU_NEXT_FEATURE_INT8 UINT64_C(32)
#define OGPU_NEXT_FEATURE_INT16 UINT64_C(64)
#define OGPU_NEXT_FEATURE_INT64 UINT64_C(128)
#define OGPU_NEXT_FEATURE_FLOAT64 UINT64_C(256)
#define OGPU_NEXT_FEATURE_STORAGE8 UINT64_C(512)
#define OGPU_NEXT_FEATURE_BUFFER_ATOMIC64 UINT64_C(1024)
#define OGPU_NEXT_FEATURE_SHARED_ATOMIC64 UINT64_C(2048)
#define OGPU_NEXT_FEATURE_SUBGROUP_SIZE_CONTROL UINT64_C(4096)
#define OGPU_NEXT_FEATURE_FULL_SUBGROUPS UINT64_C(8192)
#define OGPU_NEXT_FEATURE_SUBGROUP_EXTENDED_TYPES UINT64_C(16384)
#define OGPU_NEXT_FEATURE_PIPELINE_STATISTICS UINT64_C(32768)
#define OGPU_NEXT_FEATURE_PRECISE_OCCLUSION UINT64_C(65536)
#define OGPU_NEXT_FEATURE_INHERITED_QUERIES UINT64_C(131072)
#define OGPU_NEXT_FEATURE_VERTEX_STORES_ATOMICS UINT64_C(262144)
#define OGPU_NEXT_FEATURE_FRAGMENT_STORES_ATOMICS UINT64_C(524288)
#define OGPU_NEXT_FEATURE_INDIRECT_FIRST_INSTANCE UINT64_C(1048576)
#define OGPU_NEXT_FEATURE_INDEPENDENT_BLEND UINT64_C(2097152)
#define OGPU_NEXT_FEATURE_DEPTH_CLAMP UINT64_C(4194304)
#define OGPU_NEXT_FEATURE_DEPTH_BIAS_CLAMP UINT64_C(8388608)
#define OGPU_NEXT_FEATURE_NON_SOLID_FILL UINT64_C(16777216)
#define OGPU_NEXT_FEATURE_WIDE_LINES UINT64_C(33554432)
#define OGPU_NEXT_FEATURE_DEPTH_BOUNDS UINT64_C(67108864)
#define OGPU_NEXT_FEATURE_SAMPLE_SHADING UINT64_C(134217728)
#define OGPU_NEXT_FEATURE_ALPHA_TO_ONE UINT64_C(268435456)
#define OGPU_NEXT_FEATURE_UNUSED_ATTACHMENTS UINT64_C(536870912)
#define OGPU_NEXT_FEATURE_LOGIC_OP UINT64_C(1073741824)
#define OGPU_NEXT_FEATURE_DUAL_SOURCE_BLEND UINT64_C(2147483648)
#define OGPU_NEXT_FEATURE_SHADER_OUTPUT_LAYER UINT64_C(4294967296)
#define OGPU_NEXT_FEATURE_BUFFER_FLOAT32_ATOMICS UINT64_C(8589934592)
#define OGPU_NEXT_FEATURE_BUFFER_FLOAT32_ATOMIC_ADD UINT64_C(17179869184)
#define OGPU_NEXT_FEATURE_BUFFER_FLOAT32_ATOMIC_MIN_MAX UINT64_C(34359738368)
#define OGPU_NEXT_FEATURE_SHARED_FLOAT32_ATOMICS UINT64_C(68719476736)
#define OGPU_NEXT_FEATURE_SHARED_FLOAT32_ATOMIC_ADD UINT64_C(137438953472)
#define OGPU_NEXT_FEATURE_SHARED_FLOAT32_ATOMIC_MIN_MAX UINT64_C(274877906944)
#define OGPU_NEXT_FEATURE_BUFFER_FLOAT64_ATOMICS UINT64_C(549755813888)
#define OGPU_NEXT_FEATURE_BUFFER_FLOAT64_ATOMIC_ADD UINT64_C(1099511627776)
#define OGPU_NEXT_FEATURE_BUFFER_FLOAT64_ATOMIC_MIN_MAX UINT64_C(2199023255552)
#define OGPU_NEXT_FEATURE_SHARED_FLOAT64_ATOMICS UINT64_C(4398046511104)
#define OGPU_NEXT_FEATURE_SHARED_FLOAT64_ATOMIC_ADD UINT64_C(8796093022208)
#define OGPU_NEXT_FEATURE_SHARED_FLOAT64_ATOMIC_MIN_MAX UINT64_C(17592186044416)
#define OGPU_NEXT_FEATURE_COOPERATIVE_MATRIX UINT64_C(35184372088832)
#define OGPU_NEXT_FEATURE_VULKAN_MEMORY_MODEL UINT64_C(70368744177664)
#define OGPU_NEXT_FEATURE_VULKAN_MEMORY_MODEL_DEVICE_SCOPE UINT64_C(140737488355328)
#define OGPU_NEXT_FEATURE_INTEGER_DOT UINT64_C(281474976710656)
/* VULKAN_MEMORY_MODEL enables SPIR-V VulkanMemoryModel independently of matrix/scalar
 * features. DEVICE_SCOPE must be requested separately and requires the base bit
 * (otherwise device creation returns INVALID). Enabling the base without DEVICE_SCOPE
 * forbids Device memory scope in ALL shaders on that device, including GLSL450
 * artifacts. Availability/visibility chains are not enabled by either bit.
 * Memory ordering remains explicit in shader code;
 * no shader translation or inserted host/GPU barriers. */
/* FLOAT32_ATOMICS means floating-point atomic load/store/exchange; ADD and MIN_MAX
 * are separate, independently enabled operations, not implied by that bit. BUFFER
 * covers storage/address-backed buffer memory; SHARED covers workgroup memory.
 * Shader requirements declare every used operation/domain. Graphics storage-write
 * enabling remains separate. No CAS-loop/shader rewrite, deterministic reduction
 * order, cross-workgroup barrier or throughput guarantee is provided. NaN, signed
 * zero, denormal and rounding behavior follow the native artifact contract; the
 * ordinary floating-point-controls query does not establish atomic arithmetic
 * policy. FLOAT64 operation/domain bits have the same independence; FLOAT64
 * arithmetic/type enabling must also be requested by artifacts using double.
 * Image, sparse-image and 16-bit floating atomic profiles remain work.
 */
/* SHADER_OUTPUT_LAYER enables vertex-stage Layer output (Slang
 * SV_RenderTargetArrayIndex) into ordinary layered render scopes. It neither
 * enables multiview nor implicitly adds geometry/tessellation/mesh stages.
 * Layer indices are relative to the attachment view's base layer. The caller
 * guarantees valid indices below render.layers and consistent values across each
 * primitive. Shader requirements declare this feature. No per-layer draw loop,
 * view duplication, CPU readback of indices or hidden attachment transitions.
 */
typedef struct ogpu_next_feature_info {
    uint64_t available, enabled, max_timeline_difference;
    uint32_t baseline_supported, device_scope;
} ogpu_next_feature_info;
enum {
    OGPU_NEXT_FLOAT_PRESERVE_SIGNED_ZERO_INF_NAN = 1,
    OGPU_NEXT_FLOAT_PRESERVE_DENORMS = 2, OGPU_NEXT_FLOAT_FLUSH_DENORMS = 4,
    OGPU_NEXT_FLOAT_ROUND_NEAREST_EVEN = 8, OGPU_NEXT_FLOAT_ROUND_TOWARD_ZERO = 16
};
enum {
    OGPU_NEXT_FLOAT_INDEPENDENT_32 = 0, OGPU_NEXT_FLOAT_INDEPENDENT_ALL = 1,
    OGPU_NEXT_FLOAT_INDEPENDENT_NONE = 2
};
typedef struct ogpu_next_float_controls {
    uint32_t binary16, binary32, binary64;
    uint32_t denorm_independence, rounding_independence, reserved;
} ogpu_next_float_controls;
/* QUERY_FLOAT_CONTROLS is one immutable physical-capability record, identical on
 * adapter/device and independent of optional type enabling. Per-width masks report
 * supported artifact execution modes, NOT active/default modes or fast throughput.
 * Both denorm choices or rounding choices may be supported; that does not permit
 * selecting conflicting modes for one width. INDEPENDENT_32 permits 32-bit policy
 * to differ while 16/64 must agree; ALL permits per-width policies; NONE requires
 * one policy across widths. Denorm and rounding independence are separate.
 * An artifact/compiler chooses legal modes subject to these constraints and declares
 * arithmetic features separately. OGPU does not change modes, rewrite SPIR-V, widen
 * arithmetic, impose fast-math, or emulate unsupported behavior. Missing execution
 * modes do not acquire preservation guarantees from this query. Instruction-specific
 * native precision/exception rules still apply; this is not a blanket IEEE-754,
 * correctly-rounded-transcendental, NaN-payload or cross-device reproducibility promise.
 * reserved=0. No artifact mode-reflection/validation record or command-time floating
 * policy setter is implemented by this query. Later float-controls-2 modes remain
 * separate work. On baseline-ineligible adapters, zero masks make no mode promise.
 */
/* Feature records describe native enabling, not availability of future OGPU
 * draw/dispatch commands. Adapter queries: enabled=0, device_scope=0. Device
 * queries: only explicitly requested optional bits enabled, device_scope=1.
 * Queue flags and memory properties describe physical facilities, not enabled
 * sparse/protected functionality. Domain/type IDs belong to this adapter only.
 * FLOAT16/INT8/INT16/INT64/FLOAT64 enable the corresponding shader arithmetic
 * types. STORAGE8 independently enables 8-bit storage-buffer access (including
 * physical-address storage); 16-bit storage-buffer access is baseline. Arithmetic
 * enabling does not imply uniform-buffer/push-byte narrow storage, atomics, matrix
 * instructions, subgroup operations, fast native throughput or particular floating
 * point rounding/denormal modes. The compiler describes actual artifact requirements;
 * no widening, precision conversion or software fallback is performed by OGPU.
 * BUFFER_ATOMIC64 and SHARED_ATOMIC64 independently enable 64-bit integer atomics
 * on buffers/physical-address storage and workgroup memory, respectively. Their
 * shaders also require INT64; enabling an atomic domain does not enable another.
 * Atomicity does not imply ordering of unrelated accesses or a global execution
 * barrier. Shader memory scopes/semantics, convergence and inter-dispatch hazards
 * are caller/compiler contracts. Floating-point atomics are a separate profile.
 * VERTEX_STORES_ATOMICS and FRAGMENT_STORES_ATOMICS independently enable storage
 * writes/atomics in their shader stages; they do not enable RASTER, additional
 * stages, 64-bit/floating atomics or a storage-image format. Artifacts declare the
 * required bits; OGPU does not scan/rewrite shaders. Caller owns address validity,
 * synchronization and native invocation semantics (including duplicated vertex
 * work, fragment helper invocations and early/late tests). A draw is not an
 * exactly-once general compute dispatch. Attachmentless rasterization may use
 * these operations without allocating dummy color/depth targets.
 * INDIRECT_FIRST_INSTANCE permits nonzero GPU-resident indirect first_instance;
 * it does not add a CPU scan, remap or patch of argument records. INDEPENDENT_BLEND
 * permits distinct per-color-target blend/write state during preparation. Both
 * features are independent of RASTER enabling and preserve the existing commands.
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
    OGPU_NEXT_D24_UNORM_S8_UINT = 15, OGPU_NEXT_D32_FLOAT_S8_UINT = 16,
    OGPU_NEXT_RG32_FLOAT = 17, OGPU_NEXT_RGB32_FLOAT = 18
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
    uint32_t level, reserved;
} ogpu_next_arena_desc;
enum { OGPU_NEXT_ONE_SHOT = 1, OGPU_NEXT_SERIAL_REPLAY = 2, OGPU_NEXT_SIMULTANEOUS_REPLAY = 3 };
enum { OGPU_NEXT_PRIMARY = 0, OGPU_NEXT_SECONDARY = 1 };
typedef struct ogpu_next_recording_desc {
    ogpu_next_record header;
    uint32_t replay_mode, level;
    const ogpu_next_record *inheritance;
} ogpu_next_recording_desc;
typedef struct ogpu_next_inheritance {
    ogpu_next_record header;
    uint32_t occlusion, query_flags;
    uint64_t statistics;
    const ogpu_next_record *rendering;
    const ogpu_next_record *resource_heap, *sampler_heap;
} ogpu_next_inheritance;
/* Arena level is fixed at creation/reserve; recording level must match. reserved=0.
 * Primary inheritance=NULL. Secondary inheritance=NULL means standalone, no query
 * inheritance; otherwise INHERITANCE optionally names RENDER_INHERITANCE below.
 * One-shot accepts one successful execution (direct or through a primary).
 * Serial replay requires prior executions complete before resubmission;
 * simultaneous replay permits pending executions but does NOT resolve hazards.
 * The caller tracks pending use and keeps referenced resources alive, including
 * between replays. No resource lists, pending scans or implicit timeline values.
 * Secondary one-shot consumption is caller-tracked: submitting a primary never
 * walks its children. Non-simultaneous secondary lists may be referenced only once
 * by one live primary recording; recording into another invalidates the prior
 * primary under native rules. A simultaneous primary needs simultaneous children.
 * Reset/trim/destruction of a secondary arena invalidates ALL referring primary
 * recordings, even when idle. Discard/reset those primaries before future use.
 * No reset, re-recording or destruction while any direct/indirect use is pending.
 * Query inheritance occlusion=0/1; nonzero occlusion/query_flags requires enabled
 * INHERITED_QUERIES; query_flags=0 or QUERY_PRECISE. Nonzero statistics needs
 * PIPELINE_STATISTICS and a supported mask. Executing under any active query needs
 * INHERITED_QUERIES; the declared masks must cover that query. Secondary commands
 * must not begin query types active in the parent (caller proves this).
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
/* TRANSFER covers all native transfer operations. COPY excludes fill/clear,
 * blit and resolve; CLEAR includes buffer fill and standalone image clears.
 * Attachment clears/resolves instead use COLOR or DEPTH as appropriate.
 * VERTEX means vertex shader only, not fixed-function input. DEPTH combines
 * EARLY_DEPTH/LATE_DEPTH; VERTEX_INPUT combines INDEX_INPUT/VERTEX_FETCH.
 * Aggregate scopes are choices, never automatically added to narrower scopes.
 * Except COMPUTE/VERTEX/FRAGMENT, these are synchronization-only stage bits. */
#define OGPU_NEXT_STAGE_TRANSFER UINT64_C(2)
#define OGPU_NEXT_STAGE_COMPUTE UINT64_C(4)
#define OGPU_NEXT_STAGE_VERTEX UINT64_C(8)
#define OGPU_NEXT_STAGE_FRAGMENT UINT64_C(16)
#define OGPU_NEXT_STAGE_INDIRECT UINT64_C(32)
#define OGPU_NEXT_STAGE_COLOR UINT64_C(64)
#define OGPU_NEXT_STAGE_DEPTH UINT64_C(128)
#define OGPU_NEXT_STAGE_HOST UINT64_C(256)
#define OGPU_NEXT_STAGE_COPY UINT64_C(512)
#define OGPU_NEXT_STAGE_INDEX_INPUT UINT64_C(1024)
#define OGPU_NEXT_STAGE_VERTEX_FETCH UINT64_C(2048)
#define OGPU_NEXT_STAGE_EARLY_DEPTH UINT64_C(4096)
#define OGPU_NEXT_STAGE_LATE_DEPTH UINT64_C(8192)
#define OGPU_NEXT_STAGE_CLEAR UINT64_C(16384)
#define OGPU_NEXT_STAGE_RESOLVE UINT64_C(32768)
#define OGPU_NEXT_STAGE_BLIT UINT64_C(65536)
#define OGPU_NEXT_STAGE_VERTEX_INPUT UINT64_C(131072)
#define OGPU_NEXT_STAGE_ALL_GRAPHICS UINT64_C(262144)
#define OGPU_NEXT_STAGE_PRE_RASTER UINT64_C(524288)
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
#define OGPU_NEXT_ACCESS_SAMPLED_READ UINT64_C(262144)
#define OGPU_NEXT_ACCESS_STORAGE_READ UINT64_C(524288)
#define OGPU_NEXT_ACCESS_STORAGE_WRITE UINT64_C(1048576)
/* COPY_READ/WRITE cover native transfer accesses across COPY/CLEAR/BLIT/RESOLVE.
 * SHADER_READ/WRITE are broad shader accesses; sampled/storage/uniform and heap
 * accesses can be selected independently. Unknown masks are not widened. */
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
typedef struct ogpu_next_render_inheritance {
    ogpu_next_record header;
    uint32_t color_count, samples, view_mask, reserved;
    ogpu_next_format depth_format, stencil_format;
    const ogpu_next_format *color_formats;
    ogpu_next_host_span scratch;
} ogpu_next_render_inheritance;
/* Rendering inheritance declares compatible attachment formats/sample count, not
 * attachment handles. Zero formats mean unused; view_mask=reserved=0 for now.
 * Both nonzero depth/stencil formats must match. Format translation uses caller
 * scratch, borrowed with all description data only during commands_begin. The
 * caller proves formats/count/samples match each eventual primary render scope.
 * This secondary starts/ends INSIDE that scope: do not call render_begin/end.
 * Standalone secondaries may record complete inline rendering scopes themselves.
 * Neither flavor inherits pipeline, arguments, vertex/index or viewport state:
 * initialize everything used. Nested-secondary execution is not enabled.
 * Optional resource_heap/sampler_heap inheritance pointers name HEAP_BINDING
 * records of the corresponding kind. At EVERY execute, caller proves the parent
 * has identical address/size/reservation bindings. No parent-state scan or implicit
 * bind is added. An inherited heap cannot be rebound in that child; the other heap
 * remains independently bindable. NULL means not inherited, not an inferred heap.
 * Descriptions are borrowed only during begin, but their backing/reserved ranges
 * remain live and reserved through all child and parent future/pending uses.
 */
ogpu_next_status ogpu_next_inheritance_scratch_requirements(uint32_t color_count, ogpu_next_host_requirements *);
ogpu_next_status ogpu_next_execute_scratch_requirements(uint32_t count, ogpu_next_host_requirements *);
/* Execute from a primary only. Inputs and aligned scratch are borrowed only for
 * this call; the lists, arenas and their resources survive all future/pending use.
 * Entire array is validated before native execution. count=0 is a no-op.
 * Nonempty execution invalidates the primary's pipeline, inline/root bytes,
 * viewport/scissor and vertex/index bindings: re-establish before using.
 * A heap binding survives only when EVERY executed child inherits that heap;
 * otherwise it becomes undefined. No implicit binding restoration.
 * Render/query scopes survive. No implicit restoration or retained child graph.
 * Inside rendering requires RENDER_SECONDARY; outside requires standalone children.
 */
void ogpu_next_execute_lists(ogpu_next_encoder *, uint32_t count, ogpu_next_list *const *, ogpu_next_host_span scratch);
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

/* Prepared executable state. This implementation supports compute or vertex with
 * optional fragment SPIR-V with native heap/pointer access. Native artifacts, descriptor
 * set-to-heap mappings and launch extensions follow; these
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
    const ogpu_next_record *subgroup;
} ogpu_next_shader;
typedef struct ogpu_next_subgroup_limits {
    uint32_t default_size, min_size, max_size, max_compute_workgroup_subgroups;
    ogpu_next_stages stages, required_size_stages;
    uint32_t operations, quad_all_stages;
} ogpu_next_subgroup_limits;
enum {
    OGPU_NEXT_COMPONENT_FLOAT16 = 0, OGPU_NEXT_COMPONENT_FLOAT32 = 1,
    OGPU_NEXT_COMPONENT_FLOAT64 = 2, OGPU_NEXT_COMPONENT_SINT8 = 3,
    OGPU_NEXT_COMPONENT_SINT16 = 4, OGPU_NEXT_COMPONENT_SINT32 = 5,
    OGPU_NEXT_COMPONENT_SINT64 = 6, OGPU_NEXT_COMPONENT_UINT8 = 7,
    OGPU_NEXT_COMPONENT_UINT16 = 8, OGPU_NEXT_COMPONENT_UINT32 = 9,
    OGPU_NEXT_COMPONENT_UINT64 = 10
};
enum {
    OGPU_NEXT_DOT_8_UNSIGNED = 1,
    OGPU_NEXT_DOT_8_SIGNED = 2,
    OGPU_NEXT_DOT_8_MIXED = 4,
    OGPU_NEXT_DOT_PACKED_4X8_UNSIGNED = 8,
    OGPU_NEXT_DOT_PACKED_4X8_SIGNED = 16,
    OGPU_NEXT_DOT_PACKED_4X8_MIXED = 32,
    OGPU_NEXT_DOT_16_UNSIGNED = 64,
    OGPU_NEXT_DOT_16_SIGNED = 128,
    OGPU_NEXT_DOT_16_MIXED = 256,
    OGPU_NEXT_DOT_32_UNSIGNED = 512,
    OGPU_NEXT_DOT_32_SIGNED = 1024,
    OGPU_NEXT_DOT_32_MIXED = 2048,
    OGPU_NEXT_DOT_64_UNSIGNED = 4096,
    OGPU_NEXT_DOT_64_SIGNED = 8192,
    OGPU_NEXT_DOT_64_MIXED = 16384
};
typedef struct ogpu_next_integer_dot_info {
    uint32_t accelerated, accumulating_saturating_accelerated;
} ogpu_next_integer_dot_info;
/* QUERY_INTEGER_DOT_INFO returns physical acceleration hints, independent of
 * feature enabling. Both fields are masks of DOT_* operand profiles. Acceleration
 * follows the native Vulkan definition, not a measured throughput guarantee.
 * Zero means no advertised acceleration, not lack of instruction support.
 * INTEGER_DOT enables native signed/unsigned/mixed dot and accumulating-saturating
 * instructions; INT8/16/64 arithmetic and narrow storage remain separate features.
 * Packed 4x8 operates on 32-bit values and needs no INT8/STORAGE8 enabling.
 * Caller/compiler chooses packing, signedness, accumulation and overflow policy;
 * OGPU neither rewrites kernels nor allocates quantization metadata/workspace. */
enum { OGPU_NEXT_MATRIX_SCOPE_SUBGROUP = 3 };
typedef struct ogpu_next_matrix_tuple {
    uint32_t m, n, k, a, b, c, result, saturating, scope, reserved;
    ogpu_next_stages stages;
} ogpu_next_matrix_tuple;
/* QUERY_MATRIX_TUPLES reports immutable physical multiply-add tuples in native
 * enumeration order, independent of device enabling. Each tuple describes
 * A[M,K] * B[K,N] + C[M,N] -> result[M,N]; component enums are not image formats.
 * saturating=0/1 is the exact required integer saturation operand, not permission
 * to freely choose either. All shape/type/scope fields match one tuple together;
 * maxima from different tuples cannot be combined. This implemented profile is
 * KHR subgroup-scope, COMPUTE-stage, core component types only. Later workgroup,
 * flexible-size, BF16/FP8 and tensor-resource profiles are not inferred from it.
 * No supported tuples returns count=0, not a synthetic scalar fallback.
 * COOPERATIVE_MATRIX must be explicitly enabled and declared in shader requirements.
 * Scalar features needed by the artifact (e.g. FLOAT16/INT8) remain independent.
 * The caller/compiler verifies exact native matrix types/operands and tuple support,
 * converged participation, memory alignment/layout/stride/bounds and synchronization.
 * OGPU does not reflect/rewrite code, choose subgroup size, pad tensors, allocate
 * workspace, or promise tensor-core throughput. Robust matrix access is not enabled.
 */
enum {
    OGPU_NEXT_SUBGROUP_BASIC = 1, OGPU_NEXT_SUBGROUP_VOTE = 2,
    OGPU_NEXT_SUBGROUP_ARITHMETIC = 4, OGPU_NEXT_SUBGROUP_BALLOT = 8,
    OGPU_NEXT_SUBGROUP_SHUFFLE = 16, OGPU_NEXT_SUBGROUP_SHUFFLE_RELATIVE = 32,
    OGPU_NEXT_SUBGROUP_CLUSTERED = 64, OGPU_NEXT_SUBGROUP_QUAD = 128
};
enum { OGPU_NEXT_SUBGROUP_ALLOW_VARYING = 1, OGPU_NEXT_SUBGROUP_REQUIRE_FULL = 2 };
typedef struct ogpu_next_subgroup_state {
    ogpu_next_record header;
    uint32_t operations, required_size, flags, reserved;
} ogpu_next_subgroup_state;
/* Optional per-shader SUBGROUP_STATE; NULL leaves native defaults. operations is
 * the artifact's required operation-group mask, checked against subgroup_limits
 * and stage support. Query stage masks currently represent COMPUTE/VERTEX/FRAGMENT
 * only. QUAD in vertex requires quad_all_stages. No lane-to-invocation mapping is
 * promised. Lane participation/convergence and actual opcode/type requirements
 * remain compiler/caller contracts, not inferred by runtime SPIR-V reflection.
 * required_size=0 preserves native default; otherwise an exact supported power of
 * two, requiring enabled SUBGROUP_SIZE_CONTROL and a required_size_stages match.
 * ALLOW_VARYING also requires SUBGROUP_SIZE_CONTROL and forbids required_size!=0.
 * REQUIRE_FULL requires enabled FULL_SUBGROUPS and COMPUTE in this profile. The
 * declared post-specialization local X must be divisible by required_size, or by
 * max_size when varying, or default_size otherwise. A specified size also bounds
 * total local invocations by size*max_compute_workgroup_subgroups. reserved=0.
 * SUBGROUP_EXTENDED_TYPES enables narrow/wide subgroup operand types, not their
 * arithmetic/storage features; those must also be requested. All choices apply
 * at preparation, never by changing dispatch dimensions or compiling while recording.
 */
typedef struct ogpu_next_executable_desc {
    ogpu_next_record header;
    uint32_t kind, shader_count;
    const ogpu_next_shader *shaders;
    const ogpu_next_record *static_state;
    uint64_t dynamic_state;
    const ogpu_next_record *requirements;
    ogpu_next_executable_cache *cache;
    uint32_t compile_flags, reserved;
} ogpu_next_executable_desc;
enum { OGPU_NEXT_COMPILE_FAIL_IF_REQUIRED = 1 };
enum { OGPU_NEXT_CACHE_NATIVE_SYNCHRONIZATION = 0, OGPU_NEXT_CACHE_CALLER_SYNCHRONIZATION = 1 };
typedef struct ogpu_next_executable_cache_desc {
    ogpu_next_record header;
    uint32_t synchronization, reserved;
    ogpu_next_bytes initial;
} ogpu_next_executable_cache_desc;
/* Optional preparation-only storage, shared across compute/graphics executables
 * on the same device. No default/global cache, disk I/O, hidden lookup or retention.
 * initial={NULL,0} creates an empty cache. Nonempty data MUST be unchanged native
 * bytes previously exported, with the returned size. Header size/version, vendor,
 * device and cache UUID are checked; identity mismatch is UNSUPPORTED, never silently
 * retried as an empty cache. The opaque payload remains trusted, not validated.
 * Bytes are borrowed during creation only. The cache borrows its device. reserved=0.
 * NATIVE_SYNCHRONIZATION permits concurrent preparations sharing this cache via
 * native synchronization. CALLER_SYNCHRONIZATION requires enabled CACHE_CONTROL
 * and exclusive access to this cache during preparation; native cache locking is
 * disabled. Independent caches may always be prepared concurrently. Export excludes
 * all mutation in either mode; destruction excludes every cache use. Prepared
 * executables do not retain the cache and may outlive it. No GPU wait is needed.
 * data=NULL queries maximum export size. Otherwise *size is input capacity and
 * output bytes written: CAPACITY may write a valid partial native cache, unlike
 * atomic array queries. Query size again to request a full export. All output bytes
 * and size storage are disjoint. Native caching is advisory, not a hit/latency promise.
 */
ogpu_next_status ogpu_next_executable_cache_create(ogpu_next_device *, const ogpu_next_executable_cache_desc *, ogpu_next_executable_cache **);
ogpu_next_status ogpu_next_executable_cache_data(ogpu_next_executable_cache *, size_t *size, void *data);
/* Merge preserves destination contents and borrows sources only during this call.
 * All caches belong to the same device; destination must not be a source. The
 * caller excludes all other destination access and source mutation/destruction.
 * count=0 is a no-op. Cold translation may allocate; no recording-path work.
 * On native failure no rollback of advisory cache contents is promised. */
ogpu_next_status ogpu_next_executable_cache_merge(ogpu_next_executable_cache *destination, uint32_t count, ogpu_next_executable_cache *const *sources);
void ogpu_next_executable_cache_destroy(ogpu_next_executable_cache *);
/* stage=COMPUTE; kind=COMPUTE; shader_count=1; static_state=NULL, dynamic_state=0,
 * cache=NULL or an explicit cache above. Explicit nonempty UTF-8 entry name (not forced to main).
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
 * is borrowed through destruction; no current context or implicit shared cache.
 * compile_flags=0 permits native compilation. FAIL_IF_REQUIRED requires enabled
 * CACHE_CONTROL and applies to compute and graphics, with or without a cache.
 * If native compilation is needed, returns COMPILE_REQUIRED and a NULL executable;
 * no automatic retry, background work, or deferred compilation is scheduled.
 * Caller may retry explicitly with flags=0. This is not an allocation-free or
 * bounded-latency guarantee, nor a cache-hit guarantee, even after an earlier
 * successful preparation or cache import. reserved=0; unknown flags are rejected.
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
    uint32_t integer_color_samples, stencil_samples;
    uint32_t max_vertex_bindings, max_vertex_attributes, max_vertex_attribute_offset, max_vertex_stride;
    uint64_t dynamic_states; /* supported bits; VIEWPORT_SCISSOR remains required in this profile */
    float line_width_range[2], line_width_granularity;
    uint32_t max_dual_source_attachments;
    uint32_t depth_resolve_modes, stencil_resolve_modes;
    uint32_t independent_resolve_none, independent_resolve;
} ogpu_next_graphics_limits;
typedef struct ogpu_next_query_limits {
    float timestamp_period_ns;
    uint32_t timestamp_compute_graphics;
    uint64_t statistics;
} ogpu_next_query_limits;
enum {
    OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR = 1, OGPU_NEXT_DYNAMIC_BLEND_CONSTANTS = 2,
    OGPU_NEXT_DYNAMIC_STENCIL_REFERENCE = 4, OGPU_NEXT_DYNAMIC_STENCIL_COMPARE_MASK = 8,
    OGPU_NEXT_DYNAMIC_STENCIL_WRITE_MASK = 16
};
enum { OGPU_NEXT_FACE_FRONT = 1, OGPU_NEXT_FACE_BACK = 2, OGPU_NEXT_FACE_BOTH = 3 };
/* Dynamic values map directly to native commands, inside or outside rendering,
 * on graphics domains with RASTER enabled. No pipeline need be bound yet.
 * Preparation selects each optional dynamic bit independently; unselected fields
 * stay static. Dynamic fields ignore their corresponding static-state values.
 * The caller initializes every state/face needed by the draw. Dynamic values
 * survive binds declaring that state dynamic, but a bind declaring it STATIC
 * invalidates its prior dynamic value. Reissue the setter before the next draw
 * using that state dynamically. Setting a field after binding a static pipeline
 * requires a subsequent compatible pipeline bind before drawing; no automatic
 * restore, cache, elision, pipeline variant/JIT, or per-draw state walk is added.
 * Secondary execution invalidates ordinary dynamic state; it is not restored.
 * faces=FRONT/BACK/BOTH, or 0 (native no-op); bits outside BOTH are INVALID.
 * Reference/masks are full u32 values, not normalized to a guessed stencil width.
 * Blend values are passed unchanged, without clamping; the four floats are
 * borrowed only during the call. Native numerical rules apply.
 */
void ogpu_next_set_blend_constants(ogpu_next_encoder *, const float values[4]);
void ogpu_next_set_stencil_reference(ogpu_next_encoder *, uint32_t faces, uint32_t reference);
void ogpu_next_set_stencil_compare_mask(ogpu_next_encoder *, uint32_t faces, uint32_t mask);
void ogpu_next_set_stencil_write_mask(ogpu_next_encoder *, uint32_t faces, uint32_t mask);
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
    OGPU_NEXT_BLEND_SRC_ALPHA_SATURATE = 14,
    OGPU_NEXT_BLEND_SRC1_COLOR = 15, OGPU_NEXT_BLEND_ONE_MINUS_SRC1_COLOR = 16,
    OGPU_NEXT_BLEND_SRC1_ALPHA = 17, OGPU_NEXT_BLEND_ONE_MINUS_SRC1_ALPHA = 18
};
/* SRC1 factors require DUAL_SOURCE_BLEND, including when stored in disabled blend
 * state. They use the fragment output's second source index at the SAME location,
 * not another color attachment. The artifact supplies source indices 0 and 1;
 * OGPU does not rewrite output locations, create another target or blend in a shader.
 * With enabled dual-source blending, all statically used fragment output locations
 * must be below max_dual_source_attachments. This limits shader output locations,
 * not the number of declared pipeline slots (unused slots need not be removed).
 * Shader requirements must declare DUAL_SOURCE_BLEND when its interface needs it.
 * Output type/components, secondary outputs and their limit are caller artifact
 * contracts; no shader reflection or draw-time output scan is performed.
 */
enum { OGPU_NEXT_BLEND_ADD = 0, OGPU_NEXT_BLEND_SUBTRACT = 1, OGPU_NEXT_BLEND_REVERSE_SUBTRACT = 2, OGPU_NEXT_BLEND_MIN = 3, OGPU_NEXT_BLEND_MAX = 4 };
typedef struct ogpu_next_color_state {
    ogpu_next_format format;
    uint32_t write_mask, blend;
    uint32_t src_color, dst_color, color_op, src_alpha, dst_alpha, alpha_op;
} ogpu_next_color_state;
enum {
    OGPU_NEXT_STENCIL_KEEP = 0, OGPU_NEXT_STENCIL_ZERO = 1, OGPU_NEXT_STENCIL_REPLACE = 2,
    OGPU_NEXT_STENCIL_INCREMENT_CLAMP = 3, OGPU_NEXT_STENCIL_DECREMENT_CLAMP = 4,
    OGPU_NEXT_STENCIL_INVERT = 5, OGPU_NEXT_STENCIL_INCREMENT_WRAP = 6, OGPU_NEXT_STENCIL_DECREMENT_WRAP = 7
};
typedef struct ogpu_next_stencil_state {
    uint32_t fail, pass, depth_fail, compare, compare_mask, write_mask, reference;
} ogpu_next_stencil_state;
typedef struct ogpu_next_graphics_state {
    ogpu_next_record header;
    uint32_t topology, cull, front_face, samples, color_count;
    const ogpu_next_color_state *colors;
    ogpu_next_format depth_format;
    uint32_t depth_test, depth_write, depth_compare;
    float blend_constants[4];
    uint32_t stencil_test;
    ogpu_next_stencil_state stencil_front, stencil_back;
    const ogpu_next_record *vertex_input;
    const ogpu_next_record *raster_options;
    uint32_t logic_op_enable, logic_op;
} ogpu_next_graphics_state;
enum {
    OGPU_NEXT_LOGIC_CLEAR = 0, OGPU_NEXT_LOGIC_AND = 1, OGPU_NEXT_LOGIC_AND_REVERSE = 2,
    OGPU_NEXT_LOGIC_COPY = 3, OGPU_NEXT_LOGIC_AND_INVERTED = 4, OGPU_NEXT_LOGIC_NO_OP = 5,
    OGPU_NEXT_LOGIC_XOR = 6, OGPU_NEXT_LOGIC_OR = 7, OGPU_NEXT_LOGIC_NOR = 8,
    OGPU_NEXT_LOGIC_EQUIVALENT = 9, OGPU_NEXT_LOGIC_INVERT = 10, OGPU_NEXT_LOGIC_OR_REVERSE = 11,
    OGPU_NEXT_LOGIC_COPY_INVERTED = 12, OGPU_NEXT_LOGIC_OR_INVERTED = 13,
    OGPU_NEXT_LOGIC_NAND = 14, OGPU_NEXT_LOGIC_SET = 15
};
/* logic_op_enable is 0/1; enabling requires LOGIC_OP. logic_op is one of LOGIC_*.
 * One static operation applies independently per stored component on all targets,
 * after source format conversion and before the per-target write mask. Enabling
 * logic operations disables blending natively; blend state is not rewritten.
 * Native formats that do not support logic operations pass through source values.
 * This is not arbitrary bitwise arithmetic on shader floating-point encodings.
 * No shader emulation, extra pass or implicit attachment read transition.
 */
enum { OGPU_NEXT_POLYGON_FILL = 0, OGPU_NEXT_POLYGON_LINE = 1, OGPU_NEXT_POLYGON_POINT = 2 };
typedef struct ogpu_next_raster_options {
    ogpu_next_record header;
    uint32_t primitive_restart, depth_clamp, discard, polygon_mode;
    uint32_t depth_bias;
    float depth_bias_constant, depth_bias_clamp, depth_bias_slope;
    float line_width;
    uint32_t depth_bounds;
    float min_depth_bounds, max_depth_bounds;
    uint64_t sample_mask;
    uint32_t sample_shading;
    float min_sample_shading;
    uint32_t alpha_to_coverage, alpha_to_one;
} ogpu_next_raster_options;
/* raster_options=NULL selects fill, width=1, all samples enabled, bounds=[0,1],
 * all enables/factors zero. Otherwise borrow RASTER_OPTIONS during preparation:
 * booleans are 0/1; floats finite, line_width>0, bounds ordered within [0,1],
 * min_sample_shading in [0,1]. Supply explicit defaults: a zeroed record is NOT
 * equivalent to NULL (zero width is invalid; zero sample mask suppresses coverage).
 * Restart is available for LINE_STRIP/TRIANGLE_STRIP, using the fixed all-ones
 * index value for the bound index type. List restart is a separate future profile.
 * DEPTH_CLAMP enables clamping and disables Z clipping; independent depth clipping
 * is not exposed. Nonzero bias clamp needs DEPTH_BIAS_CLAMP even with bias disabled.
 * Non-fill modes, non-unit width, bounds testing, sample shading and alpha-to-one
 * need NON_SOLID_FILL/WIDE_LINES/DEPTH_BOUNDS/SAMPLE_SHADING/ALPHA_TO_ONE respectively.
 * Non-unit width must fit the reported range; native granularity/rounding applies.
 * Bounds testing uses the existing stored depth, not incoming fragment depth;
 * with no depth attachment the native test passes. Discard suppresses rasterization,
 * not vertex work; discard pipelines supply only VERTEX (no FRAGMENT shader).
 * Static discard requires zero color/depth formats. It can execute in an attached
 * render scope when UNUSED_ATTACHMENTS is enabled; otherwise use attachmentless scopes.
 * Attached static discard currently supports single-sample scopes only; native
 * static discard omits multisample state. Multisample attached discard remains work.
 * The viewport/scissor dynamic bit remains required by this profile.
 * Sample bit n controls sample n; bits beyond samples are ignored, including the
 * high 32 bits for counts <=32. No custom sample locations or mask expansion.
 * Sample shading specifies a minimum fraction, not an exact invocation count.
 * Alpha coverage uses fragment output zero's alpha; conversion is native, not
 * portable bit-exact coverage. Alpha-to-one applies after that coverage operation.
 * All are static state: no draw-time tracking, shader patch or implicit resolve.
 */
enum { OGPU_NEXT_VERTEX_RATE_VERTEX = 0, OGPU_NEXT_VERTEX_RATE_INSTANCE = 1 };
typedef struct ogpu_next_vertex_binding { uint32_t binding, stride, rate; } ogpu_next_vertex_binding;
typedef struct ogpu_next_vertex_attribute { uint32_t location, binding; ogpu_next_format format; uint32_t offset; } ogpu_next_vertex_attribute;
typedef struct ogpu_next_vertex_input {
    ogpu_next_record header;
    uint32_t binding_count, attribute_count;
    const ogpu_next_vertex_binding *bindings;
    const ogpu_next_vertex_attribute *attributes;
} ogpu_next_vertex_input;
/* vertex_input=NULL means no fixed-function input (vertex pulling remains legal).
 * Otherwise VERTEX_INPUT describes explicit unique binding IDs and unique attribute
 * locations; each attribute references a listed binding. Sparse binding IDs are
 * allowed. Limits and native vertex-format support are checked during preparation.
 * Strides are static, including zero and overlapping records; attribute extent is
 * not artificially constrained to stride. Rate is per vertex or per instance with
 * divisor one. Shader input types/locations and native fetch alignment are caller
 * contracts. Metadata is borrowed only during preparation. Additional divisor and
 * dynamic input/stride profiles are not enabled; no repack or shader rewriting.
 */
/* Graphics preparation: RASTER must be enabled. kind=GRAPHICS, VERTEX followed
 * by optional FRAGMENT (FRAGMENT must be absent when discard is enabled), static_state=GRAPHICS_STATE,
 * dynamic_state includes DYNAMIC_VIEWPORT_SCISSOR and any selected supported
 * optional bits (graphics_limits.dynamic_states). requirements local_size/shared_memory=0.
 * Supplied shaders provide identical full argument interfaces; root visibility is
 * VERTEX, FRAGMENT or both. Independent roots use distinct offsets. No reflected
 * resource list, automatic per-stage remapping or hidden upload/PSO compilation.
 * Format count may be zero or up to max_colors; format=0 means no depth, otherwise
 * D16/D32 or combined D24S8/D32S8. Combined formats declare both depth and stencil
 * pipeline attachment formats. Stencil testing requires a combined format;
 * front/back operations, masks and reference are explicit static pipeline state.
 * samples is a single supported count bit (1/2/4/8/16/32/64), selected
 * from the intersection of the corresponding graphics sample masks. Integer
 * color has its own mask. Exact image tuples still use image_requirements.
 * Color mask bits are R=1,G=2,B=4,A=8. Booleans 0/1, compare uses COMPARE_*.
 * Color entries need identical blend/write state unless INDEPENDENT_BLEND is enabled.
 * Color format=0 declares an unused output slot (no format-support query).
 * Without UNUSED_ATTACHMENTS, pipeline/scope color counts must match, and null
 * color views require corresponding format=0; non-null views match exact formats.
 * With UNUSED_ATTACHMENTS, color counts may differ and a format=0/absent output
 * may coexist with a non-null view, or a defined output with a null view. For a
 * format=0 slot with a non-null view, the shader must not write it or its write_mask
 * must be zero. This is a caller precondition, not shader reflection or mask rewriting.
 * Null-view outputs are discarded; attachment clears,
 * stores and resolves still follow the explicit render scope. Defined non-null
 * pairs still require matching formats. Depth/stencil format=0 versus absent view
 * is similarly relaxed. No inferred transitions, attachment allocation or remap.
 * Additional static raster/sample controls use raster_options. Custom
 * sample locations and independent depth clipping remain unimplemented profiles.
 * Both vertex pulling and native vertex fetch are available. Points require shader
 * PointSize=1; vertex/fragment storage writes require their independent feature bits.
 * Missing profiles are UNSUPPORTED, not emulated.
 * Omitting FRAGMENT still rasterizes and runs fixed depth/stencil tests and writes.
 * It is useful for depth-only passes and occlusion, not equivalent to discard.
 * No dummy fragment shader is created. With no fragment shader, color outputs and
 * shader-derived alpha are undefined: do not consume them (disable color writes
 * and alpha-to-coverage). Attachment load/store operations remain explicit.
 */
enum { OGPU_NEXT_LOAD = 0, OGPU_NEXT_CLEAR = 1, OGPU_NEXT_DONT_CARE = 2 };
enum { OGPU_NEXT_STORE = 0, OGPU_NEXT_DISCARD = 1 };
enum {
    OGPU_NEXT_ATTACHMENT_RESOLVE_NONE = 0, OGPU_NEXT_ATTACHMENT_RESOLVE_SAMPLE_ZERO = 1,
    OGPU_NEXT_ATTACHMENT_RESOLVE_AVERAGE = 2, OGPU_NEXT_ATTACHMENT_RESOLVE_MIN = 4,
    OGPU_NEXT_ATTACHMENT_RESOLVE_MAX = 8
};
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
enum { OGPU_NEXT_RENDER_SECONDARY = 1 };
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
 * scopes. Area is explicit and nonempty; layers>0; view_mask=0.
 * flags=0 selects inline commands; RENDER_SECONDARY selects secondary execution
 * in a primary scope and disallows inline draws. Nested execution is unsupported.
 * samples is one supported count and must match all non-resolve attachments.
 * Views are 2D/array with identity component mapping,
 * one mip, with sufficient extent/layers and attachment usage. Null color/depth
 * views (state=UNDEFINED) are unused; executable format compatibility follows the
 * UNUSED_ATTACHMENTS contract above.
 * Color states GENERAL/COLOR_ATTACHMENT; depth GENERAL/DEPTH_STENCIL_ATTACHMENT/
 * DEPTH_STENCIL_READ. Stencil uses these same states. Read-only depth/stencil
 * forbids CLEAR and writes to the respective aspect. When both records exist,
 * they must name the same view and state; load/store/clear remain independent.
 * Depth/stencil attachment view aspect masks are ignored by the native API.
 * Separate depth/stencil layouts are not enabled. Clear union
 * member matches the format; depth clear is finite [0,1]. No inferred transition,
 * content-preservation tracking, initialization or implicit viewport/scissor.
 * Scratch is sized per color count, aligned, disjoint and borrowed only during
 * begin. Attachments are borrowed through every recorded future/pending use.
 * All local attachment validation precedes the native begin call.
 * Color resolve is explicit: NONE requires resolve_view=NULL and resolve_state=0;
 * AVERAGE for normalized/float formats, SAMPLE_ZERO for integer formats. These
 * attachment modes differ from standalone RESOLVE_NATIVE_COLOR. Source samples>1;
 * destination has one sample, identical view format, sufficient extent/layers,
 * COLOR_ATTACHMENT usage and GENERAL/COLOR_ATTACHMENT state. It resolves at scope
 * end, independently of source STORE/DISCARD; no temporary image or transition.
 * Depth/stencil resolves select one supported bit from graphics_limits'
 * depth_resolve_modes/stencil_resolve_modes, with identical source/target format,
 * DEPTH_STENCIL_ATTACHMENT usage and GENERAL/DEPTH_STENCIL_ATTACHMENT target state.
 * No feature bit is needed. With both depth and stencil views active, differing
 * modes require independent_resolve when neither is NONE, or independent_resolve_none
 * when one is NONE. When both resolve, they use the same target view/state.
 * Absent depth/stencil records do not impose paired-mode constraints. Source discard
 * does not discard the resolved target. Caller proves non-aliasing accesses; no
 * automatic shader resolve, temporary target, barrier or content tracking.
 * Native Vulkan attachment resolves (including depth/stencil) use COLOR stage
 * with COLOR_READ/COLOR_WRITE access, not standalone RESOLVE or DEPTH scopes.
 * Sample shading, masks and alpha coverage are explicit raster options; custom
 * sample positions remain a separate unimplemented profile.
 *
 * Each draw TRUSTS matching pipeline/attachment formats, counts, samples and
 * depth/stencil write permission; shader output types and accesses must match. No per-draw
 * attachment scan/resource registry. Begin/end nesting, missing pipeline/viewport
 * and invalid command scope poison the list. Transfers, dispatch and generic
 * barriers are outside rendering only in this profile; local dependencies follow.
 * Graphics/compute bind points are independent. Viewport/scissor and byte state
 * persist across compatible binds/scopes, reset to undefined by command begin.
 * Dynamic viewport allows negative height and reversed depth endpoints within
 * [0,1]. set_graphics_state accepts VIEWPORT_STATE and the declared dynamic
 * blend/stencil records. Draw requires
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
 * must also be <= max_indirect_count. GPU first_instance must be zero unless
 * INDIRECT_FIRST_INSTANCE is enabled. This is a caller execution-time obligation;
 * argument memory is never read or patched by OGPU. Other GPU draw fields/index
 * bounds and resulting shader accesses are TRUSTED valid at every execution.
 * Index/argument/count memory requires caller synchronization and recorded/pending
 * lifetime. No resource retention, hidden allocation or implicit barrier.
 */
void ogpu_next_bind_indices(ogpu_next_encoder *, ogpu_next_span, uint32_t index_type);
/* Bind consecutive native vertex slots from explicit VERTEX spans. No executable
 * or active render scope required. count=0 is a no-op; first+count <= max bindings.
 * Scratch is sized by count, disjoint/aligned, borrowed only during the call.
 * All local checks precede one native bind; no allocation, repack, barrier or
 * resource retention. Binding state persists across scopes/pipeline binds, not
 * across command begin/reset. Pipeline static strides are left unchanged.
 * Each draw TRUSTS that every fetched binding is set and that all vertex/instance
 * indices (including first/base offsets), attribute offsets, strides and formats
 * address aligned, initialized bytes within the supplied spans. No per-draw scan.
 * Spans must remain alive and synchronized through recorded future/pending uses.
 */
ogpu_next_status ogpu_next_vertex_scratch_requirements(uint32_t count, ogpu_next_host_requirements *);
void ogpu_next_bind_vertices(ogpu_next_encoder *, uint32_t first, uint32_t count, const ogpu_next_span *, ogpu_next_host_span scratch);
void ogpu_next_draw_indexed(ogpu_next_encoder *, const ogpu_next_draw_desc *);
void ogpu_next_draw_indirect(ogpu_next_encoder *, const ogpu_next_indirect *);
void ogpu_next_draw_indexed_indirect(ogpu_next_encoder *, const ogpu_next_indirect *);
enum { OGPU_NEXT_QUERY_TIMESTAMP = 1, OGPU_NEXT_QUERY_OCCLUSION = 2, OGPU_NEXT_QUERY_PIPELINE_STATISTICS = 3 };
enum { OGPU_NEXT_QUERY_PRECISE = 1 };
#define OGPU_NEXT_STATISTIC_INPUT_VERTICES UINT64_C(1)
#define OGPU_NEXT_STATISTIC_INPUT_PRIMITIVES UINT64_C(2)
#define OGPU_NEXT_STATISTIC_VERTEX_INVOCATIONS UINT64_C(4)
#define OGPU_NEXT_STATISTIC_GEOMETRY_INVOCATIONS UINT64_C(8)
#define OGPU_NEXT_STATISTIC_GEOMETRY_PRIMITIVES UINT64_C(16)
#define OGPU_NEXT_STATISTIC_CLIPPING_INVOCATIONS UINT64_C(32)
#define OGPU_NEXT_STATISTIC_CLIPPING_PRIMITIVES UINT64_C(64)
#define OGPU_NEXT_STATISTIC_FRAGMENT_INVOCATIONS UINT64_C(128)
#define OGPU_NEXT_STATISTIC_TESS_CONTROL_PATCHES UINT64_C(256)
#define OGPU_NEXT_STATISTIC_TESS_EVALUATION_INVOCATIONS UINT64_C(512)
#define OGPU_NEXT_STATISTIC_COMPUTE_INVOCATIONS UINT64_C(1024)
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
 * UNINITIALIZED until a recorded reset executes. count>0; statistics=0 for
 * timestamps/occlusion. PIPELINE_STATISTICS requires enabled PIPELINE_STATISTICS
 * and a nonempty statistics mask contained in query_limits.statistics (physical
 * capability, not implicit enabling). Geometry/tessellation counters do not enable
 * those shader stages. No implicit creation/reset/recycling, CPU readback,
 * retained list reference or wait-on-destroy. Caller owns reset/use/resolve order
 * and all recorded/pending lifetimes, including independent or simultaneous lists.
 * Replaying a reset/write list concurrently against the same slots is invalid;
 * independent pools/ranges remain usable independently. Host recording is local
 * to command arenas; there is no device-wide or query-pool recording mutex.
 *
 * Occlusion requires RASTER. One query of EACH type may be active per list;
 * occlusion and statistics may overlap. Begin/end match pool/index and render
 * scope. An outside-scope query may enclose complete scopes. An inside-scope query
 * must end before render_end. Begin flags=0, or PRECISE for occlusion with enabled
 * PRECISE_OCCLUSION; only PRECISE promises the actual number of passing samples.
 * Statistics use flags=0 and require a queue supporting every selected operation:
 * COMPUTE for compute invocations, GRAPHICS for all other counters. A compute-only
 * query does not require RASTER. Native counting rules apply, not a portable shader
 * cost model or an exact count of unique source invocations.
 * Timestamp requires nonzero queue_info.timestamp_bits and a scope lowering to
 * one supported native stage bit (including VERTEX shader and narrow transfer/
 * input/depth scopes). HOST, zero and masks lowering to multiple native bits,
 * such as DEPTH, are rejected. The requested stage can measure
 * a later native stage; it is not a whole-list completion or calibrated host clock.
 * Timestamp period is nanoseconds per tick. Mask to the queue's valid low bits and
 * account for wrap; compare only within a known compatible device/queue clock
 * domain. timestamp_compute_graphics is native timestampComputeAndGraphics, not
 * an extra clock-synchronization guarantee. No CPU/GPU clock calibration API yet.
 *
 * Reset/resolve are outside rendering on graphics/compute queues; first/count
 * names a nonempty in-bounds range with no active overlapping query. Resolve
 * writes COPY_DST memory: one unsigned 32-bit result, or 64-bit with RESULT_64.
 * Statistics write one value per selected bit in ascending bit order; optional
 * availability follows ALL values at the same width (0 unavailable, nonzero ready).
 * Stride is bytes between records; when count>1, nonzero and aligned to result
 * width. Destination must fit every record and be width-aligned; overlapping
 * records are not repacked or given an OGPU write-order guarantee. WAIT is
 * an EXPLICIT GPU-side wait, never a host wait. Without WAIT, result data must not
 * be interpreted before available; PARTIAL permits interim occlusion/statistics values.
 * Queried slots must have been reset and issued by prior execution; do not resolve
 * never-issued slots. Timestamp PARTIAL is invalid. Writes need a subsequent
 * COPY/WRITE dependency before host/shader use. Completion and cache invalidation
 * remain explicit; result availability does not prove unrelated work complete.
 */
ogpu_next_status ogpu_next_query_pool_create(ogpu_next_device *, const ogpu_next_query_pool_desc *, ogpu_next_query_pool **);
void ogpu_next_query_pool_destroy(ogpu_next_query_pool *);
void ogpu_next_queries_reset(ogpu_next_encoder *, ogpu_next_query_pool *, uint32_t first, uint32_t count);
void ogpu_next_query_begin(ogpu_next_encoder *, ogpu_next_query_pool *, uint32_t index, uint32_t flags);
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
/* One native submission call for count ordered batches. Each retains its own
 * list/wait/signal boundary; batches are not flattened or submitted separately.
 * Each descriptor supplies its usual submit scratch. Additional batch scratch
 * stores only translated native batch headers. ALL scratch regions must be
 * mutually disjoint and disjoint from input records, borrowed for this call only.
 * Every batch is validated before native submission; on local failure no batch
 * is submitted and no one-shot is consumed (scratch may have been written).
 * Native OOM leaves ALL batches retryable. Other native failures follow submit's
 * terminal policy. On success all one-shot lists are consumed. Caller guarantees
 * duplicate-list legality across the entire call, and all native timeline ordering
 * and pending rules. Duplicate wait/signal restrictions apply within each batch;
 * a later batch may wait for an earlier batch's signal. count=0 is a no-op on a
 * live, non-lost device, accepts descs=NULL and requires no scratch. No allocation,
 * retained submission object, automatic timeline or host wait is added.
 */
ogpu_next_status ogpu_next_submit_batch_scratch_requirements(uint32_t count, ogpu_next_host_requirements *);
ogpu_next_status ogpu_next_queue_submit_batch(ogpu_next_queue *, uint32_t count, const ogpu_next_submit_desc *descs, ogpu_next_host_span scratch);

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
/* Independent device-only split dependency. Creation is cold and initially
 * unsignaled. The caller keeps split/device and referenced resources alive through
 * all recorded future/pending uses; destruction never waits or retains commands.
 * Objects are not arena-local: endpoints may be recorded independently, but every
 * acquire must follow its matching release on the SAME native queue in submission
 * order. This is not a timeline, cross-queue wait, or general GPU-address semaphore.
 * Compute/graphics domains only, outside rendering, no HOST scope. Primary and
 * standalone secondary lists are allowed. Release and acquire dependencies must
 * match exactly (stages/accesses/ranges/layouts/ownership/flags and array order;
 * host addresses/scratch may differ). Caller proves matching and valid sequencing;
 * no endpoint registry, inferred hazards or automatic ownership handoff exists.
 * A release on an already-signaled object does not create a new dependency.
 * Reset must be explicitly ordered after ALL earlier acquires/releases and before
 * the next release. Native reset/set lack an implicit ordering edge: use explicit
 * execution dependencies (e.g. ALL-to-ALL) or completed executions as appropriate.
 * Replay/simultaneous-list permission does not make shared event cycles independent;
 * caller must prevent races and stale signals across every execution.
 */
ogpu_next_status ogpu_next_split_create(ogpu_next_device *, ogpu_next_split **);
void ogpu_next_split_destroy(ogpu_next_split *);
void ogpu_next_split_reset(ogpu_next_encoder *, const ogpu_next_split *, ogpu_next_stages before);
void ogpu_next_split_release(ogpu_next_encoder *, const ogpu_next_split *, const ogpu_next_dependency *);
typedef struct ogpu_next_split_dependency {
    const ogpu_next_split *split;
    const ogpu_next_dependency *dependency;
} ogpu_next_split_dependency;
ogpu_next_status ogpu_next_split_scratch_requirements(uint32_t count, ogpu_next_host_requirements *);
/* Batch acquire uses one native wait. Every local record validates first; failure
 * records no wait and poisons end. count=0 is a no-op on a valid encoder/domain.
 * Header scratch AND each dependency's range scratch must be mutually disjoint,
 * aligned and disjoint from inputs. All description/scratch storage is borrowed
 * only during the call and can be overwritten immediately. Duplicate event use,
 * endpoint compatibility and asynchronous lifetime are trusted caller obligations.
 */
void ogpu_next_split_acquire(ogpu_next_encoder *, uint32_t count, const ogpu_next_split_dependency *, ogpu_next_host_span scratch);
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
typedef struct ogpu_next_image_transfer {
    ogpu_next_image_region source, destination;
    ogpu_next_image_state source_state, destination_state;
} ogpu_next_image_transfer;
enum { OGPU_NEXT_RESOLVE_NATIVE_COLOR = 1 };
/* Image-to-image operations: both region extents/layer counts/aspects match;
 * source COPY_SRC usage/state (or GENERAL), destination COPY_DST (or GENERAL).
 * No implicit layouts, staging, allocation or resource retention. Caller proves
 * disjoint source/destination memory regions, even for distinct aliasing images,
 * and consistent layouts for shared subresources. Same-image disjoint copies
 * are allowed; GENERAL is needed when one subresource is both source/destination.
 * Current copy profile: matching dimensionality/sample counts, uncompressed
 * color formats of equal texel size (raw bits, no conversion), or identical
 * depth/stencil formats. Offsets, mip/layer bounds and queue granularity checked.
 * Depth/stencil copies currently require a graphics family. 2D-array/3D cross-
 * dimensional mappings, compressed/multiplanar and depth/color reinterpretation
 * remain profiles to add, not silently staged.
 * Resolve: graphics queue, multisample source, single-sample destination, identical
 * color format with native color-attachment format support. NATIVE_COLOR preserves
 * Vulkan's implementation-dependent default: float/normalized resolve mode and
 * precision are native; integer formats select one sample. It does NOT promise
 * explicit average/sample-zero or a particular sRGB transfer-function policy.
 * Depth/stencil and explicit-mode resolve need further feature/profile enabling.
 * These are outside-rendering transfer operations, separate from attachment resolve.
 */
void ogpu_next_copy_image(ogpu_next_encoder *, ogpu_next_image *dst, ogpu_next_image *src, const ogpu_next_image_transfer *);
void ogpu_next_resolve_image(ogpu_next_encoder *, ogpu_next_image *dst, ogpu_next_image *src, const ogpu_next_image_transfer *, uint32_t mode);
typedef struct ogpu_next_blit_region {
    uint32_t aspect, mip, first_layer, layer_count;
    ogpu_next_offset offsets[2];
} ogpu_next_blit_region;
typedef struct ogpu_next_image_blit {
    ogpu_next_blit_region source, destination;
    ogpu_next_image_state source_state, destination_state;
    uint32_t filter, reserved;
} ogpu_next_image_blit;
/* Native single-sample blit: explicit scaling, axis reversal and format conversion.
 * Graphics family required, not RASTER enabling; outside rendering, BLIT stage.
 * COPY_SRC/DST usage and respective states or GENERAL. reserved=0; filter is
 * NEAREST or LINEAR with native format support (no fallback shader or staging).
 * Both endpoint coordinates lie within their mip, inclusive. Endpoints may be
 * reversed independently; only destination zero-size axes are forbidden.
 * Inactive dimensions use exactly [0,1]. Aspects/layer counts match; if either
 * image is 3D, both base layers are zero and layer counts one in this profile.
 * Unsigned integer colors convert only to unsigned integer colors; depth/stencil
 * requires identical formats and NEAREST. Other supported colors convert via
 * native float/filter/sRGB rules. Rounding/filter precision is implementation-
 * dependent, not a portable bitwise numerical kernel or custom sampling policy.
 * Caller proves destination memory does not overlap ANY potentially sampled texel
 * (including filter footprints, not merely the declared source box). Same-image
 * mip generation is allowed with explicit per-mip barriers. Shared subresources
 * require GENERAL. No automatic mip chain, transition, barrier or retention.
 */
void ogpu_next_blit_image(ogpu_next_encoder *, ogpu_next_image *dst, ogpu_next_image *src, const ogpu_next_image_blit *);
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
