#ifndef OGPU_NEXT_H
#define OGPU_NEXT_H
/* Experimental foundation revision 12, not ABI 20; the default Linux SDK surface.
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
#define OGPU_NEXT_VERSION 12u
typedef struct ogpu_next_discovery ogpu_next_discovery;
typedef struct ogpu_next_adapter ogpu_next_adapter;
typedef struct ogpu_next_device ogpu_next_device;
typedef struct ogpu_next_descriptor_heap ogpu_next_descriptor_heap;
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
    OGPU_NEXT_COMPILE_REQUIRED = 3, OGPU_NEXT_SUBOPTIMAL = 4,
    OGPU_NEXT_INVALID = -1, OGPU_NEXT_UNSUPPORTED = -2,
    OGPU_NEXT_OUT_OF_MEMORY = -3, OGPU_NEXT_DEVICE_LOST = -4,
    OGPU_NEXT_CAPACITY = -5, OGPU_NEXT_BACKEND_ERROR = -6,
    OGPU_NEXT_VERSION_MISMATCH = -7, OGPU_NEXT_INTERNAL_ERROR = -8,
    OGPU_NEXT_SURFACE_LOST = -9, OGPU_NEXT_OUT_OF_DATE = -10
};
typedef uint64_t ogpu_next_address;
typedef uint64_t ogpu_next_stages;
typedef uint64_t ogpu_next_access;
typedef uint32_t ogpu_next_feature;
typedef struct ogpu_next_features {
    const ogpu_next_feature *ids;
    uint32_t count, reserved;
} ogpu_next_features;
/* Feature IDs are opaque nonzero identifiers, not flags. Lists have no fixed
 * capacity; reserved=0. Input IDs may appear in any order; duplicates are
 * idempotent. Unknown IDs return UNSUPPORTED, zero returns INVALID. count=0
 * ignores ids. Inputs are borrowed for the host call only.
 * Discovery returns sorted, unique available/enabled lists in immutable storage
 * owned by the queried adapter/device. These pointers stay valid until discovery
 * (adapter query) or device (device query) destruction. Copy IDs to retain them
 * beyond that lifetime. Do not modify or free output ID storage. A zero-length output list has ids=NULL.
 * Selection/requirement checks are cold; command recording does not search lists.
 */
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
    OGPU_NEXT_QUERY_MEMORY_BUDGETS = 16, /* Only memory_budget_query, not immutable queries. */
    OGPU_NEXT_DEVICE_DESC = 100, OGPU_NEXT_MEMORY_DESC = 101,
    OGPU_NEXT_ARENA_DESC = 102, OGPU_NEXT_RECORDING_DESC = 103,
    OGPU_NEXT_IMAGE_DESC = 106, OGPU_NEXT_VIEW_DESC = 107,
    OGPU_NEXT_SAMPLER_DESC = 108,
    OGPU_NEXT_EXECUTABLE_DESC = 110,
    OGPU_NEXT_SHADER_REQUIREMENTS = 112, OGPU_NEXT_SPECIALIZATION = 113,
    OGPU_NEXT_GRAPHICS_STATE = 114, OGPU_NEXT_RENDER_DESC = 115,
    OGPU_NEXT_VIEWPORT_STATE = 116, OGPU_NEXT_QUERY_POOL_DESC = 117,
    OGPU_NEXT_EXECUTABLE_CACHE_DESC = 118, OGPU_NEXT_VERTEX_INPUT = 119,
    OGPU_NEXT_SUBGROUP_STATE = 120, OGPU_NEXT_INHERITANCE = 121,
    OGPU_NEXT_RENDER_INHERITANCE = 122, OGPU_NEXT_IMAGE_PROFILE = 123,
    OGPU_NEXT_RASTER_OPTIONS = 124
};
/* Version 1 records require exact byte_size, flags=0 and next=NULL unless an
 * operation explicitly permits an extension. Unknown kinds/versions are UNSUPPORTED, not ignored.
 * Query capacity=0 obtains count with OK and no data access. Otherwise capacity
 * must fit the entire result: CAPACITY writes count but no array elements.
 * element_size must exactly match the record type, even for count-only queries.
 * Except OK/CAPACITY, queries leave all outputs unchanged. Immutable adapter/device
 * snapshot queries do not allocate; optional live queries state any exceptions.
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
typedef struct ogpu_next_memory_budget {
    uint64_t budget, usage;
    uint32_t heap, reserved;
} ogpu_next_memory_budget;
/* Explicit live cold-path query: header kind QUERY_MEMORY_BUDGETS and element_size
 * sizeof(memory_budget). Uses the usual count/capacity protocol; count-only and
 * insufficient-capacity calls do not sample the driver. Full calls sample afresh.
 * One record per physical heap, IDs matching QUERY_MEMORY_HEAPS. budget is an
 * estimate of total process usage before pressure/performance degradation, NOT
 * additional free bytes or an allocation guarantee. usage is estimated process
 * allocation, may include other devices/libraries, and may exceed budget.
 * Values can change asynchronously and need not track each allocation immediately.
 * No optional feature enabling is required; UNSUPPORTED preserves count/data when
 * the native budget query is unavailable. Static adapter/device queries do not
 * accept this kind. CAPACITY changes only count; other failures preserve outputs.
 * Safe concurrently with independent allocation/query operations on a live device;
 * no OGPU allocation, polling, GPU wait, eviction or automatic OOM policy. */
ogpu_next_status ogpu_next_memory_budget_query(const ogpu_next_device *, ogpu_next_query *);
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
/* Independent per-format permissions for storage shaders declaring an unknown
 * image format, not device enables. Also require STORAGE, a matching numeric
 * class and native texel component rules. Typed storage needs STORAGE and a
 * compatible shader format; there is no separate extended-formats device gate
 * on this baseline. */
#define OGPU_NEXT_FORMAT_STORAGE_READ_WITHOUT_FORMAT UINT64_C(2048)
#define OGPU_NEXT_FORMAT_STORAGE_WRITE_WITHOUT_FORMAT UINT64_C(4096)
#define OGPU_NEXT_FORMAT_BUFFER_VERTEX UINT64_C(1)
#define OGPU_NEXT_FORMAT_BUFFER_UNIFORM_TEXEL UINT64_C(2)
#define OGPU_NEXT_FORMAT_BUFFER_STORAGE_TEXEL UINT64_C(4)
#define OGPU_NEXT_FORMAT_BUFFER_ATOMIC_TEXEL UINT64_C(8)
typedef struct ogpu_next_format_info {
    ogpu_next_format format;
    uint32_t aspects;
    uint64_t optimal_features, linear_features, buffer_features;
    /* Copy footprint, NOT optimal-image allocation size. Extent in texels;
     * bytes per block independently for COLOR, DEPTH, STENCIL (zero if absent).
     * Ordinary formats have extent {1,1,1}; compressed formats count whole blocks,
     * including partial blocks at mip edges. Depth/stencil copies are separate. */
    uint32_t copy_block_extent[3], copy_block_bytes[3];
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
#define OGPU_NEXT_FEATURE_MULTIVIEW UINT32_C(62)
#define OGPU_NEXT_FEATURE_MESH_MULTIVIEW UINT32_C(63)
#define OGPU_NEXT_FEATURE_PIPELINE_SHADING_RATE UINT32_C(64)
#define OGPU_NEXT_FEATURE_PRIMITIVE_SHADING_RATE UINT32_C(65)
#define OGPU_NEXT_FEATURE_ATTACHMENT_SHADING_RATE UINT32_C(66)
#define OGPU_NEXT_FEATURE_MESH_PRIMITIVE_SHADING_RATE UINT32_C(67)
#define OGPU_NEXT_FEATURE_LOCAL_READ UINT32_C(68)
#define OGPU_NEXT_FEATURE_LARGE_POINTS UINT32_C(74)
#define OGPU_NEXT_FEATURE_DEPTH_CLIP UINT32_C(75)
#define OGPU_NEXT_FEATURE_VERTEX_DIVISOR UINT32_C(76)
#define OGPU_NEXT_FEATURE_VERTEX_ZERO_DIVISOR UINT32_C(77)
#define OGPU_NEXT_FEATURE_LIST_RESTART UINT32_C(78)
#define OGPU_NEXT_FEATURE_PATCH_RESTART UINT32_C(79)
#define OGPU_NEXT_FEATURE_CONSERVATIVE_RASTER UINT32_C(80)
#define OGPU_NEXT_FEATURE_PROVOKING_LAST UINT32_C(81)
#define OGPU_NEXT_FEATURE_DEPTH_NEGATIVE_ONE_TO_ONE UINT32_C(82)
#define OGPU_NEXT_FEATURE_RECTANGULAR_LINES UINT32_C(83)
#define OGPU_NEXT_FEATURE_BRESENHAM_LINES UINT32_C(84)
#define OGPU_NEXT_FEATURE_SMOOTH_LINES UINT32_C(85)
#define OGPU_NEXT_FEATURE_STIPPLED_RECTANGULAR_LINES UINT32_C(86)
#define OGPU_NEXT_FEATURE_STIPPLED_BRESENHAM_LINES UINT32_C(87)
#define OGPU_NEXT_FEATURE_STIPPLED_SMOOTH_LINES UINT32_C(88)
#define OGPU_NEXT_FEATURE_DYNAMIC_CONSERVATIVE_MODE UINT32_C(89)
#define OGPU_NEXT_FEATURE_DYNAMIC_EXTRA_OVERESTIMATION UINT32_C(90)
#define OGPU_NEXT_FEATURE_DYNAMIC_PROVOKING_VERTEX UINT32_C(91)
#define OGPU_NEXT_FEATURE_DYNAMIC_DEPTH_CLIP UINT32_C(92)
#define OGPU_NEXT_FEATURE_DYNAMIC_DEPTH_CONVENTION UINT32_C(93)
#define OGPU_NEXT_FEATURE_DYNAMIC_LINE_MODE UINT32_C(94)
#define OGPU_NEXT_FEATURE_DYNAMIC_LINE_STIPPLE_ENABLE UINT32_C(95)



#define OGPU_NEXT_FEATURE_RASTER UINT32_C(1)
#define OGPU_NEXT_FEATURE_FLOAT16 UINT32_C(2)
#define OGPU_NEXT_FEATURE_UNIFIED_IMAGES UINT32_C(3)
#define OGPU_NEXT_FEATURE_SAMPLER_ANISOTROPY UINT32_C(4)
#define OGPU_NEXT_FEATURE_CACHE_CONTROL UINT32_C(5)
#define OGPU_NEXT_FEATURE_INT8 UINT32_C(6)
#define OGPU_NEXT_FEATURE_INT16 UINT32_C(7)
#define OGPU_NEXT_FEATURE_INT64 UINT32_C(8)
#define OGPU_NEXT_FEATURE_FLOAT64 UINT32_C(9)
#define OGPU_NEXT_FEATURE_STORAGE8 UINT32_C(10)
#define OGPU_NEXT_FEATURE_BUFFER_ATOMIC64 UINT32_C(11)
#define OGPU_NEXT_FEATURE_SHARED_ATOMIC64 UINT32_C(12)
#define OGPU_NEXT_FEATURE_SUBGROUP_SIZE_CONTROL UINT32_C(13)
#define OGPU_NEXT_FEATURE_FULL_SUBGROUPS UINT32_C(14)
#define OGPU_NEXT_FEATURE_SUBGROUP_EXTENDED_TYPES UINT32_C(15)
#define OGPU_NEXT_FEATURE_PIPELINE_STATISTICS UINT32_C(16)
#define OGPU_NEXT_FEATURE_PRECISE_OCCLUSION UINT32_C(17)
#define OGPU_NEXT_FEATURE_INHERITED_QUERIES UINT32_C(18)
#define OGPU_NEXT_FEATURE_VERTEX_STORES_ATOMICS UINT32_C(19)
#define OGPU_NEXT_FEATURE_FRAGMENT_STORES_ATOMICS UINT32_C(20)
#define OGPU_NEXT_FEATURE_INDIRECT_FIRST_INSTANCE UINT32_C(21)
#define OGPU_NEXT_FEATURE_INDEPENDENT_BLEND UINT32_C(22)
#define OGPU_NEXT_FEATURE_DEPTH_CLAMP UINT32_C(23)
#define OGPU_NEXT_FEATURE_DEPTH_BIAS_CLAMP UINT32_C(24)
#define OGPU_NEXT_FEATURE_NON_SOLID_FILL UINT32_C(25)
#define OGPU_NEXT_FEATURE_WIDE_LINES UINT32_C(26)
#define OGPU_NEXT_FEATURE_DEPTH_BOUNDS UINT32_C(27)
#define OGPU_NEXT_FEATURE_SAMPLE_SHADING UINT32_C(28)
#define OGPU_NEXT_FEATURE_ALPHA_TO_ONE UINT32_C(29)
#define OGPU_NEXT_FEATURE_UNUSED_ATTACHMENTS UINT32_C(30)
#define OGPU_NEXT_FEATURE_LOGIC_OP UINT32_C(31)
#define OGPU_NEXT_FEATURE_DUAL_SOURCE_BLEND UINT32_C(32)
#define OGPU_NEXT_FEATURE_SHADER_OUTPUT_LAYER UINT32_C(33)
#define OGPU_NEXT_FEATURE_BUFFER_FLOAT32_ATOMICS UINT32_C(34)
#define OGPU_NEXT_FEATURE_BUFFER_FLOAT32_ATOMIC_ADD UINT32_C(35)
#define OGPU_NEXT_FEATURE_BUFFER_FLOAT32_ATOMIC_MIN_MAX UINT32_C(36)
#define OGPU_NEXT_FEATURE_SHARED_FLOAT32_ATOMICS UINT32_C(37)
#define OGPU_NEXT_FEATURE_SHARED_FLOAT32_ATOMIC_ADD UINT32_C(38)
#define OGPU_NEXT_FEATURE_SHARED_FLOAT32_ATOMIC_MIN_MAX UINT32_C(39)
#define OGPU_NEXT_FEATURE_BUFFER_FLOAT64_ATOMICS UINT32_C(40)
#define OGPU_NEXT_FEATURE_BUFFER_FLOAT64_ATOMIC_ADD UINT32_C(41)
#define OGPU_NEXT_FEATURE_BUFFER_FLOAT64_ATOMIC_MIN_MAX UINT32_C(42)
#define OGPU_NEXT_FEATURE_SHARED_FLOAT64_ATOMICS UINT32_C(43)
#define OGPU_NEXT_FEATURE_SHARED_FLOAT64_ATOMIC_ADD UINT32_C(44)
#define OGPU_NEXT_FEATURE_SHARED_FLOAT64_ATOMIC_MIN_MAX UINT32_C(45)
#define OGPU_NEXT_FEATURE_COOPERATIVE_MATRIX UINT32_C(46)
#define OGPU_NEXT_FEATURE_VULKAN_MEMORY_MODEL UINT32_C(47)
#define OGPU_NEXT_FEATURE_VULKAN_MEMORY_MODEL_DEVICE_SCOPE UINT32_C(48)
#define OGPU_NEXT_FEATURE_VULKAN_MEMORY_MODEL_CHAINS UINT32_C(56)
/* Cube-array views and shader sampling; independent of raster-stage support. */
#define OGPU_NEXT_FEATURE_CUBE_ARRAY UINT32_C(57)
#define OGPU_NEXT_FEATURE_INTEGER_DOT UINT32_C(49)
#define OGPU_NEXT_FEATURE_BUFFER_FLOAT16_ATOMICS UINT32_C(50)
#define OGPU_NEXT_FEATURE_BUFFER_FLOAT16_ATOMIC_ADD UINT32_C(51)
#define OGPU_NEXT_FEATURE_BUFFER_FLOAT16_ATOMIC_MIN_MAX UINT32_C(52)
#define OGPU_NEXT_FEATURE_SHARED_FLOAT16_ATOMICS UINT32_C(53)
#define OGPU_NEXT_FEATURE_SHARED_FLOAT16_ATOMIC_ADD UINT32_C(54)
#define OGPU_NEXT_FEATURE_SHARED_FLOAT16_ATOMIC_MIN_MAX UINT32_C(55)
/* VULKAN_MEMORY_MODEL enables SPIR-V VulkanMemoryModel independently of matrix/scalar
 * features. DEVICE_SCOPE and CHAINS are independent and require the base feature
 * (otherwise device creation returns INVALID). Enabling the base without DEVICE_SCOPE
 * forbids Device memory scope in ALL shaders on that device, including GLSL450
 * artifacts. CHAINS permits native availability/visibility chains with more than
 * one element; it does not implicitly enable Device scope. Artifacts declare
 * every required feature. Scope/order/semantics and converged participation remain
 * shader contracts, not implied device-wide execution or forward-progress guarantees.
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
 * policy. FLOAT16/FLOAT64 operation/domain features have the same independence; their
 * arithmetic/type enabling must also be requested by artifacts using half/double.
 * Image, sparse-image and vector floating atomic profiles remain work.
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
    ogpu_next_features available, enabled;
    uint64_t max_timeline_difference;
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
 * queries: only explicitly requested optional features enabled, device_scope=1.
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
 * writes/atomics before rasterization (VERTEX includes tessellation/geometry/mesh)
 * and in FRAGMENT, respectively; they do not enable RASTER, additional
 * stages, 64-bit/floating atomics or a storage-image format. Artifacts declare the
 * required features; OGPU does not scan/rewrite shaders. Caller owns address validity,
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

/* Format IDs are OGPU values, not backend enum passthrough. Existing IDs retain
 * their values. The table covers Vulkan 1.0 core uncompressed formats with up to
 * 32-bit components, including scaled vertex formats and packed color/depth/stencil.
 * This vocabulary does NOT imply device support for any usage. Query FORMATS,
 * then exact image-profile limits/requirements; unsupported combinations reject.
 * PACK8/16/32 names list components from high to low bits of the packed word,
 * following the corresponding native format (X = unused, E = shared exponent).
 * Other color names list components in increasing memory order. No channel
 * rearrangement, conversion or decompression is implicit. BC1-7, ETC2/EAC and
 * 2D ASTC LDR formats are included, with per-format capability queries (no coarse
 * compression device-enable requirement). Multiplanar,
 * 64-bit-component and extension-specific formats remain separate profiles. */
enum {
    OGPU_NEXT_R8_UNORM = 1, OGPU_NEXT_RG8_UNORM = 2,
    OGPU_NEXT_RGBA8_UNORM = 3, OGPU_NEXT_RGBA8_SRGB = 4,
    OGPU_NEXT_BGRA8_UNORM = 5, OGPU_NEXT_BGRA8_SRGB = 6,
    OGPU_NEXT_R16_FLOAT = 7, OGPU_NEXT_RG16_FLOAT = 8, OGPU_NEXT_RGBA16_FLOAT = 9,
    OGPU_NEXT_R32_FLOAT = 10, OGPU_NEXT_R32_UINT = 11, OGPU_NEXT_RGBA32_FLOAT = 12,
    OGPU_NEXT_D16_UNORM = 13, OGPU_NEXT_D32_FLOAT = 14,
    OGPU_NEXT_D24_UNORM_S8_UINT = 15, OGPU_NEXT_D32_FLOAT_S8_UINT = 16,
    OGPU_NEXT_RG32_FLOAT = 17, OGPU_NEXT_RGB32_FLOAT = 18,
    OGPU_NEXT_RGBA16_UNORM = 19,
    OGPU_NEXT_R4G4_UNORM_PACK8 = 20,
    OGPU_NEXT_R4G4B4A4_UNORM_PACK16 = 21,
    OGPU_NEXT_B4G4R4A4_UNORM_PACK16 = 22,
    OGPU_NEXT_R5G6B5_UNORM_PACK16 = 23,
    OGPU_NEXT_B5G6R5_UNORM_PACK16 = 24,
    OGPU_NEXT_R5G5B5A1_UNORM_PACK16 = 25,
    OGPU_NEXT_B5G5R5A1_UNORM_PACK16 = 26,
    OGPU_NEXT_A1R5G5B5_UNORM_PACK16 = 27,
    OGPU_NEXT_R8_SNORM = 28,
    OGPU_NEXT_R8_USCALED = 29,
    OGPU_NEXT_R8_SSCALED = 30,
    OGPU_NEXT_R8_UINT = 31,
    OGPU_NEXT_R8_SINT = 32,
    OGPU_NEXT_R8_SRGB = 33,
    OGPU_NEXT_RG8_SNORM = 34,
    OGPU_NEXT_RG8_USCALED = 35,
    OGPU_NEXT_RG8_SSCALED = 36,
    OGPU_NEXT_RG8_UINT = 37,
    OGPU_NEXT_RG8_SINT = 38,
    OGPU_NEXT_RG8_SRGB = 39,
    OGPU_NEXT_RGB8_UNORM = 40,
    OGPU_NEXT_RGB8_SNORM = 41,
    OGPU_NEXT_RGB8_USCALED = 42,
    OGPU_NEXT_RGB8_SSCALED = 43,
    OGPU_NEXT_RGB8_UINT = 44,
    OGPU_NEXT_RGB8_SINT = 45,
    OGPU_NEXT_RGB8_SRGB = 46,
    OGPU_NEXT_BGR8_UNORM = 47,
    OGPU_NEXT_BGR8_SNORM = 48,
    OGPU_NEXT_BGR8_USCALED = 49,
    OGPU_NEXT_BGR8_SSCALED = 50,
    OGPU_NEXT_BGR8_UINT = 51,
    OGPU_NEXT_BGR8_SINT = 52,
    OGPU_NEXT_BGR8_SRGB = 53,
    OGPU_NEXT_RGBA8_SNORM = 54,
    OGPU_NEXT_RGBA8_USCALED = 55,
    OGPU_NEXT_RGBA8_SSCALED = 56,
    OGPU_NEXT_RGBA8_UINT = 57,
    OGPU_NEXT_RGBA8_SINT = 58,
    OGPU_NEXT_BGRA8_SNORM = 59,
    OGPU_NEXT_BGRA8_USCALED = 60,
    OGPU_NEXT_BGRA8_SSCALED = 61,
    OGPU_NEXT_BGRA8_UINT = 62,
    OGPU_NEXT_BGRA8_SINT = 63,
    OGPU_NEXT_A8B8G8R8_UNORM_PACK32 = 64,
    OGPU_NEXT_A8B8G8R8_SNORM_PACK32 = 65,
    OGPU_NEXT_A8B8G8R8_USCALED_PACK32 = 66,
    OGPU_NEXT_A8B8G8R8_SSCALED_PACK32 = 67,
    OGPU_NEXT_A8B8G8R8_UINT_PACK32 = 68,
    OGPU_NEXT_A8B8G8R8_SINT_PACK32 = 69,
    OGPU_NEXT_A8B8G8R8_SRGB_PACK32 = 70,
    OGPU_NEXT_A2R10G10B10_UNORM_PACK32 = 71,
    OGPU_NEXT_A2R10G10B10_SNORM_PACK32 = 72,
    OGPU_NEXT_A2R10G10B10_USCALED_PACK32 = 73,
    OGPU_NEXT_A2R10G10B10_SSCALED_PACK32 = 74,
    OGPU_NEXT_A2R10G10B10_UINT_PACK32 = 75,
    OGPU_NEXT_A2R10G10B10_SINT_PACK32 = 76,
    OGPU_NEXT_A2B10G10R10_UNORM_PACK32 = 77,
    OGPU_NEXT_A2B10G10R10_SNORM_PACK32 = 78,
    OGPU_NEXT_A2B10G10R10_USCALED_PACK32 = 79,
    OGPU_NEXT_A2B10G10R10_SSCALED_PACK32 = 80,
    OGPU_NEXT_A2B10G10R10_UINT_PACK32 = 81,
    OGPU_NEXT_A2B10G10R10_SINT_PACK32 = 82,
    OGPU_NEXT_R16_UNORM = 83,
    OGPU_NEXT_R16_SNORM = 84,
    OGPU_NEXT_R16_USCALED = 85,
    OGPU_NEXT_R16_SSCALED = 86,
    OGPU_NEXT_R16_UINT = 87,
    OGPU_NEXT_R16_SINT = 88,
    OGPU_NEXT_RG16_UNORM = 89,
    OGPU_NEXT_RG16_SNORM = 90,
    OGPU_NEXT_RG16_USCALED = 91,
    OGPU_NEXT_RG16_SSCALED = 92,
    OGPU_NEXT_RG16_UINT = 93,
    OGPU_NEXT_RG16_SINT = 94,
    OGPU_NEXT_RGB16_UNORM = 95,
    OGPU_NEXT_RGB16_SNORM = 96,
    OGPU_NEXT_RGB16_USCALED = 97,
    OGPU_NEXT_RGB16_SSCALED = 98,
    OGPU_NEXT_RGB16_UINT = 99,
    OGPU_NEXT_RGB16_SINT = 100,
    OGPU_NEXT_RGB16_FLOAT = 101,
    OGPU_NEXT_RGBA16_SNORM = 102,
    OGPU_NEXT_RGBA16_USCALED = 103,
    OGPU_NEXT_RGBA16_SSCALED = 104,
    OGPU_NEXT_RGBA16_UINT = 105,
    OGPU_NEXT_RGBA16_SINT = 106,
    OGPU_NEXT_R32_SINT = 107,
    OGPU_NEXT_RG32_UINT = 108,
    OGPU_NEXT_RG32_SINT = 109,
    OGPU_NEXT_RGB32_UINT = 110,
    OGPU_NEXT_RGB32_SINT = 111,
    OGPU_NEXT_RGBA32_UINT = 112,
    OGPU_NEXT_RGBA32_SINT = 113,
    OGPU_NEXT_B10G11R11_UFLOAT_PACK32 = 114,
    OGPU_NEXT_E5B9G9R9_UFLOAT_PACK32 = 115,
    OGPU_NEXT_X8_D24_UNORM_PACK32 = 116,
    OGPU_NEXT_S8_UINT = 117,
    OGPU_NEXT_D16_UNORM_S8_UINT = 118,
    OGPU_NEXT_BC1_RGB_UNORM_BLOCK = 119,
    OGPU_NEXT_BC1_RGB_SRGB_BLOCK = 120,
    OGPU_NEXT_BC1_RGBA_UNORM_BLOCK = 121,
    OGPU_NEXT_BC1_RGBA_SRGB_BLOCK = 122,
    OGPU_NEXT_BC2_UNORM_BLOCK = 123,
    OGPU_NEXT_BC2_SRGB_BLOCK = 124,
    OGPU_NEXT_BC3_UNORM_BLOCK = 125,
    OGPU_NEXT_BC3_SRGB_BLOCK = 126,
    OGPU_NEXT_BC4_UNORM_BLOCK = 127,
    OGPU_NEXT_BC4_SNORM_BLOCK = 128,
    OGPU_NEXT_BC5_UNORM_BLOCK = 129,
    OGPU_NEXT_BC5_SNORM_BLOCK = 130,
    OGPU_NEXT_BC6H_UFLOAT_BLOCK = 131,
    OGPU_NEXT_BC6H_SFLOAT_BLOCK = 132,
    OGPU_NEXT_BC7_UNORM_BLOCK = 133,
    OGPU_NEXT_BC7_SRGB_BLOCK = 134,
    OGPU_NEXT_ETC2_RGB8_UNORM_BLOCK = 135,
    OGPU_NEXT_ETC2_RGB8_SRGB_BLOCK = 136,
    OGPU_NEXT_ETC2_RGB8A1_UNORM_BLOCK = 137,
    OGPU_NEXT_ETC2_RGB8A1_SRGB_BLOCK = 138,
    OGPU_NEXT_ETC2_RGBA8_UNORM_BLOCK = 139,
    OGPU_NEXT_ETC2_RGBA8_SRGB_BLOCK = 140,
    OGPU_NEXT_EAC_R11_UNORM_BLOCK = 141,
    OGPU_NEXT_EAC_R11_SNORM_BLOCK = 142,
    OGPU_NEXT_EAC_RG11_UNORM_BLOCK = 143,
    OGPU_NEXT_EAC_RG11_SNORM_BLOCK = 144,
    OGPU_NEXT_ASTC_4X4_UNORM_BLOCK = 145,
    OGPU_NEXT_ASTC_4X4_SRGB_BLOCK = 146,
    OGPU_NEXT_ASTC_5X4_UNORM_BLOCK = 147,
    OGPU_NEXT_ASTC_5X4_SRGB_BLOCK = 148,
    OGPU_NEXT_ASTC_5X5_UNORM_BLOCK = 149,
    OGPU_NEXT_ASTC_5X5_SRGB_BLOCK = 150,
    OGPU_NEXT_ASTC_6X5_UNORM_BLOCK = 151,
    OGPU_NEXT_ASTC_6X5_SRGB_BLOCK = 152,
    OGPU_NEXT_ASTC_6X6_UNORM_BLOCK = 153,
    OGPU_NEXT_ASTC_6X6_SRGB_BLOCK = 154,
    OGPU_NEXT_ASTC_8X5_UNORM_BLOCK = 155,
    OGPU_NEXT_ASTC_8X5_SRGB_BLOCK = 156,
    OGPU_NEXT_ASTC_8X6_UNORM_BLOCK = 157,
    OGPU_NEXT_ASTC_8X6_SRGB_BLOCK = 158,
    OGPU_NEXT_ASTC_8X8_UNORM_BLOCK = 159,
    OGPU_NEXT_ASTC_8X8_SRGB_BLOCK = 160,
    OGPU_NEXT_ASTC_10X5_UNORM_BLOCK = 161,
    OGPU_NEXT_ASTC_10X5_SRGB_BLOCK = 162,
    OGPU_NEXT_ASTC_10X6_UNORM_BLOCK = 163,
    OGPU_NEXT_ASTC_10X6_SRGB_BLOCK = 164,
    OGPU_NEXT_ASTC_10X8_UNORM_BLOCK = 165,
    OGPU_NEXT_ASTC_10X8_SRGB_BLOCK = 166,
    OGPU_NEXT_ASTC_10X10_UNORM_BLOCK = 167,
    OGPU_NEXT_ASTC_10X10_SRGB_BLOCK = 168,
    OGPU_NEXT_ASTC_12X10_UNORM_BLOCK = 169,
    OGPU_NEXT_ASTC_12X10_SRGB_BLOCK = 170,
    OGPU_NEXT_ASTC_12X12_UNORM_BLOCK = 171,
    OGPU_NEXT_ASTC_12X12_SRGB_BLOCK = 172,
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
 * Discarded alias reuse can use a global memory dependency covering accesses
 * through the old alias plus an UNDEFINED/discard transition for the new image,
 * followed by full initialization before reading. An image-only dependency on
 * the new alias does not cover accesses through a distinct old alias. Discard
 * does not remove hazards or promise cross-alias content preservation.
 * Concurrent domains follow memory_desc rules, independently of backing.
 * View-format lists restrict permitted interpretations. Reinterpretation needs
 * MUTABLE_FORMAT and a compatible format class; depth/stencil keeps its format.
 * This revision excludes linear tiling, sparse/external/disjoint images, extended
 * usage, 2D views of 3D slices and multisampled storage images.
 * Such strategies are UNSUPPORTED, never emulated; broader profiles follow.
 * CUBE/CUBE_ARRAY views require a CUBE-compatible image. Cube-array views require
 * FEATURE_CUBE_ARRAY and a positive layer_count divisible by six (six layers per
 * cube). first_layer is in image layers, not cube indices, and need not be a
 * multiple of six. A shader's cube-array index is relative to the selected view.
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
    ogpu_next_descriptor_heap *resource_heap, *sampler_heap;
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
 * COMPUTE/VERTEX/FRAGMENT/MESH/TASK/TESS_CONTROL/TESS_EVALUATION/GEOMETRY
 * are shader stages too; the remaining bits are synchronization-only. */
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
    uint64_t max_resource_capacity, max_sampler_capacity;
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
 * attachment handles. Zero formats mean unused; reserved=0. view_mask must match the parent scope.
 * Both nonzero depth/stencil formats must match. Format translation uses caller
 * scratch, borrowed with all description data only during commands_begin. The
 * caller proves formats/count/samples match each eventual primary render scope.
 * This secondary starts/ends INSIDE that scope: do not call render_begin/end.
 * Standalone secondaries may record complete inline rendering scopes themselves.
 * Neither flavor inherits pipeline, arguments, vertex/index or viewport state:
 * initialize everything used. Nested-secondary execution is not enabled.
 * Optional resource_heap/sampler_heap inheritance pointers borrow heap objects
 * of the corresponding kind. At EVERY execute, caller proves the parent
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
 * Nonempty execution invalidates the primary's pipeline, argument bytes,
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
enum {
    OGPU_NEXT_HEAP_HOST = 0,   /* borrow ordinary host bytes; never shader visible */
    OGPU_NEXT_HEAP_MEMORY = 1, /* borrow explicit GPU backing and optional mapping */
    OGPU_NEXT_HEAP_NATIVE = 2  /* allocate an opaque native heap at creation */
};
enum {
    OGPU_NEXT_HEAP_SHADER_VISIBLE = 1,
    OGPU_NEXT_HEAP_CPU_WRITE = 2,
    OGPU_NEXT_HEAP_CPU_COPY_SOURCE = 4,
    OGPU_NEXT_HEAP_CPU_COPY_DESTINATION = 8
};

typedef struct ogpu_next_heap_request {
    uint32_t kind; /* existing RESOURCE or SAMPLER heap kind */
    uint32_t storage;
    uint32_t usage;
    uint32_t flags; /* zero */
    uint64_t capacity; /* native location units, includes any reserved interval */
} ogpu_next_heap_request;

typedef struct ogpu_next_heap_requirements {
    uint64_t unit_bytes; /* 0: opaque units, never a native handle increment */
    uint64_t backing_size;
    uint64_t backing_alignment;
    uint64_t reserved_size;
    uint64_t reserved_alignment; /* reservation values are in location units */
    uint64_t memory_usage; /* minimum backing usage; zero for HOST/NATIVE */
} ogpu_next_heap_requirements;

/* Creation is explicit and cold. capacity includes the reserved interval, whose
 * offset/size use native location units. HOST borrows host bytes and cannot be
 * shader visible. MEMORY borrows a GPU span and, for CPU operations, its actual
 * live mapping at the same origin. No implicit map, allocation, flush or fallback.
 * Vulkan supports HOST/MEMORY; NATIVE is unsupported. Unused spans must be empty.
 * flags=0; unknown kinds/storage/usage reject. All required extents, alignment,
 * usage and reservation bounds are checked. The bound range is exactly capacity
 * times unit_bytes, even with larger supplied backing. Queries leave outputs
 * unchanged on error; creation clears the output before validation.
 *
 * Device/backing/mapping outlive the heap and all recorded/future/pending uses.
 * Destruction never waits or destroys referenced resources. Reserved storage is
 * inaccessible until all referencing recordings are reset/destroyed, including
 * after completion. Reservations may overlap only when exactly identical and of
 * the same kind. Callers prove alias, lifetime, cache-atom and hazard obligations.
 */
typedef struct ogpu_next_heap_desc {
    ogpu_next_heap_request request;
    ogpu_next_span memory;      /* MEMORY only; borrowed */
    ogpu_next_host_span host;   /* HOST, or the actual mapped MEMORY interval */
    uint64_t reserved_offset;
    uint64_t reserved_size;     /* units, not bytes */
} ogpu_next_heap_desc;

typedef struct ogpu_next_descriptor_shape {
    uint64_t size;
    uint64_t alignment;
    uint64_t array_stride; /* units per descriptor in the native typed-index ABI */
} ogpu_next_descriptor_shape;

typedef struct ogpu_next_heap_location {
    ogpu_next_descriptor_heap *heap;
    uint64_t offset; /* units relative to this heap, never a host/GPU pointer */
} ogpu_next_heap_location;

typedef struct ogpu_next_descriptor_copy {
    ogpu_next_heap_location source;
    ogpu_next_heap_location destination;
    uint64_t size; /* units; source/destination device and heap kind must match */
} ogpu_next_descriptor_copy;

ogpu_next_status ogpu_next_heap_requirements_query(
    const ogpu_next_device *, const ogpu_next_heap_request *,
    ogpu_next_heap_requirements *);
ogpu_next_status ogpu_next_resource_descriptor_shape(
    const ogpu_next_device *, const ogpu_next_resource_descriptor *,
    ogpu_next_descriptor_shape *);
ogpu_next_status ogpu_next_sampler_descriptor_shape(
    const ogpu_next_device *, const ogpu_next_sampler_desc *,
    ogpu_next_descriptor_shape *);
ogpu_next_status ogpu_next_heap_create(
    ogpu_next_device *, const ogpu_next_heap_desc *, ogpu_next_descriptor_heap **);
void ogpu_next_heap_destroy(ogpu_next_descriptor_heap *);
/* Shape queries validate the actual description without encoding or creating
 * resources. Locations are aligned native units; size is descriptor extent and
 * array_stride is the typed native shader-index step. Writers check CPU_WRITE,
 * device/kind, extent, alignment and reservation exclusion. Heap contents are the
 * sole native representation; no recipes, ownership table or shadow mirror.
 *
 * Returns immutable creation/storage facts. Spans remain borrowed; no mapping,
 * allocation or recipe storage is created by this query. NATIVE has empty spans.
 */
void ogpu_next_heap_info(const ogpu_next_descriptor_heap *,
    ogpu_next_heap_desc *, ogpu_next_heap_requirements *);

enum {
    OGPU_NEXT_DESCRIPTOR_WRITE_RESOURCE = 0,
    OGPU_NEXT_DESCRIPTOR_WRITE_SAMPLER = 1,
    OGPU_NEXT_DESCRIPTOR_COPY = 2
};
ogpu_next_status ogpu_next_descriptor_scratch_requirements(
    const ogpu_next_device *, uint32_t operation, uint32_t count,
    ogpu_next_host_requirements *);
ogpu_next_status ogpu_next_write_resource_descriptors(
    const ogpu_next_device *, uint32_t count, const ogpu_next_resource_descriptor *,
    const ogpu_next_heap_location *, ogpu_next_host_span scratch);
ogpu_next_status ogpu_next_write_sampler_descriptors(
    const ogpu_next_device *, uint32_t count, const ogpu_next_sampler_desc *,
    const ogpu_next_heap_location *, ogpu_next_host_span scratch);
/* CPU-timeline copy. Source/destination require CPU_COPY_SOURCE/DESTINATION,
 * belong to this device and have matching kind. All ranges must fit capacity,
 * avoid reservations and be mutually nonoverlapping, including wrapper aliases.
 * Known host-range aliases are checked; the caller proves other aliases. Whole
 * batch preflight precedes any copy. No memmove ordering, hidden staging, mapping,
 * cache maintenance or GPU work. Scratch/inputs are disjoint from each other and
 * copied contents; scratch is aligned per query. count=0 permits NULL inputs and
 * empty scratch. Zero-sized copies validate objects/rights/ranges without touching
 * memory. Concurrent disjoint updates retain cache and execution obligations. */
ogpu_next_status ogpu_next_copy_descriptors(
    const ogpu_next_device *, uint32_t count, const ogpu_next_descriptor_copy *,
    ogpu_next_host_span scratch);
/* Binds this heap's kind, preserving the other kind. O(1) in heap capacity.
 * Retaining current native heap handles in the encoder is allowed; retaining
 * or reconstructing descriptor contents/resource ownership is not.
 */
void ogpu_next_bind_heap(ogpu_next_encoder *, ogpu_next_descriptor_heap *);


/* Prepared executable state. This implementation supports compute, vertex or mesh with
 * optional fragment SPIR-V with native heap/pointer access. Native artifacts, descriptor
 * set-to-heap mappings and launch extensions follow; these
 * are unimplemented profiles, not excluded designs. No runtime shader translation.
 * Preparation may allocate/compile. Binding, arguments and dispatch never allocate
 * OGPU storage, compile, retain resources, upload pointees or insert dependencies.
 */
enum { OGPU_NEXT_EXECUTABLE_COMPUTE = 1, OGPU_NEXT_EXECUTABLE_GRAPHICS = 2 };
enum { OGPU_NEXT_SHADER_SPIRV = 0 };
typedef struct ogpu_next_execution_limits {
    uint64_t max_inline_bytes;
    uint32_t max_groups[3], max_local_size[3], max_local_invocations;
    uint32_t max_shared_memory, argument_kinds;
} ogpu_next_execution_limits;
/* Optional mesh shading: explicitly enable MESH with RASTER; TASK additionally
 * requires MESH. No vertex-stage fallback. QUERY_MESH_LIMITS returns physical
 * limits (zero if the native extension is absent), not implicit enabling. */
#define OGPU_NEXT_FEATURE_MESH_SHADER UINT32_C(59)
#define OGPU_NEXT_FEATURE_TASK_SHADER UINT32_C(60)
#define OGPU_NEXT_STAGE_MESH UINT64_C(1048576)
#define OGPU_NEXT_STAGE_TASK UINT64_C(2097152)
enum { OGPU_NEXT_QUERY_MESH_LIMITS = 17, OGPU_NEXT_MESH_REQUIREMENTS = 129 };
typedef struct ogpu_next_mesh_workgroup_limits {
    uint32_t max_groups[3], max_total_groups, max_local[3], max_invocations, max_shared_memory;
} ogpu_next_mesh_workgroup_limits;
typedef struct ogpu_next_mesh_limits {
    ogpu_next_mesh_workgroup_limits task, mesh;
    uint32_t max_task_payload;
    uint32_t max_task_payload_shared;
    uint32_t max_mesh_payload_shared;
    uint32_t max_mesh_output_memory;
    uint32_t max_mesh_payload_output;
    uint32_t max_mesh_output_components;
    uint32_t max_mesh_output_vertices;
    uint32_t max_mesh_output_primitives;
    uint32_t max_mesh_output_layers;
    uint32_t max_mesh_multiview_views;
    uint32_t output_vertex_granularity;
    uint32_t output_primitive_granularity;
    uint32_t preferred_task_invocations;
    uint32_t preferred_mesh_invocations;
    /* Preferences, not constraints: local vertex/primitive output indexing,
     * compact vertex/primitive output respectively use bits 1,2,4,8. */
    uint32_t preference_flags, reserved;
} ogpu_next_mesh_limits;
/* Mesh graphics replaces SHADER_REQUIREMENTS with this exact record.
 * Metadata describes the specialized artifact: no runtime shader reflection.
 * task_local/shared/payload are zero without TASK; mesh_local is nonzero.
 * Output memory follows native padded output accounting, not packed host bytes.
 * output_layers is one plus the maximum emitted layer; output topology is in
 * the artifact. Caller proves metadata/payload producer-consumer agreement.
 * Multiview, primitive shading rate and mesh-specific queries are not enabled
 * by MESH/TASK. Their physical limits do not advertise those separate profiles. */
typedef struct ogpu_next_mesh_requirements {
    ogpu_next_record header;
    ogpu_next_features features;
    uint32_t task_local[3], task_shared, mesh_local[3], mesh_shared;
    uint32_t payload_size, output_memory, output_vertices, output_primitives;
    uint32_t output_components, output_layers;
} ogpu_next_mesh_requirements;
typedef struct ogpu_next_bytes { const void *data; size_t size; } ogpu_next_bytes;
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
typedef struct ogpu_next_argument_layout ogpu_next_argument_layout;
enum {
    OGPU_NEXT_ARGUMENT_INLINE = 0,
    OGPU_NEXT_ARGUMENT_UNIFORM_ADDRESS = 1,
    OGPU_NEXT_ARGUMENT_READ_ADDRESS = 2,
    OGPU_NEXT_ARGUMENT_READ_WRITE_ADDRESS = 3
};
enum {
    OGPU_NEXT_ARGUMENT_COMPUTE = 0,
    OGPU_NEXT_ARGUMENT_GRAPHICS = 1
};
/* INLINE may target a direct push-data declaration. Otherwise space/binding
 * identify a compiler-declared buffer resource, whose role is given by kind.
 * Both must be DIRECT together; DIRECT is legal only for INLINE.
 */
#define OGPU_NEXT_ARGUMENT_DIRECT UINT32_MAX
typedef struct ogpu_next_argument_field {
    uint32_t offset;
    uint32_t size;
    uint32_t kind;
    uint32_t address_alignment; /* INLINE: 0; address: requested power of two */
    ogpu_next_stages stages;    /* artifact visibility, not isolated storage */
    uint32_t space;
    uint32_t binding;
} ogpu_next_argument_field;

/* Fields are ordered, nonoverlapping, nonempty and within byte_size; padding is
 * allowed. Empty layouts have zero size/fields. Extents are four-byte aligned;
 * native addresses occupy eight bytes at eight-byte aligned offsets. Their
 * requested pointee alignment must satisfy the native binding. INLINE alignment
 * is zero. Field stages belong to the execution class; source resource/type/layout
 * agreement is the artifact producer's contract, without runtime reflection.
 * Creation copies field metadata and checks the complete native budget; it never
 * spills values. The device outlives the layout, which outlives all borrowing
 * executables and recorded/future/pending uses. Destruction never waits. */
typedef struct ogpu_next_argument_layout_desc {
    uint32_t execution_class;
    uint32_t byte_size;
    uint32_t field_count;
    uint32_t flags; /* zero */
    const ogpu_next_argument_field *fields;
} ogpu_next_argument_layout_desc;

enum {
    OGPU_NEXT_ARGUMENT_BIND_PRESERVES = 0,
    OGPU_NEXT_ARGUMENT_BIND_INVALIDATES_ON_CHANGE = 1
};
/* Read-only facts about command-local argument storage. These are not handles.
 * IDs compare only within one live device; zero is not a valid domain/key.
 * domain identifies potentially overlapping argument storage. Byte coordinates
 * are offset_base + logical offset. bind_rule applies to that domain.
 * Different layouts may have the same binding_key when natively compatible;
 * sharing the same layout always shares the key. The application must not
 * fabricate keys or compare them across devices/recreations.
 */
typedef struct ogpu_next_argument_state_info {
    uint64_t domain;
    uint64_t binding_key;
    uint32_t offset_base;
    uint32_t byte_size;
    uint32_t bind_rule;
    uint32_t reserved; /* zero */
} ogpu_next_argument_state_info;

ogpu_next_status ogpu_next_argument_layout_create(
    ogpu_next_device *, const ogpu_next_argument_layout_desc *,
    ogpu_next_argument_layout **);
void ogpu_next_argument_layout_destroy(ogpu_next_argument_layout *);
void ogpu_next_argument_layout_info(
    const ogpu_next_argument_layout *, ogpu_next_argument_state_info *);


typedef struct ogpu_next_shader {
    uint32_t stage, format;
    ogpu_next_bytes code;
    const char *entry;
    const ogpu_next_record *specialization;
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
 * and stage support. Query masks include every implemented shader stage.
 * QUAD outside compute/fragment requires quad_all_stages. No lane-to-invocation mapping is
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
 * The explicitly borrowed layout is mandatory, including empty layouts.
 * requirements names SHADER_REQUIREMENTS.
 * Shader semantics and metadata agreement are trusted: callers supply the actual
 * post-specialization local size and shared-memory use. This is not a SPIR-V
 * validator/reflection engine. Capabilities, memory safety, uniformity promises
 * and every reachable GPU access remain the artifact producer/caller's contract.
 * Buffer set/binding variables map through layout fields; direct heap access remains available.
 * Optional SPECIALIZATION entries have unique IDs, nonempty bounded byte spans,
 * and sizes/types matching the shader (including 4-byte native bools); reserved=0.
 * Code/metadata inputs are borrowed during creation. Layout and device remain
 * borrowed through destruction and recorded uses; no current context or implicit cache.
 * compile_flags=0 permits native compilation. COMPILE_INDIRECT_BINDABLE (below)
 * opts into native generated-command executable selection, requires enabled
 * GENERATED_COMMANDS and matching pipeline_binding_stages, and may be combined
 * with FAIL_IF_REQUIRED. It is never silently added to an ordinary executable.
 * FAIL_IF_REQUIRED requires enabled
 * CACHE_CONTROL and applies to compute and graphics, with or without a cache.
 * If native compilation is needed, returns COMPILE_REQUIRED and a NULL executable;
 * no automatic retry, background work, or deferred compilation is scheduled.
 * Caller may retry explicitly with flags=0. This is not an allocation-free or
 * bounded-latency guarantee, nor a cache-hit guarantee, even after an earlier
 * successful preparation or cache import. reserved=0; unknown flags are rejected.
 */
ogpu_next_status ogpu_next_executable_create(ogpu_next_device *, const ogpu_next_executable_desc *, ogpu_next_argument_layout *, ogpu_next_executable **);
void ogpu_next_executable_destroy(ogpu_next_executable *);
/* Binding borrows the executable and layout through all recorded/pending uses.
 * It does not restore arguments. Use argument_layout_info for native reuse rules.
 * Vulkan shares one preserving push-data domain across compute and graphics. */
void ogpu_next_bind_executable(ogpu_next_encoder *, ogpu_next_executable *);
/* The layout must belong to the executable
 * currently bound for its execution class. Record values now, pointees never.
 * offset/size are 4-aligned, in bounds; size=0 permits data=NULL.
 * Any intersected address field must be covered completely (8 bytes).
 * INLINE fields allow 4-byte subranges. Padding is uninterpreted.
 * The caller initializes every byte read by work; begin has undefined contents.
 * Pointee lifetime, bounds and hazards apply to every execution/replay. Null
 * addresses may be encoded but not dereferenced; native address fields enforce
 * their declared alignment. Inline physical-pointer bits remain caller-validated.
 * Full local preflight precedes native writes; invalid recording is poisoned.
 * No heap allocation, payload cache, resource retention or implicit update at
 * dispatch/draw. data need only be valid during this call.
 */
void ogpu_next_set_arguments(
    ogpu_next_encoder *, const ogpu_next_argument_layout *,
    uint32_t offset, uint32_t size, const void *data);

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


/* Optional native tessellation/geometry stages; see docs/raster-stages.md.
 * Enables are independent. *_MULTIVIEW also requires the matching base stage and
 * MULTIVIEW. TESSELLATION_GEOMETRY_POINT_SIZE permits explicit point-size writes
 * by tessellation/geometry; values other than one also require LARGE_POINTS. Caller
 * proves output-mode/interface requirements.
 * QUERY_RASTER_STAGE_LIMITS reports physical limits, not selected enables. */
#define OGPU_NEXT_FEATURE_TESSELLATION UINT32_C(69)
#define OGPU_NEXT_FEATURE_GEOMETRY UINT32_C(70)
#define OGPU_NEXT_FEATURE_TESSELLATION_MULTIVIEW UINT32_C(71)
#define OGPU_NEXT_FEATURE_GEOMETRY_MULTIVIEW UINT32_C(72)
#define OGPU_NEXT_FEATURE_TESSELLATION_GEOMETRY_POINT_SIZE UINT32_C(73)
#define OGPU_NEXT_STAGE_TESS_CONTROL UINT64_C(16777216)
#define OGPU_NEXT_STAGE_TESS_EVALUATION UINT64_C(33554432)
#define OGPU_NEXT_STAGE_GEOMETRY UINT64_C(67108864)
#define OGPU_NEXT_QUERY_RASTER_STAGE_LIMITS UINT32_C(23)
#define OGPU_NEXT_TESSELLATION_STATE UINT32_C(133)
#define OGPU_NEXT_TRIANGLE_FAN UINT32_C(5)
#define OGPU_NEXT_LINES_ADJACENCY UINT32_C(6)
#define OGPU_NEXT_LINE_STRIP_ADJACENCY UINT32_C(7)
#define OGPU_NEXT_TRIANGLES_ADJACENCY UINT32_C(8)
#define OGPU_NEXT_TRIANGLE_STRIP_ADJACENCY UINT32_C(9)
#define OGPU_NEXT_PATCHES UINT32_C(10)
#define OGPU_NEXT_DOMAIN_UPPER_LEFT UINT32_C(0)
#define OGPU_NEXT_DOMAIN_LOWER_LEFT UINT32_C(1)
typedef struct ogpu_next_raster_stage_limits {
    uint32_t max_generation_level;
    uint32_t max_patch_size;
    uint32_t control_input_components;
    uint32_t control_output_components;
    uint32_t patch_output_components;
    uint32_t control_total_output_components;
    uint32_t evaluation_input_components;
    uint32_t evaluation_output_components;
    uint32_t geometry_invocations;
    uint32_t geometry_input_components;
    uint32_t geometry_output_components;
    uint32_t geometry_output_vertices;
    uint32_t geometry_total_output_components;
} ogpu_next_raster_stage_limits;
/* Required only for paired TESS_CONTROL/TESS_EVALUATION with PATCHES topology.
 * patch_control_points is input patch size, 1..max_patch_size, not shader output
 * patch size. domain_origin is UPPER_LEFT/LOWER_LEFT. Other tessellation modes,
 * output counts, interfaces and geometry input/output modes are in the artifact;
 * caller/compiler proves limits and inter-stage/topology compatibility.
 * Chain on graphics_state.header.next, at most once, optionally with one
 * RENDER_MAPPING in either order. Unknown/duplicate/cyclic chains reject.
 * No dynamic patch-size policy, CPU geometry expansion or per-draw stage scan. */
typedef struct ogpu_next_tessellation_state {
    ogpu_next_record header;
    uint32_t patch_control_points, domain_origin;
} ogpu_next_tessellation_state;

/* Optional native dynamic-rendering local reads (LOCAL_READ enable).
 * LIMITS returns immutable physical support, not selected device enables.
 * max_inputs bounds input indices; depth_stencil/multisample are independent
 * Boolean support for those reads. Shader/attachment compatibility is caller proof.
 * LOCAL_READ is an explicit layout, not automatic dependency or feedback ordering.
 * See docs/local-read.md for native mapping and remaining acceptance. */
#define OGPU_NEXT_QUERY_LOCAL_READ_LIMITS 22u
#define OGPU_NEXT_RENDER_MAPPING 132u
#define OGPU_NEXT_STATE_LOCAL_READ 10u
#define OGPU_NEXT_ACCESS_INPUT_ATTACHMENT_READ UINT64_C(16777216)
#define OGPU_NEXT_DEPENDENCY_BY_REGION 1u
#define OGPU_NEXT_DEPENDENCY_VIEW_LOCAL 2u
#define OGPU_NEXT_ATTACHMENT_UNUSED UINT32_MAX
typedef struct ogpu_next_local_read_limits {
    uint32_t max_inputs, depth_stencil, multisample, reserved;
} ogpu_next_local_read_limits;
typedef struct ogpu_next_render_locations {
    uint32_t color_count, reserved;
    const uint32_t *locations; /* attachment slot -> output location; NULL = identity */
} ogpu_next_render_locations;
typedef struct ogpu_next_render_inputs {
    uint32_t color_count, reserved;
    /* Color slots -> shader input indices; NULL colors = identity.
     * NULL depth/stencil means an input WITHOUT InputAttachmentIndex decoration;
     * a pointer to ATTACHMENT_UNUSED disables that aspect. Depth/stencil may share
     * an index; colors must be unique and disjoint from them. */
    const uint32_t *colors, *depth, *stencil;
} ogpu_next_render_inputs;
/* Fragment shader input attachment at compiler space/binding. root_offset is a
 * four-aligned byte offset in the existing argument layout, containing a uint32
 * resource-heap location. array_stride is the location-unit step between elements
 * (zero is valid). Vulkan uses byte locations; stride must be image-descriptor
 * aligned. Caller proves actual root value/array bounds, descriptor kind/layout,
 * matching attachment/view and shader interface. No runtime descriptor synthesis.
 * Compiler resource classification may include or omit NonWritable. */
typedef struct ogpu_next_input_attachment_binding {
    uint32_t space, binding, root_offset, array_stride;
} ogpu_next_input_attachment_binding;
/* On graphics_state.header.next, optionally chained with TESSELLATION_STATE;
 * one of each, in either order. On render_inheritance this is the sole extension
 * and its own header.next must be NULL. locations/inputs may be NULL for defaults.
 * Bindings are preparation-only on graphics_state; inheritance binding_count=0.
 * Nonzero binding_count requires bindings; space/binding pairs must be unique.
 * Arrays/records borrow caller storage only through creation/recording.
 * All reserved fields are zero; non-UNUSED color locations/indices are unique. */
typedef struct ogpu_next_render_mapping {
    ogpu_next_record header;
    const ogpu_next_render_locations *locations;
    const ogpu_next_render_inputs *inputs;
    uint32_t binding_count, reserved;
    const ogpu_next_input_attachment_binding *bindings;
} ogpu_next_render_mapping;
/* Inside rendering only, including render-continuation secondary lists; primary
 * SECONDARY contents cannot use these setters. color_count must match the current
 * scope. Pipeline, current mapping, secondary inheritance and shader indices must
 * agree at use; caller proves this. No per-draw comparison or retained arrays. */
void ogpu_next_set_render_locations(ogpu_next_encoder *, const ogpu_next_render_locations *);
void ogpu_next_set_render_inputs(ogpu_next_encoder *, const ogpu_next_render_inputs *);

/* Independent PIPELINE/PRIMITIVE/ATTACHMENT_SHADING_RATE feature enables; none
 * implies another. MESH_PRIMITIVE_SHADING_RATE also requires MESH_SHADER and
 * PRIMITIVE_SHADING_RATE. Physical limits/tuples are identical on adapter/device;
 * zero limits/empty tuples if the native extension is absent. Samples is a bitmask.
 * Interaction properties and tuples describe native final-rate reduction, not
 * additional hard validity restrictions or a promise of the exact requested rate.
 * Vulkan maps this profile to VK_KHR_fragment_shading_rate, not density maps. */
#define OGPU_NEXT_FORMAT_SHADING_RATE UINT64_C(8192)
#define OGPU_NEXT_QUERY_SHADING_RATE_LIMITS 20u
#define OGPU_NEXT_QUERY_SHADING_RATES 21u
#define OGPU_NEXT_DYNAMIC_SHADING_RATE UINT64_C(8192)
#define OGPU_NEXT_IMAGE_SHADING_RATE UINT64_C(256)
#define OGPU_NEXT_STATE_SHADING_RATE 9u
#define OGPU_NEXT_STAGE_SHADING_RATE UINT64_C(8388608)
#define OGPU_NEXT_ACCESS_SHADING_RATE_READ UINT64_C(8388608)
#define OGPU_NEXT_COMPILE_SHADING_RATE_ATTACHMENT 4u
enum { OGPU_NEXT_SHADING_RATE_KEEP = 0, OGPU_NEXT_SHADING_RATE_REPLACE = 1,
       OGPU_NEXT_SHADING_RATE_MIN = 2, OGPU_NEXT_SHADING_RATE_MAX = 3, OGPU_NEXT_SHADING_RATE_MUL = 4 };
/* Fragment dimensions are 1, 2 or 4; other values INVALID. Without PIPELINE
 * enable they must be 1x1. Combine pipeline with primitive using combiners[0],
 * then combine that result with attachment using combiners[1]. KEEP keeps the
 * first input, REPLACE chooses the second. Disabled primitive/attachment inputs
 * require KEEP in their respective slot. MIN/MAX/MUL require nontrivial_combiners.
 * Static state is borrowed only through executable creation; NULL means
 * {{1,1},{KEEP,KEEP}}. DYNAMIC_SHADING_RATE ignores the static pointer and requires
 * the caller to set dynamic state before use, respecting native invalidation. */
typedef struct ogpu_next_shading_rate_state {
    uint32_t fragment_size[2], combiners[2];
} ogpu_next_shading_rate_state;
/* Render pointer NULL means absent. Otherwise view is non-NULL, bound, same-device,
 * 2D/2D_ARRAY with IMAGE_SHADING_RATE usage and identity component mapping; image
 * samples=1, state=GENERAL or SHADING_RATE, reserved=0. texel_size is a power of
 * two within queried min/max/aspect limits. The selected base mip covers the
 * entire offset render area after scaling by texel_size. One layer broadcasts;
 * otherwise enough layers for render layers/highest view-mask bit are required.
 * Multiple layers require layered_attachments. The view cannot equal an ordinary
 * attachment or resolve view in this render scope. Caller owns backing, contents,
 * barriers, layout and retirement. There is no implicit generation or transition.
 * Graphics executables used with this attachment must explicitly request
 * COMPILE_SHADING_RATE_ATTACHMENT (requires ATTACHMENT enable; invalid for compute).
 * Vulkan texels use the native unsigned shading-rate encoding; query format usage.
 * Rate state/attachment record are borrowed through the call, resource through use. */
typedef struct ogpu_next_shading_rate_attachment {
    ogpu_next_view *view;
    uint32_t state, texel_size[2], reserved;
} ogpu_next_shading_rate_attachment;
typedef struct ogpu_next_shading_rate {
    uint32_t fragment_size[2], samples;
} ogpu_next_shading_rate;
typedef struct ogpu_next_shading_rate_limits {
    uint32_t min_attachment_texel[2], max_attachment_texel[2];
    uint32_t max_attachment_texel_aspect_ratio, primitive_with_multiple_viewports;
    uint32_t layered_attachments, nontrivial_combiners, max_fragment_size[2];
    uint32_t max_fragment_aspect_ratio, max_coverage_samples, max_rasterization_samples;
    uint32_t with_shader_depth_stencil_writes, with_sample_mask, with_shader_sample_mask;
    uint32_t with_conservative_rasterization, with_fragment_shader_interlock;
    uint32_t with_custom_sample_locations, strict_multiply_combiner;
} ogpu_next_shading_rate_limits;
/* Borrow state through this recording call. Requires a graphics queue and at
 * least one enabled shading-rate input. One native state command, no allocation,
 * compilation or cache. Errors poison recording via the usual encoder contract. */
void ogpu_next_set_shading_rate(ogpu_next_encoder *, const ogpu_next_shading_rate_state *);

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
    /* One greater than the highest supported view index (<=32), not popcount. */
    uint32_t max_multiview_views, max_multiview_instance_index;
} ogpu_next_graphics_limits;
typedef struct ogpu_next_query_limits {
    float timestamp_period_ns;
    uint32_t timestamp_compute_graphics;
    uint64_t statistics;
} ogpu_next_query_limits;
enum {
    OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR = 1, OGPU_NEXT_DYNAMIC_BLEND_CONSTANTS = 2,
    OGPU_NEXT_DYNAMIC_STENCIL_REFERENCE = 4, OGPU_NEXT_DYNAMIC_STENCIL_COMPARE_MASK = 8,
    OGPU_NEXT_DYNAMIC_STENCIL_WRITE_MASK = 16,
    OGPU_NEXT_DYNAMIC_DEPTH_TEST = 32,
    OGPU_NEXT_DYNAMIC_DEPTH_WRITE = 64,
    OGPU_NEXT_DYNAMIC_DEPTH_COMPARE = 128,
    OGPU_NEXT_DYNAMIC_STENCIL_TEST = 256,
    OGPU_NEXT_DYNAMIC_STENCIL_OPS = 512,
    OGPU_NEXT_DYNAMIC_CULL = 1024,
    OGPU_NEXT_DYNAMIC_FRONT_FACE = 2048,
    OGPU_NEXT_DYNAMIC_VERTEX_STRIDE = 4096
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
/* Booleans are exactly 0/1; cull=NONE/FRONT/BACK/BOTH, front_face=CCW/CW.
 * Stencil operations and compare use the existing STENCIL_* and COMPARE_* enums.
 * Each setter selects only that independent state; no coupled reset is implied.
 * Dynamic stencil ops leave reference/compare-mask/write-mask unchanged. */
void ogpu_next_set_depth_test(ogpu_next_encoder *, uint32_t enabled);
void ogpu_next_set_depth_write(ogpu_next_encoder *, uint32_t enabled);
void ogpu_next_set_depth_compare(ogpu_next_encoder *, uint32_t compare);
void ogpu_next_set_stencil_test(ogpu_next_encoder *, uint32_t enabled);
void ogpu_next_set_stencil_ops(ogpu_next_encoder *, uint32_t faces, uint32_t fail,
    uint32_t pass, uint32_t depth_fail, uint32_t compare);
void ogpu_next_set_cull(ogpu_next_encoder *, uint32_t cull);
void ogpu_next_set_front_face(ogpu_next_encoder *, uint32_t front_face);
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
    uint32_t view_mask; /* zero: ordinary layered rendering; otherwise MULTIVIEW */
    const ogpu_next_shading_rate_state *shading_rate;
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
#define OGPU_NEXT_DYNAMIC_CONSERVATIVE_MODE (UINT64_C(1) << 14)
#define OGPU_NEXT_DYNAMIC_EXTRA_OVERESTIMATION (UINT64_C(1) << 15)
#define OGPU_NEXT_DYNAMIC_PROVOKING_VERTEX (UINT64_C(1) << 16)
#define OGPU_NEXT_DYNAMIC_DEPTH_CLIP (UINT64_C(1) << 17)
#define OGPU_NEXT_DYNAMIC_DEPTH_CONVENTION (UINT64_C(1) << 18)
#define OGPU_NEXT_DYNAMIC_LINE_MODE (UINT64_C(1) << 19)
#define OGPU_NEXT_DYNAMIC_LINE_STIPPLE_ENABLE (UINT64_C(1) << 20)
#define OGPU_NEXT_DYNAMIC_LINE_STIPPLE (UINT64_C(1) << 21)

/* Core dynamic raster state. No new optional enable: existing DEPTH_BOUNDS,
 * WIDE_LINES, DEPTH_BIAS_CLAMP, list/patch restart and stage capabilities apply
 * to the values/draws that use them. Selected static values are ignored, except
 * topology remains a class selector when unrestricted_topology is zero.
 * No current-pipeline tracking: caller establishes each dynamic value before
 * draws and proves topology/stage/restart compatibility, including generated work.
 */
/* Native viewport/scissor arrays. MULTI_VIEWPORT enables counts/indices beyond
 * slot zero; SHADER_OUTPUT_VIEWPORT_INDEX independently enables shader output
 * in stages governed by that feature. Stage-specific artifact requirements apply.
 * Count state defaults to one of each; chain VIEWPORT_COUNTS to GRAPHICS_STATE.
 * Dynamic count flags ignore the corresponding static count and use SET_COUNT.
 * Without SET_COUNT, update [first,first+count) of fixed-count dynamic state.
 * With SET_COUNT, first must be zero and count nonzero; replaces count + values.
 * Count/range must fit VIEWPORT_LIMITS and requested MULTI_VIEWPORT capability.
 * Zero-count range updates are no-ops and allow null. Arrays are borrowed directly.
 * Every draw must have matching effective viewport/scissor counts and all used
 * slots set, respecting native invalidation, secondary and shader-index rules.
 * No active-count/slot tracking. set_graphics_state remains a slot-zero shorthand
 * for fixed-count viewport and scissor state, not a count-setting operation.
 */
/* Extended depth and independent color-write profiles. Static clamp and bias
 * representation records extend RASTER_OPTIONS; COLOR_WRITE_STATE extends
 * GRAPHICS_STATE. Bias representation is ignored with DYNAMIC_DEPTH_BIAS.
 * Dynamic clamp/color state ignores static values (reserved must remain zero).
 * Base enables are explicit; no shader emulation or state/pipeline tracking.
 */
#define OGPU_NEXT_FEATURE_UNRESTRICTED_DEPTH_RANGE UINT32_C(112)
#define OGPU_NEXT_FEATURE_DEPTH_CLAMP_CONTROL UINT32_C(113)
#define OGPU_NEXT_FEATURE_DEPTH_BIAS_CONTROL UINT32_C(114)
#define OGPU_NEXT_FEATURE_DEPTH_BIAS_UNORM UINT32_C(115)
#define OGPU_NEXT_FEATURE_DEPTH_BIAS_FLOAT UINT32_C(116)
#define OGPU_NEXT_FEATURE_DEPTH_BIAS_EXACT UINT32_C(117)
#define OGPU_NEXT_FEATURE_COLOR_WRITE_ENABLE UINT32_C(118)

#define OGPU_NEXT_DEPTH_CLAMP_CONTROL_STATE UINT32_C(143)
#define OGPU_NEXT_DEPTH_BIAS_REPRESENTATION UINT32_C(144)
#define OGPU_NEXT_COLOR_WRITE_STATE UINT32_C(145)
#define OGPU_NEXT_DYNAMIC_DEPTH_CLAMP_RANGE (UINT64_C(1) << 46)
#define OGPU_NEXT_DYNAMIC_COLOR_WRITE_ENABLE (UINT64_C(1) << 47)
enum { OGPU_NEXT_CLAMP_VIEWPORT_RANGE=0, OGPU_NEXT_CLAMP_USER_RANGE=1 };
enum { OGPU_NEXT_BIAS_FORMAT=0, OGPU_NEXT_BIAS_UNORM=1, OGPU_NEXT_BIAS_FLOAT=2 };
typedef struct ogpu_next_depth_clamp_control { ogpu_next_record header; uint32_t mode,reserved; float min,max; } ogpu_next_depth_clamp_control;
typedef struct ogpu_next_depth_bias_representation { ogpu_next_record header; uint32_t representation,exact; } ogpu_next_depth_bias_representation;
typedef struct ogpu_next_color_write_state { ogpu_next_record header; uint32_t count,reserved; const uint32_t *enables; } ogpu_next_color_write_state;
void ogpu_next_set_depth_clamp_range(ogpu_next_encoder *,uint32_t mode,float min,float max);
void ogpu_next_set_depth_bias_control(ogpu_next_encoder *,float constant,float clamp,float slope,uint32_t representation,uint32_t exact);
void ogpu_next_set_color_write_enable(ogpu_next_encoder *,uint32_t count,const uint32_t *enables);
/* Clamp viewport mode ignores min/max; user range is ordered and finite, within
 * [0,1] unless UNRESTRICTED_DEPTH_RANGE is requested. That capability also permits
 * finite viewport/depth-bounds values outside [0,1]. Bias representations retain
 * native numerical units; UNORM/FLOAT/EXACT independently require their features
 * plus DEPTH_BIAS_CONTROL. set_depth_bias remains the default representation path.
 * Color-write enable controls whole attachments independently of component masks.
 * Array values are 0/1, count <= max_colors; zero count is a no-op. Static count
 * equals pipeline color_count. Dynamic values must cover every consumed target.
 */

#define OGPU_NEXT_FEATURE_MULTI_VIEWPORT UINT32_C(110)
#define OGPU_NEXT_FEATURE_SHADER_OUTPUT_VIEWPORT_INDEX UINT32_C(111)
#define OGPU_NEXT_QUERY_VIEWPORT_LIMITS UINT32_C(28)
#define OGPU_NEXT_VIEWPORT_COUNTS UINT32_C(142)
#define OGPU_NEXT_SET_COUNT UINT32_C(1)
#define OGPU_NEXT_DYNAMIC_VIEWPORT_COUNT (UINT64_C(1) << 44)
#define OGPU_NEXT_DYNAMIC_SCISSOR_COUNT (UINT64_C(1) << 45)
typedef struct ogpu_next_viewport_limits { uint32_t max_viewports, subpixel_bits; } ogpu_next_viewport_limits;
typedef struct ogpu_next_viewport_counts { ogpu_next_record header; uint32_t viewports, scissors; } ogpu_next_viewport_counts;
typedef struct ogpu_next_viewport { float x,y,width,height,min_depth,max_depth; } ogpu_next_viewport;
typedef struct ogpu_next_scissor { int32_t x,y; uint32_t width,height; } ogpu_next_scissor;
void ogpu_next_set_viewports(ogpu_next_encoder *,uint32_t first,uint32_t count,const ogpu_next_viewport *,uint32_t flags);
void ogpu_next_set_scissors(ogpu_next_encoder *,uint32_t first,uint32_t count,const ogpu_next_scissor *,uint32_t flags);

#define OGPU_NEXT_QUERY_DYNAMIC_RASTER_LIMITS UINT32_C(27)
typedef struct ogpu_next_dynamic_raster_limits {
    uint32_t unrestricted_topology;
} ogpu_next_dynamic_raster_limits;
#define OGPU_NEXT_DYNAMIC_TOPOLOGY (UINT64_C(1) << 25)
#define OGPU_NEXT_DYNAMIC_PRIMITIVE_RESTART (UINT64_C(1) << 26)
#define OGPU_NEXT_DYNAMIC_RASTERIZER_DISCARD (UINT64_C(1) << 27)
#define OGPU_NEXT_DYNAMIC_LINE_WIDTH (UINT64_C(1) << 28)
#define OGPU_NEXT_DYNAMIC_DEPTH_BIAS (UINT64_C(1) << 29)
#define OGPU_NEXT_DYNAMIC_DEPTH_BIAS_ENABLE (UINT64_C(1) << 30)
#define OGPU_NEXT_DYNAMIC_DEPTH_BOUNDS (UINT64_C(1) << 31)
#define OGPU_NEXT_DYNAMIC_DEPTH_BOUNDS_ENABLE (UINT64_C(1) << 32)
void ogpu_next_set_topology(ogpu_next_encoder *, uint32_t topology);
void ogpu_next_set_primitive_restart(ogpu_next_encoder *, uint32_t enable);
void ogpu_next_set_rasterizer_discard(ogpu_next_encoder *, uint32_t enable);
void ogpu_next_set_line_width(ogpu_next_encoder *, float width);
void ogpu_next_set_depth_bias(ogpu_next_encoder *, float constant, float clamp, float slope);
void ogpu_next_set_depth_bias_enable(ogpu_next_encoder *, uint32_t enable);
void ogpu_next_set_depth_bounds(ogpu_next_encoder *, float min, float max);
void ogpu_next_set_depth_bounds_enable(ogpu_next_encoder *, uint32_t enable);

/* Bias constants/slope/clamp are finite; nonzero clamp needs DEPTH_BIAS_CLAMP.
 * Bounds are ordered and finite; [0,1] unless UNRESTRICTED_DEPTH_RANGE is requested.
 * Enabling their test needs DEPTH_BOUNDS.
 * Width is finite/positive; widths other than one need WIDE_LINES. Values pass
 * through unchanged; native supported-width selection/rounding applies, including
 * values outside the queried range. Dynamic discard can toggle on a pipeline with fragment/output state;
 * attachments/clears still execute. Restart enabling does not infer a topology.
 */

/* Optional dynamic raster controls. Each FEATURE_DYNAMIC_* is independently
 * requested; each DYNAMIC_* ignores its corresponding static field. Existing
 * value capabilities (DEPTH_CLAMP, NON_SOLID_FILL, ALPHA_TO_ONE, LOGIC_OP,
 * DUAL_SOURCE_BLEND) are not enabled implicitly. See docs/extended-dynamic-raster.md.
 * Setter arrays are borrowed for the call. Zero count is a no-op after capability
 * and attachment-range checks. No scratch or retained host pointers.
 */
#define OGPU_NEXT_FEATURE_DYNAMIC_DEPTH_CLAMP UINT32_C(99)
#define OGPU_NEXT_DYNAMIC_DEPTH_CLAMP (UINT64_C(1) << 33)
#define OGPU_NEXT_FEATURE_DYNAMIC_POLYGON_MODE UINT32_C(100)
#define OGPU_NEXT_DYNAMIC_POLYGON_MODE (UINT64_C(1) << 34)
#define OGPU_NEXT_FEATURE_DYNAMIC_RASTERIZATION_SAMPLES UINT32_C(101)
#define OGPU_NEXT_DYNAMIC_RASTERIZATION_SAMPLES (UINT64_C(1) << 35)
#define OGPU_NEXT_FEATURE_DYNAMIC_SAMPLE_MASK UINT32_C(102)
#define OGPU_NEXT_DYNAMIC_SAMPLE_MASK (UINT64_C(1) << 36)
#define OGPU_NEXT_FEATURE_DYNAMIC_ALPHA_TO_COVERAGE UINT32_C(103)
#define OGPU_NEXT_DYNAMIC_ALPHA_TO_COVERAGE (UINT64_C(1) << 37)
#define OGPU_NEXT_FEATURE_DYNAMIC_ALPHA_TO_ONE UINT32_C(104)
#define OGPU_NEXT_DYNAMIC_ALPHA_TO_ONE (UINT64_C(1) << 38)
#define OGPU_NEXT_FEATURE_DYNAMIC_LOGIC_OP_ENABLE UINT32_C(105)
#define OGPU_NEXT_DYNAMIC_LOGIC_OP_ENABLE (UINT64_C(1) << 39)
#define OGPU_NEXT_FEATURE_DYNAMIC_COLOR_BLEND_ENABLE UINT32_C(106)
#define OGPU_NEXT_DYNAMIC_COLOR_BLEND_ENABLE (UINT64_C(1) << 40)
#define OGPU_NEXT_FEATURE_DYNAMIC_COLOR_BLEND_EQUATION UINT32_C(107)
#define OGPU_NEXT_DYNAMIC_COLOR_BLEND_EQUATION (UINT64_C(1) << 41)
#define OGPU_NEXT_FEATURE_DYNAMIC_COLOR_WRITE_MASK UINT32_C(108)
#define OGPU_NEXT_DYNAMIC_COLOR_WRITE_MASK (UINT64_C(1) << 42)
#define OGPU_NEXT_FEATURE_DYNAMIC_LOGIC_OP UINT32_C(109)
#define OGPU_NEXT_DYNAMIC_LOGIC_OP (UINT64_C(1) << 43)
typedef struct ogpu_next_blend_equation {
    uint32_t src_color, dst_color, color_op, src_alpha, dst_alpha, alpha_op;
} ogpu_next_blend_equation;
void ogpu_next_set_depth_clamp(ogpu_next_encoder *, uint32_t value);
void ogpu_next_set_polygon_mode(ogpu_next_encoder *, uint32_t value);
void ogpu_next_set_rasterization_samples(ogpu_next_encoder *, uint32_t value);
void ogpu_next_set_sample_mask(ogpu_next_encoder *, uint32_t samples, uint64_t mask);
void ogpu_next_set_alpha_to_coverage(ogpu_next_encoder *, uint32_t value);
void ogpu_next_set_alpha_to_one(ogpu_next_encoder *, uint32_t value);
void ogpu_next_set_logic_op_enable(ogpu_next_encoder *, uint32_t value);
void ogpu_next_set_color_blend_enable(ogpu_next_encoder *, uint32_t first, uint32_t count, const uint32_t *values);
void ogpu_next_set_color_blend_equation(ogpu_next_encoder *, uint32_t first, uint32_t count, const ogpu_next_blend_equation *values);
void ogpu_next_set_color_write_mask(ogpu_next_encoder *, uint32_t first, uint32_t count, const uint32_t *values);
void ogpu_next_set_logic_op(ogpu_next_encoder *, uint32_t value);
/* Sample counts are powers of two in [1,64]. A dynamic mask supplies bits for
 * at least the draw sample count; bits above its declared count are ignored.
 * Dynamic sample count ignores GraphicsState.samples; a static sample_mask still
 * supplies all 64 bits. Caller proves attachment/format, sample-shading, custom
 * sample-position and shader compatibility at every draw. Dynamic per-attachment
 * state must cover every consumed slot. Blending needs blendable formats; dual
 * source factors need matching shader output and attachment limits. Native state
 * invalidation and secondary/generated-command obligations remain caller-owned.
 */

#define OGPU_NEXT_FEATURE_DYNAMIC_VERTEX_INPUT UINT32_C(98)
#define OGPU_NEXT_DYNAMIC_VERTEX_INPUT (UINT64_C(1) << 24)
#define OGPU_NEXT_FEATURE_SAMPLE_LOCATIONS UINT32_C(96)
#define OGPU_NEXT_FEATURE_DYNAMIC_SAMPLE_LOCATIONS_ENABLE UINT32_C(97)
#define OGPU_NEXT_QUERY_SAMPLE_LOCATION_LIMITS UINT32_C(26)
#define OGPU_NEXT_SAMPLE_LOCATIONS UINT32_C(140)
#define OGPU_NEXT_SAMPLE_LOCATION_STATE UINT32_C(141)
#define OGPU_NEXT_IMAGE_SAMPLE_LOCATIONS_DEPTH UINT32_C(8)
#define OGPU_NEXT_DYNAMIC_SAMPLE_LOCATIONS (UINT64_C(1) << 22)
#define OGPU_NEXT_DYNAMIC_SAMPLE_LOCATIONS_ENABLE (UINT64_C(1) << 23)
typedef struct ogpu_next_sample_location_limits {
    uint32_t samples, max_grid[2];
    float coordinate_range[2];
    uint32_t subpixel_bits, variable, grids[7][2];
} ogpu_next_sample_location_limits;
typedef struct ogpu_next_sample_location { float x, y; } ogpu_next_sample_location;
typedef struct ogpu_next_sample_locations {
    ogpu_next_record header;
    uint32_t samples, grid[2], count;
    const ogpu_next_sample_location *locations;
} ogpu_next_sample_locations;
typedef struct ogpu_next_sample_location_state {
    ogpu_next_record header;
    uint32_t enable, reserved;
    const ogpu_next_sample_locations *locations;
} ogpu_next_sample_location_state;
/* SAMPLE_LOCATION_STATE extends raster_options.header.next. Its pattern and the
 * SAMPLE_LOCATIONS record on image_barrier.next require next=NULL. Pattern count
 * equals samples*grid[0]*grid[1]; grid dimensions divide grids[log2(samples)].
 * samples is a supported power of two <=64. Coordinates are finite, natively
 * clamped/quantized; ordering is ((y*grid.width+x)*samples+sample).
 * Dynamic pattern/enable ignore only their selected static fields. Enable is
 * boolean, reserved=0. Initialize selected dynamic state before use.
 * Depth attachments used with custom positions need IMAGE_SAMPLE_LOCATIONS_DEPTH.
 * Preserving their depth across layout transitions needs the last-written pattern
 * on the barrier; NULL deliberately leaves transition depth contents undefined.
 * Changing that pattern requires clearing depth before reusing it. Matching the
 * historical pattern, draw samples, pass-wide consistency when variable=0,
 * secondary/continuation rules and shader/shading-rate compatibility are TRUSTED
 * caller obligations; OGPU stores no last-pattern/pipeline cache. Arrays borrow
 * only through preparation/recording. See docs/sample-locations.md.
 */
void ogpu_next_set_sample_locations(ogpu_next_encoder *, const ogpu_next_sample_locations *);
void ogpu_next_set_sample_locations_enable(ogpu_next_encoder *, uint32_t enable);

#define OGPU_NEXT_QUERY_RASTER_MECHANISM_LIMITS UINT32_C(25)
#define OGPU_NEXT_CONSERVATIVE_DISABLED UINT32_C(0)
#define OGPU_NEXT_CONSERVATIVE_OVERESTIMATE UINT32_C(1)
#define OGPU_NEXT_CONSERVATIVE_UNDERESTIMATE UINT32_C(2)
#define OGPU_NEXT_LINE_DEFAULT UINT32_C(0)
#define OGPU_NEXT_LINE_RECTANGULAR UINT32_C(1)
#define OGPU_NEXT_LINE_BRESENHAM UINT32_C(2)
#define OGPU_NEXT_LINE_SMOOTH_RECTANGULAR UINT32_C(3)
#define OGPU_NEXT_PROVOKING_VERTEX_FIRST UINT32_C(0)
#define OGPU_NEXT_PROVOKING_VERTEX_LAST UINT32_C(1)
#define OGPU_NEXT_CONSERVATIVE_STATE UINT32_C(136)
#define OGPU_NEXT_LINE_STATE UINT32_C(137)
#define OGPU_NEXT_PROVOKING_STATE UINT32_C(138)
#define OGPU_NEXT_DEPTH_CONVENTION_STATE UINT32_C(139)
typedef struct ogpu_next_raster_mechanism_limits {
    float primitive_overestimation, max_extra_overestimation, extra_overestimation_granularity;
    uint32_t underestimation, conservative_points_lines, degenerate_triangles, degenerate_lines;
    uint32_t fully_covered_input, conservative_post_depth_coverage, provoking_per_pipeline;
    uint32_t line_subpixel_precision, strict_lines;
} ogpu_next_raster_mechanism_limits;
typedef struct ogpu_next_conservative_state { ogpu_next_record header; uint32_t mode; float extra_overestimation; } ogpu_next_conservative_state;
typedef struct ogpu_next_line_state { ogpu_next_record header; uint32_t mode, stipple_enable, factor, pattern; } ogpu_next_line_state;
typedef struct ogpu_next_provoking_state { ogpu_next_record header; uint32_t mode, reserved; } ogpu_next_provoking_state;
typedef struct ogpu_next_depth_convention_state { ogpu_next_record header; uint32_t negative_one_to_one, reserved; } ogpu_next_depth_convention_state;
/* Optional raster_options.header.next chain: one of each of these records, SAMPLE_LOCATION_STATE and
 * DEPTH_CLIP_STATE, in any order. Unknown/duplicate/cyclic records reject. Reserved
 * fields=0. Conservative mode=0 disabled, 1 overestimate, 2 underestimate; mode 2
 * needs the queried property. Extra size is finite, nonnegative and <= maximum.
 * Line mode=0 default, 1 rectangular, 2 Bresenham, 3 smooth rectangular. Each
 * nondefault mode and its stipple feature are independently enabled. Stipple is
 * boolean; factor=1..256, pattern=0..65535. Default+stipple needs strict_lines.
 * Provoking mode=0 first, 1 last. Depth convention=0 [0,w], 1 [-w,w] Z clipping.
 * Omission preserves native defaults. Explicit conservative/provoking/convention
 * records need their base feature, even when selecting default values.
 * Dynamic fields ignore static values; initialize every selected dynamic value.
 * Native primitive/output-mode, sample-shading/alpha and pass-wide provoking
 * compatibility are TRUSTED caller rules; no current-pipeline/state cache.
 */
void ogpu_next_set_conservative_mode(ogpu_next_encoder *, uint32_t mode);
void ogpu_next_set_extra_overestimation(ogpu_next_encoder *, float extra);
void ogpu_next_set_provoking_vertex(ogpu_next_encoder *, uint32_t mode);
void ogpu_next_set_depth_clip(ogpu_next_encoder *, uint32_t enable);
void ogpu_next_set_depth_convention(ogpu_next_encoder *, uint32_t negative_one_to_one);
void ogpu_next_set_line_mode(ogpu_next_encoder *, uint32_t mode);
void ogpu_next_set_line_stipple_enable(ogpu_next_encoder *, uint32_t enable);
void ogpu_next_set_line_stipple(ogpu_next_encoder *, uint32_t factor, uint32_t pattern);
#define OGPU_NEXT_QUERY_RASTER_CONTROL_LIMITS UINT32_C(24)
#define OGPU_NEXT_DEPTH_CLIP_STATE UINT32_C(134)
#define OGPU_NEXT_VERTEX_DIVISORS UINT32_C(135)
typedef struct ogpu_next_raster_control_limits {
    float point_size_range[2], point_size_granularity;
    uint32_t max_vertex_divisor, nonzero_first_instance, polygon_mode_point_size;
} ogpu_next_raster_control_limits;
typedef struct ogpu_next_depth_clip_state {
    ogpu_next_record header;
    uint32_t enable, reserved;
} ogpu_next_depth_clip_state;
/* Optional member of raster_options.header.next; reserved=0, enable=0/1.
 * Requires DEPTH_CLIP even when enable=0. Controls Z clipping independently of
 * depth_clamp; absence preserves the native coupled clamp/clip default. */
typedef struct ogpu_next_vertex_divisor { uint32_t binding, divisor; } ogpu_next_vertex_divisor;
typedef struct ogpu_next_vertex_divisors {
    ogpu_next_record header;
    uint32_t count, reserved;
    const ogpu_next_vertex_divisor *entries;
} ogpu_next_vertex_divisors;
/* Optional vertex_input.header.next; next=NULL, reserved=0. Unique entries name
 * declared INSTANCE-rate bindings. Omitted bindings use divisor one. Divisor>1
 * needs VERTEX_DIVISOR; zero needs VERTEX_ZERO_DIVISOR independently. Each divisor
 * must fit max_vertex_divisor. Native fetch index is firstInstance + floor(local
 * instance/divisor); zero fetches firstInstance for every instance. If the physical
 * nonzero_first_instance property is zero, draws with any divisor!=1 must use
 * firstInstance=0, including indirect/generated records: a TRUSTED caller rule.
 * No shader patching, expanded instance data or per-draw inspection.
 * Limits are physical, not enabled features. PointSize!=1 needs LARGE_POINTS;
 * shader writes must be positive; native range clamping and granularity apply.
 * polygon_mode_point_size reports whether polygon POINT uses shader point size.
 */
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
 * Restart is available for line/triangle strips (including adjacency) and triangle
 * fans, using the fixed all-ones index value for the bound index type.
 * List topologies additionally require LIST_RESTART; PATCHES requires PATCH_RESTART.
 * DEPTH_CLAMP enables clamping and, without DEPTH_CLIP_STATE, disables Z clipping.
 * DEPTH_CLIP_STATE on header.next controls clipping independently. Nonzero bias clamp needs DEPTH_BIAS_CLAMP even with bias disabled.
 * Non-fill modes, non-unit width, bounds testing, sample shading and alpha-to-one
 * need NON_SOLID_FILL/WIDE_LINES/DEPTH_BOUNDS/SAMPLE_SHADING/ALPHA_TO_ONE respectively.
 * Non-unit width must fit the reported range; native granularity/rounding applies.
 * Bounds testing uses the existing stored depth, not incoming fragment depth;
 * with no depth attachment the native test passes. Discard suppresses rasterization,
 * not pre-raster work; discard pipelines omit FRAGMENT.
 * Static discard requires zero color/depth formats. It can execute in an attached
 * render scope when UNUSED_ATTACHMENTS is enabled; otherwise use attachmentless scopes.
 * Attached static discard currently supports single-sample scopes only; native
 * static discard omits multisample state. Multisample attached discard remains work.
 * The viewport/scissor dynamic bit remains required by this profile.
 * Sample bit n controls sample n; bits beyond samples are ignored, including the
 * high 32 bits for counts <=32. Custom positions use SAMPLE_LOCATION_STATE; no mask expansion.
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
/* Complete dynamic input uses the same description, including optional divisors.
 * Requires FEATURE_DYNAMIC_VERTEX_INPUT and DYNAMIC_VERTEX_INPUT on consuming
 * executables; their static vertex_input is ignored without dereferencing it.
 * Counts-only scratch query. Inputs and disjoint writable scratch are borrowed
 * only through set_vertex_input. No allocation, compilation or retained layout.
 * The command replaces all descriptions; omitted locations become undefined.
 * Divisor zero additionally needs both VERTEX_DIVISOR and VERTEX_ZERO_DIVISOR on
 * this native dynamic path. Other fetch/format/divisor limits remain unchanged.
 * If DYNAMIC_VERTEX_STRIDE is also selected, a later bind_vertices with
 * VERTEX_BIND_STRIDE overrides that binding's stride; otherwise use the stride
 * supplied here. Applications establish valid state after static pipelines and
 * before draws, including inherited/generated work, and prove shader interface,
 * memory bounds/alignment and first-instance compatibility. No state tracker.
 */
ogpu_next_status ogpu_next_vertex_input_scratch_requirements(uint32_t bindings,
    uint32_t attributes, ogpu_next_host_requirements *out);
void ogpu_next_set_vertex_input(ogpu_next_encoder *, const ogpu_next_vertex_input *,
    ogpu_next_host_span scratch);
/* vertex_input=NULL means no fixed-function input (vertex pulling remains legal).
 * Otherwise VERTEX_INPUT describes explicit unique binding IDs and unique attribute
 * locations; each attribute references a listed binding. Sparse binding IDs are
 * allowed. Limits and native vertex-format support are checked during preparation.
 * Strides are static, including zero and overlapping records; attribute extent is
 * not artificially constrained to stride. Rate is per vertex or per instance with
 * divisor one unless VERTEX_DIVISORS is supplied on header.next. Shader input
 * types/locations and native fetch alignment are caller contracts. Metadata is
 * borrowed only during preparation. Dynamic input descriptions are not enabled;
 * dynamic stride uses
 * DYNAMIC_VERTEX_STRIDE and VERTEX_BIND_STRIDE; no repack or shader rewriting.
 */
/* Graphics preparation: RASTER must be enabled. kind=GRAPHICS, canonical order:
 * VERTEX, optional paired TESS_CONTROL/TESS_EVALUATION, optional GEOMETRY, optional
 * FRAGMENT (absent with discard). Stage features must be enabled independently;
 * with nonzero view_mask the matching *_MULTIVIEW enables are also required.
 * static_state=GRAPHICS_STATE,
 * dynamic_state includes DYNAMIC_VIEWPORT_SCISSOR and any selected supported
 * optional bits (graphics_limits.dynamic_states). requirements local_size/shared_memory=0.
 * The executable borrows one graphics argument layout; fields carry shader visibility.
 * With MESH_SHADER enabled, the alternative shader order
 * is MESH then optional FRAGMENT, or TASK/MESH then optional FRAGMENT when
 * TASK_SHADER is also enabled. Mesh pipelines supply MESH_REQUIREMENTS instead;
 * vertex_input=NULL, primitive_restart=0, and topology is ignored (the artifact
 * defines output topology). Layout fields name participating MESH/TASK stages and/or
 * FRAGMENT. All shaders share the borrowed layout and native byte namespace.
 * Fragment must be absent with discard for either path. Independent values use
 * distinct offsets. No reflected
 * resource list, automatic per-stage remapping or hidden upload/PSO compilation.
 * Color format count may be zero or up to max_colors. depth_format selects the
 * depth/stencil attachment format: 0 means neither, S8 means stencil only, and
 * depth-only/combined formats declare their actual aspects. No depth attachment
 * or depth sample-mask restriction is added for S8; depth test/write requires a
 * depth aspect. Stencil testing requires a stencil aspect;
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
 * sample locations remain an unimplemented profile.
 * Both vertex pulling and native vertex fetch are available. Topologies include
 * TRIANGLE_FAN, line/triangle list/strip ADJACENCY and PATCHES. Adjacency requires
 * GEOMETRY enabling; patch lists require the paired tessellation stages/state.
 * Primitive restart is available for strips/fan; lists/patches need their enables.
 * PointSize defaults to one when not written. Values other than one require
 * LARGE_POINTS; tessellation/geometry writes also need TESSELLATION_GEOMETRY_POINT_SIZE.
 * Storage writes
 * before rasterization use VERTEX_STORES_ATOMICS, fragment writes its own enable.
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
    const ogpu_next_shading_rate_attachment *shading_rate;
} ogpu_next_render_desc;
enum { OGPU_NEXT_RENDER_SECONDARY = 1, OGPU_NEXT_RENDER_SUSPENDING = 2, OGPU_NEXT_RENDER_RESUMING = 4 };
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
 * scopes. Area is explicit and nonempty. With view_mask=0, layers must be >0.
 * A nonzero view_mask requires MULTIVIEW and selects array indices relative to
 * each attachment view's first_layer; layers is ignored. The highest set bit must
 * fit max_multiview_views and every attachment/resolve view's layer_count.
 * Pipeline and secondary inheritance masks must exactly match the render mask.
 * Mesh pipelines additionally require MESH_MULTIVIEW and max_mesh_multiview_views.
 * Multiview direct draws validate first_instance+instances-1 against
 * max_multiview_instance_index; indirect/generated contents remain caller proof.
 * Queries begun inside multiview and timestamp writes consume popcount(view_mask)
 * consecutive pool slots. Sum occlusion/statistics results across these slots;
 * their distribution is native-defined. Queries cannot enclose multiview scopes.
 * Without RENDER_SECONDARY, commands are inline. RENDER_SECONDARY selects
 * secondary execution in a primary scope and disallows inline draws. Nested
 * execution is unsupported. RENDER_SUSPENDING makes render_end suspend this pass;
 * RENDER_RESUMING makes render_begin resume it. Both may be set for middle segments.
 * Load operations occur only on the initial segment; store/resolve only on final
 * end. Do NOT replace CLEAR with LOAD on resume: native rendering parameters must
 * match across segments except these three flags (host scratch addresses may differ).
 * All segments execute in order in ONE submit_desc batch. No action/synchronization
 * command or unrelated render pass may execute between suspend and resume, even
 * across list boundaries. Ordinary state-setting commands are permitted. Separate
 * submit_desc entries in one queue_submit call are different batches and cannot
 * be used to continue a pass. Caller proves matching parameters and ordering,
 * including composed secondary lists; the runtime retains no pass description,
 * resource graph or cross-list continuation registry. No implicit barrier, store,
 * reload, wait or intermediate allocation is inserted. Ending a list after a
 * suspending render_end is legal; ending while render scope is open remains invalid.
 * Each list supplies its own executable, arguments, heaps and dynamic state as
 * usual. Continuation preserves attachments, not command state across lists.
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
 * COLOR_ATTACHMENT usage and GENERAL/COLOR_ATTACHMENT state. It resolves at the final non-suspending
 * end, independently of source STORE/DISCARD; no temporary image or transition.
 * Depth/stencil resolves select one supported bit from graphics_limits'
 * depth_resolve_modes/stencil_resolve_modes, with identical source/target format,
 * DEPTH_STENCIL_ATTACHMENT usage and GENERAL/DEPTH_STENCIL_ATTACHMENT target state.
 * No feature enable is needed. With both depth and stencil views active, differing
 * modes require independent_resolve when neither is NONE, or independent_resolve_none
 * when one is NONE. When both resolve, they use the same target view/state.
 * Absent depth/stencil records do not impose paired-mode constraints. Source discard
 * does not discard the resolved target. Caller proves non-aliasing accesses; no
 * automatic shader resolve, temporary target, barrier or content tracking.
 * Native Vulkan attachment resolves (including depth/stencil) use COLOR stage
 * with COLOR_READ/COLOR_WRITE access, not standalone RESOLVE or DEPTH scopes.
 * Sample shading, masks and alpha coverage are explicit raster options; custom
 * sample positions use the explicit SAMPLE_LOCATIONS profile.
 *
 * Each draw TRUSTS matching pipeline/attachment formats, counts, samples and
 * depth/stencil write permission; shader output types and accesses must match. No per-draw
 * attachment scan/resource registry. Complete dynamic state (including viewport
 * arrays/counts) is a caller obligation; no slot-history validation.
 * Begin/end nesting, missing pipeline
 * and invalid command scope poison the list. Transfers/dispatch are outside
 * rendering. LOCAL_READ permits explicit framebuffer-region barriers inside.
 * Graphics/compute bind points are independent. Viewport/scissor and byte state
 * persist across compatible binds/scopes, reset to undefined by command begin.
 * Dynamic viewport allows negative height and reversed depth endpoints within
 * [0,1] unless UNRESTRICTED_DEPTH_RANGE is requested.
 * set_graphics_state accepts VIEWPORT_STATE as a slot-zero shorthand;
 * blend/stencil controls have their own setters. Draw requires
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
 * across command begin/reset. VERTEX_BIND_STRIDE sets dynamic stride <=
 * max_vertex_stride and requires DYNAMIC_VERTEX_STRIDE on consuming pipelines.
 * flags=0 requires stride=0 and preserves the static-stride strategy. Consuming
 * pipelines must match this choice; rebind explicitly when switching strategies.
 * No pipeline-dependent rewriting, draw-time conversion or implicit state update.
 * Each draw TRUSTS that every fetched binding is set and that all vertex/instance
 * indices (including first/base offsets), attribute offsets, strides and formats
 * address aligned, initialized bytes within the supplied spans. No per-draw scan.
 * Spans must remain alive and synchronized through recorded future/pending uses.
 */
ogpu_next_status ogpu_next_vertex_scratch_requirements(uint32_t count, ogpu_next_host_requirements *);
enum { OGPU_NEXT_VERTEX_BIND_STRIDE = 1 };
typedef struct ogpu_next_vertex_buffer_binding {
    ogpu_next_span span;
    uint32_t stride, flags;
} ogpu_next_vertex_buffer_binding;
void ogpu_next_bind_vertices(ogpu_next_encoder *, uint32_t first, uint32_t count, const ogpu_next_vertex_buffer_binding *, ogpu_next_host_span scratch);
void ogpu_next_draw_indexed(ogpu_next_encoder *, const ogpu_next_draw_desc *);
void ogpu_next_draw_indirect(ogpu_next_encoder *, const ogpu_next_indirect *);
void ogpu_next_draw_indexed_indirect(ogpu_next_encoder *, const ogpu_next_indirect *);
/* Requires MESH_SHADER and a bound mesh executable, rendering scope and viewport.
 * Direct dimensions use task limits when TASK is present, otherwise mesh limits;
 * each axis and their product must fit. Zero-work dimensions are permitted.
 * Indirect uses the same explicit span/count/stride model above; each wire record
 * is three uint32_t group counts (12 bytes, identical to ogpu_next_extent).
 * GPU argument dimensions and task-emitted mesh dimensions must obey queried
 * limits at execution; OGPU does not read, clamp or rewrite them. Ordinary draw
 * and indexed draw reject mesh executables; mesh draw rejects vertex executables.
 * Active vertex/input-assembly/geometry/clipping/tessellation statistics are
 * incompatible with mesh draws. Fragment/compute statistics and occlusion remain
 * usable; mesh-specific statistics are not enabled by this profile.
 */
void ogpu_next_draw_mesh(ogpu_next_encoder *, ogpu_next_extent groups);
void ogpu_next_draw_mesh_indirect(ogpu_next_encoder *, const ogpu_next_indirect *);

/* Optional device-generated commands. Availability is native, not emulation.
 * It is independent of RASTER, MESH and TASK enabling; stages used by a layout
 * must also be enabled and supported by GENERATED_LIMITS. The Vulkan generated
 * mesh-token profile requires both MESH_SHADER and TASK_SHADER enabled, even for
 * mesh-only executables; ordinary mesh draws do not have this extra requirement. */
#define OGPU_NEXT_FEATURE_GENERATED_COMMANDS UINT32_C(61)
#define OGPU_NEXT_USAGE_PREPROCESS UINT64_C(256)
#define OGPU_NEXT_STAGE_PREPROCESS UINT64_C(4194304)
#define OGPU_NEXT_ACCESS_PREPROCESS_READ UINT64_C(2097152)
#define OGPU_NEXT_ACCESS_PREPROCESS_WRITE UINT64_C(4194304)
enum { OGPU_NEXT_QUERY_GENERATED_LIMITS = 18, OGPU_NEXT_GENERATED_LAYOUT_DESC = 130,
    OGPU_NEXT_GENERATED_SET_DESC = 131, OGPU_NEXT_COMPILE_INDIRECT_BINDABLE = 2 };
typedef struct ogpu_next_generated_layout ogpu_next_generated_layout;
typedef struct ogpu_next_generated_set ogpu_next_generated_set;
typedef struct ogpu_next_generated_limits {
    ogpu_next_stages stages, pipeline_binding_stages;
    uint64_t tokens; /* Bit (1 << token_kind); intersect with stage/feature support. */
    uint32_t max_pipelines, max_sequences, max_tokens, max_token_offset, max_stride, reserved;
    uint32_t index_u16, index_u32; /* Queried GPU wire encodings; valid with INDEX_BUFFER token support. */
} ogpu_next_generated_limits;
enum {
    OGPU_NEXT_GENERATED_EXECUTABLE = 1, OGPU_NEXT_GENERATED_ARGUMENT = 2,
    OGPU_NEXT_GENERATED_SEQUENCE_INDEX = 3, OGPU_NEXT_GENERATED_DISPATCH = 4,
    OGPU_NEXT_GENERATED_DRAW = 5, OGPU_NEXT_GENERATED_DRAW_INDEXED = 6,
    OGPU_NEXT_GENERATED_MESH = 7, OGPU_NEXT_GENERATED_DRAW_COUNT = 8,
    OGPU_NEXT_GENERATED_DRAW_INDEXED_COUNT = 9, OGPU_NEXT_GENERATED_MESH_COUNT = 10,
    OGPU_NEXT_GENERATED_VERTEX_BUFFER = 11, OGPU_NEXT_GENERATED_INDEX_BUFFER = 12,
    OGPU_NEXT_GENERATED_EXPLICIT_PREPROCESS = 1, OGPU_NEXT_GENERATED_UNORDERED = 2
};
typedef struct ogpu_next_generated_token {
    uint32_t kind, offset, target, size;
} ogpu_next_generated_token;
typedef struct ogpu_next_generated_layout_desc {
    ogpu_next_record header;
    ogpu_next_argument_layout *arguments;
    ogpu_next_stages stages;
    uint32_t flags, stride, token_count, reserved;
    const ogpu_next_generated_token *tokens;
} ogpu_next_generated_layout_desc;
typedef struct ogpu_next_generated_set_desc {
    ogpu_next_record header;
    ogpu_next_executable *initial;
    uint32_t capacity, reserved;
} ogpu_next_generated_set_desc;
typedef struct ogpu_next_generated_set_write {
    uint32_t index, reserved;
    ogpu_next_executable *executable;
} ogpu_next_generated_set_write;
typedef struct ogpu_next_generated_work {
    ogpu_next_generated_layout *layout;
    ogpu_next_generated_set *set;
    ogpu_next_executable *executable;
    uint32_t max_sequences, max_draws;
} ogpu_next_generated_work;
typedef struct ogpu_next_generated_requirements {
    uint64_t size, alignment;
    uint32_t memory_type_bits, reserved;
} ogpu_next_generated_requirements_info;
typedef struct ogpu_next_generated_batch {
    ogpu_next_generated_work work;
    ogpu_next_span arguments, preprocess, count;
} ogpu_next_generated_batch;
/* Nested-count GPU wire record: command_count draws at address/stride, bounded
 * by work.max_draws. This is an address, not an OGPU memory handle. */
typedef struct ogpu_next_generated_draw_count {
    ogpu_next_address address;
    uint32_t stride, command_count;
} ogpu_next_generated_draw_count;
typedef struct ogpu_next_generated_vertex_buffer {
    ogpu_next_address address;
    uint32_t size, stride;
} ogpu_next_generated_vertex_buffer;
typedef struct ogpu_next_generated_index_buffer {
    ogpu_next_address address;
    uint32_t size, index_type;
} ogpu_next_generated_index_buffer;
/* Layouts borrow a same-device argument layout of the matching execution class;
 * it outlives the generated layout and all recorded/preprocessed/pending uses.
 * No token array is retained. One action token is last, with optional EXECUTABLE
 * selection first (one u32 table index). ARGUMENT copies size bytes from
 * record+offset to target in that layout. Bounds and indivisible 8-byte
 * native-address updates are the same as set_arguments. Native addresses are
 * supplied directly by the native generated token, never decoded on the CPU.
 * GPU-produced address values must satisfy the field's alignment/access contract.
 * SEQUENCE_INDEX targets one declared four-byte INLINE field and writes the native
 * u32 index; it consumes no record bytes, at most one. Destinations cannot overlap.
 * VERTEX_BUFFER uses target as the vertex binding and size=0; INDEX_BUFFER uses
 * target=size=0. Each vertex destination and the index token occur at most once;
 * both are vertex-graphics only. Other tokens require target=size=0. Offsets are ordered,
 * 4-aligned and queried-limit-bounded. Stride is 4-aligned, may be zero or smaller
 * than the record extent: overlapping source records are not forcibly repacked.
 * Wire actions: DISPATCH/MESH=extent; DRAW/DRAW_INDEXED=existing wire records;
 * *_COUNT=generated_draw_count; VERTEX_BUFFER=generated_vertex_buffer;
 * INDEX_BUFFER=generated_index_buffer. Buffer addresses refer to VERTEX/INDEX
 * memory; caller owns bounds, alignment and pending lifetime. index_type uses
 * queried generated_limits.index_u16/index_u32 (not INDEX_U16/INDEX_U32).
 * These are the backend's GPU wire values; no CPU/GPU format translation is added.
 * Vertex tokens require DYNAMIC_VERTEX_STRIDE. Targets must be declared in static
 * input or established by the caller with DYNAMIC_VERTEX_INPUT before generated
 * work. The cold requirements query checks the initial executable;
 * set members must preserve that state contract. Ray tokens and shader-object
 * selection remain unimplemented. Indexed work without an index token uses bound
 * indices. Layout stages name one exact compute/vertex/mesh executable family.
 * UNORDERED permits native sequence reordering; compute sequences are inherently
 * unordered even without it. No sequence-to-sequence memory dependency is implied.
 * EXPLICIT_PREPROCESS selects a separate preparation call; otherwise execution
 * performs native preprocessing. Both modes use caller-provided backing.
 */
ogpu_next_status ogpu_next_generated_layout_create(ogpu_next_device *, const ogpu_next_generated_layout_desc *, ogpu_next_generated_layout **);
void ogpu_next_generated_layout_destroy(ogpu_next_generated_layout *);
/* Set creation stores initial in slot 0; other slots are undefined until written.
 * Every member must have COMPILE_INDIRECT_BINDABLE enabled at executable creation;
 * ordinary pipelines are not implicitly recompiled. All members have the same
 * stages and compatible argument layouts (checked before the native update).
 * Compatibility requires equal byte extent, execution class, native resource
 * mappings and address offsets/alignments; separate equivalent layouts are valid.
 * Direct INLINE field partitioning need not match. The caller guarantees identical
 * native static/dynamic graphics state, fragment output interface and depth/sample-
 * mask/stencil export use, and compatible artifact argument meaning across members.
 * No native pipeline reflection or host shader rewriting. Initial executable stays
 * alive until set destruction, even if slot 0 is updated. Other members stay alive
 * while referenced by set slots or recorded/preprocessed/pending execution.
 * Updates are cold, all local checks precede one native update; empty is a no-op.
 * Exclude host set calls and pending uses of updated slots. Re-query requirements
 * after every update; prepared data using changed slots must be regenerated.
 */
ogpu_next_status ogpu_next_generated_set_create(ogpu_next_device *, const ogpu_next_generated_set_desc *, ogpu_next_generated_set **);
ogpu_next_status ogpu_next_generated_set_update(ogpu_next_generated_set *, uint32_t count, const ogpu_next_generated_set_write *);
void ogpu_next_generated_set_destroy(ogpu_next_generated_set *);
/* Work selects either set (EXECUTABLE token, executable=NULL) or one fixed
 * executable (set=NULL). max_sequences>0 within queried limit. max_draws bounds
 * each nested *_COUNT action; ignored for other actions. When used, their product
 * must be <2^24 in this Vulkan profile. Query after set changes and before backing
 * allocation; output is unchanged on failure. No GPU object is allocated by query.
 * This cold query checks argument-layout compatibility with the fixed executable
 * or set's initial executable. Query each distinct work pairing before recording;
 * recording trusts this compatibility proof, without scanning mappings or fields.
 * PREPROCESS backing must satisfy BOTH this returned size/alignment/type mask and
 * ordinary memory_requirements. Bits index queried memory-type IDs. Recording
 * trusts this native requirement match: no driver query or size cache on the hot
 * path. Null preprocess span allowed only when native size is zero.
 */
ogpu_next_status ogpu_next_generated_requirements(ogpu_next_device *, const ogpu_next_generated_work *, ogpu_next_generated_requirements_info *);
/* Primary encoders only. All objects/backing/reachable GPU addresses remain live
 * through every recorded/preprocessed/pending use; no retention or hidden waits.
 * Argument/count spans use INDIRECT usage, 4-aligned addresses; count optionally
 * points to one u32. Null count means max_sequences; otherwise native execution
 * uses min(count,max_sequences), and count itself must obey queried/preparation
 * maxima. arguments covers (max_sequences-1)*stride + maximum token end, >0.
 * All GPU records, indices, dimensions, nested addresses/counts and shader accesses
 * are TRUSTED valid at execution, including per-stage draw/dispatch limits.
 * Preprocessing runs outside rendering and snapshots a still-recording state
 * encoder on the same device. It borrows that encoder only for this host call.
 * State must bind the fixed executable, or the set's ORIGINAL initial executable.
 * Execution binds that same executable before each call, and uses a compatible
 * render scope/viewport and required bindings for graphics, outside rendering for
 * compute. Buffer state supplied by a token needs no prior CPU binding.
 * preprocessed is 0/1 and must match EXPLICIT_PREPROCESS. For 1, preparation must
 * have completed with identical work/input content and compatible native state;
 * execute/state encoder queue families match, and referenced set slots unchanged.
 * Caller synchronizes GPU producer -> PREPROCESS/READ for explicit preprocessing,
 * PREPROCESS/WRITE -> INDIRECT/READ before prepared execution; integrated mode's
 * inputs use INDIRECT/READ. PREPROCESS accesses do not imply compute shader access.
 * Preserve native state (including argument bytes/heaps) between snapshot and use.
 * Executable-selection tokens invalidate the corresponding executable binding;
 * argument/sequence tokens make the affected native argument domain undefined afterward.
 * Vertex/index tokens also leave the corresponding binding state undefined;
 * ordinary indexed work must explicitly rebind indices afterward, even for zero
 * generated sequences. Restore affected vertex bindings/strides before later work.
 * Restore those explicitly for subsequent work. No hidden state restoration,
 * CPU readback, per-sequence host loop, compilation or allocation during recording.
 */
void ogpu_next_generated_preprocess(ogpu_next_encoder *, const ogpu_next_generated_batch *, ogpu_next_encoder *state);
void ogpu_next_generated_execute(ogpu_next_encoder *, const ogpu_next_generated_batch *, uint32_t preprocessed);
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
 * scope. An outside-scope query may enclose complete non-multiview scopes. An inside-scope query
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
typedef struct ogpu_next_binary_point ogpu_next_binary_point;
typedef struct ogpu_next_submit_desc {
    uint32_t list_count, wait_count, signal_count;
    uint32_t binary_wait_count, binary_signal_count, flags;
    ogpu_next_list *const *lists;
    const ogpu_next_sync_point *waits, *signals;
    const ogpu_next_binary_point *binary_waits, *binary_signals;
} ogpu_next_submit_desc;
enum { OGPU_NEXT_SUBMIT_REJECTED = 0, OGPU_NEXT_SUBMIT_ACCEPTED = 1, OGPU_NEXT_SUBMIT_UNKNOWN = 2 };
typedef struct ogpu_next_submit_outcome {
    ogpu_next_status status;
    uint32_t acceptance;
} ogpu_next_submit_outcome;
/* One submission operation: count=1 is ordinary submit. Preserve each ordered
 * list/wait/signal boundary; no flattening, automatic signals, waits or receipts.
 * One caller scratch span covers ALL native headers and nested arrays. Scratch
 * and mandatory count-entry outcomes are aligned, writable and mutually disjoint
 * from every input/object. Arrays/scratch/outcomes are borrowed only for the call.
 * Query reads only counts/flags; nested pointers may be unpopulated. Checked size
 * arithmetic; flags=0. Failed queries preserve output. count=0 requires no storage
 * (alignment=1), permits NULL arrays, and is a no-op on a live non-lost device.
 * Too-small scratch => CAPACITY; null/misaligned nonempty scratch => INVALID.
 * Every local check precedes native effects: all outcomes are REJECTED on local
 * failure, with the rejection status. Success is ACCEPTED, not completion.
 * Proven native rejection is REJECTED; Vulkan OOM leaves all batches retryable.
 * Unclassified/partial native failure is UNKNOWN for affected batches. Preserve
 * any proven earlier acceptance/later non-attempted rejection; not-attempted
 * outcomes carry the stopping error. Vulkan uses one vkQueueSubmit2 and gives
 * all batches the same acceptance. UNKNOWN is not retryable, poisons the device
 * and protects all potentially referenced objects until conservative recovery.
 * Accepted/UNKNOWN one-shots are consumed. REJECTED does not retire earlier uses.
 * Aggregate status preserves failure; every valid outcome entry is written.
 *
 * Queue access is externally serialized. Lists/timelines/binaries belong to its
 * device; lists match its domain. Caller guarantees duplicate-list legality and
 * replay/pending lifetimes, unique wait/signal semaphores within each batch and
 * native value ordering. No quadratic scan or resource walk. Binary signals target
 * unsignaled semaphores; each is consumed by one ordered wait. No state registry.
 * Wait/signal stages are preserved; HOST is invalid. Only an appropriate completed
 * signal scope proves retirement; an ALL signal after work proves whole-list
 * completion. Timeout/loss never prove completion. A later REJECTED call does not
 * retire resources used by an earlier ACCEPTED/UNKNOWN call.
 */
ogpu_next_status ogpu_next_submit_scratch_requirements(const ogpu_next_device *, uint32_t count, const ogpu_next_submit_desc *, ogpu_next_host_requirements *);
ogpu_next_status ogpu_next_queue_submit(ogpu_next_queue *, uint32_t count, const ogpu_next_submit_desc *, ogpu_next_submit_outcome *, ogpu_next_host_span scratch);

#define OGPU_NEXT_DOMAIN_IGNORED UINT32_MAX
enum { OGPU_NEXT_QUERY_DEPENDENCY_LIMITS = 19,
    OGPU_NEXT_DEPENDENCY_SPLIT = 1, OGPU_NEXT_DEPENDENCY_OWNERSHIP = 2, OGPU_NEXT_DEPENDENCY_BINARY = 4,
    OGPU_NEXT_MEMORY_SCOPE_NONE = 0, OGPU_NEXT_MEMORY_SCOPE_OBJECT = 1, OGPU_NEXT_MEMORY_SCOPE_BYTES = 2 };
/* Device-only query: one immutable record per enabled queue domain. Masks report
 * representable scopes, not measured overlap. Other native validity rules apply. */
typedef struct ogpu_next_dependency_limits {
    uint32_t domain, flags, memory_scope, reserved;
    ogpu_next_stages wait_stages, signal_stages, split_stages;
} ogpu_next_dependency_limits;
typedef struct ogpu_next_barrier_scope {
    ogpu_next_stages stages_before, stages_after;
    ogpu_next_access access_before, access_after;
} ogpu_next_barrier_scope;
typedef struct ogpu_next_memory_barrier {
    ogpu_next_barrier_scope scope;
    ogpu_next_span range;
    ogpu_next_queue_domain source_domain, destination_domain;
} ogpu_next_memory_barrier;
enum {
    OGPU_NEXT_STATE_UNDEFINED = 0, OGPU_NEXT_STATE_GENERAL = 1,
    OGPU_NEXT_STATE_COPY_SRC = 2, OGPU_NEXT_STATE_COPY_DST = 3,
    OGPU_NEXT_STATE_SHADER_READ = 4, OGPU_NEXT_STATE_COLOR_ATTACHMENT = 5,
    OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT = 6, OGPU_NEXT_STATE_DEPTH_STENCIL_READ = 7,
    OGPU_NEXT_STATE_PRESENT = 8 /* Borrowed swapchain images only; never elided. */
};
struct ogpu_next_image_barrier {
    ogpu_next_barrier_scope scope;
    ogpu_next_image *image;
    ogpu_next_subresources range;
    ogpu_next_image_state old_state, new_state;
    ogpu_next_queue_domain source_domain, destination_domain;
    uint32_t discard, reserved;
    const ogpu_next_record *next; /* optional SAMPLE_LOCATIONS; NULL otherwise */
};
typedef struct ogpu_next_dependency {
    uint32_t global_count, memory_count, image_count, flags;
    const ogpu_next_barrier_scope *global;
    const ogpu_next_memory_barrier *memory;
    const ogpu_next_image_barrier *images;
} ogpu_next_dependency;
/* One native batched dependency with independent per-entry scopes. Empty is a
 * no-op; execution-only ordering is an explicit global entry with zero accesses.
 * All local validation precedes the native command. Scratch is aligned, writable
 * and disjoint from every input; outputs/arrays are borrowed only during the call.
 * Source/destination both IGNORED => no ownership transfer. Otherwise explicit
 * exclusive-family release/acquire endpoints require matching ranges/domains and
 * a queue timeline edge. No implicit owner tracking. Empty memory ranges invalid.
 * Stage NONE (0) requires access NONE; ALL includes GPU stages, not HOST.
 * Image states are explicit native layouts, never a tracked current state. The
 * caller proves old_state and consistent state at execution/replay. discard=1
 * discards contents via UNDEFINED; it does NOT remove synchronization obligations.
 * new_state cannot be UNDEFINED. Depth/stencil combined formats currently require
 * both aspects in transitions (separate-layout enabling follows).
 * flags may include BY_REGION and VIEW_LOCAL. VIEW_LOCAL requires rendering.
 * Inside rendering requires LOCAL_READ and inline command contents: only FRAGMENT,
 * COLOR, DEPTH/EARLY_DEPTH/LATE_DEPTH stages, BY_REGION for nonzero source scopes,
 * VIEW_LOCAL with multiple views. No layout change, discard or ownership transfer.
 * Bound attachment images in image barriers must use GENERAL or LOCAL_READ;
 * caller proves binding and same-region access legality (no attachment registry).
 */
ogpu_next_status ogpu_next_barrier_scratch_requirements(const ogpu_next_device *, uint32_t global_count, uint32_t memory_count, uint32_t image_count, ogpu_next_host_requirements *);
void ogpu_next_barrier(ogpu_next_encoder *, const ogpu_next_dependency *, ogpu_next_host_span scratch);
/* Independent device-only split dependency. Creation is cold and initially
 * unsignaled. The caller keeps split/device and referenced resources alive through
 * all recorded future/pending uses; destruction never waits or retains commands.
 * Objects are not arena-local: endpoints may be recorded independently, but every
 * acquire must follow its matching release on the SAME native queue in submission
 * order. This is not a timeline, cross-queue wait, or general GPU-address semaphore.
 * Compute/graphics domains only, outside rendering, no HOST scope, dependency flags=0.
 * Primary and
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
void ogpu_next_split_release(ogpu_next_encoder *, const ogpu_next_split *, const ogpu_next_dependency *, ogpu_next_host_span scratch);
typedef struct ogpu_next_split_dependency {
    const ogpu_next_split *split;
    const ogpu_next_dependency *dependency;
} ogpu_next_split_dependency;
/* Counts/flags only: reads dependency records, not events or nested arrays.
 * Both scratch queries leave output unchanged on failure; zero storage aligns to 1. */
ogpu_next_status ogpu_next_split_scratch_requirements(const ogpu_next_device *, uint32_t count, const ogpu_next_split_dependency *, ogpu_next_host_requirements *);
/* Batch acquire uses one native wait. Every local record validates first; failure
 * records no wait and poisons end. count=0 is a no-op on a valid encoder/domain.
 * One scratch span covers all native headers and nested arrays, aligned and
 * disjoint from inputs. All description/scratch storage is borrowed
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
 * Current copy profile: matching dimensionality/sample counts, color formats
 * of equal block size AND block extent (raw bits, no conversion), or identical
 * depth/stencil formats. Offsets, mip/layer bounds and queue granularity checked.
 * Depth/stencil copies currently require a graphics family. 2D-array/3D cross-
 * dimensional mappings, unequal-block-extent/multiplanar and depth/color reinterpretation
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
 * Pitches are bytes: zero means tight, row is a copy-block-byte multiple, slice
 * is a block-row-pitch multiple. Partial edge blocks still occupy whole blocks.
 * Offsets are block-aligned; extents are block multiples or end at the mip edge.
 * Native queue granularity (in blocks for compressed images) and address alignment
 * are checked. Row/slice
 * texel counts must fit native uint32 fields; row bytes must fit INT32_MAX.
 * reserved=0. No implicit staging.
 * Compressed clears are UNSUPPORTED, never synthesized. Ordinary color clear
 * needs graphics/compute; depth/stencil clear/copy currently needs a
 * graphics family. Select the clear-value member matching the image's numeric
 * format (float for normalized/float, uint for unsigned, depth_stencil for depth).
 * Depth clear values must be finite and in [0,1], including stencil-only clears.
 */
void ogpu_next_copy_to_image(ogpu_next_encoder *, ogpu_next_image *dst, ogpu_next_span src, const ogpu_next_image_copy *);
void ogpu_next_copy_from_image(ogpu_next_encoder *, ogpu_next_span dst, ogpu_next_image *src, const ogpu_next_image_copy *);
void ogpu_next_clear_image(ogpu_next_encoder *, ogpu_next_image *, ogpu_next_image_state, const ogpu_next_subresources *, const ogpu_next_clear_value *);

/* Optional presentation setup (mutable revision 1; no swapchain runtime yet).
 * PRESENTATION is an optional ordinary device_create feature. It is available
 * only through an opt-in presentation discovery AND a supporting physical adapter.
 * Requesting it enables VK_KHR_swapchain and KHR swapchainMaintenance1, but creates
 * no surface, picks no queue and acquires no images. It is independent of RASTER.
 * Omitting it leaves even a WSI-enabled discovery's device headless. Physical
 * support can still be queried through adapter_support on a headless discovery.
 * Table lookup does not load Vulkan or access a display. Exact version and table
 * byte size are required; failures preserve the table. Plain discovery_create
 * stays headless and does not require WSI. The table's discovery_create enables
 * exactly PRESENTATION_WAYLAND (other selections return UNSUPPORTED), requiring
 * Vulkan KHR surface/Wayland/get_surface_capabilities2/surface_maintenance1.
 * This enables no device extension and chooses no queue or window. adapter_support
 * queries physical-device profile availability without a window, even on a
 * headless discovery. It may allocate temporary native enumeration storage;
 * failure leaves its output unchanged. Availability does not enable the instance
 * or device profile. Use ordinary
 * discovery_count/adapter/destroy with the resulting owner.
 *
 * surface_wayland_create borrows a live wl_display and wl_surface, not the window
 * or event loop; they must belong together and outlive surface_destroy. The
 * wl_surface must not already have another Vulkan surface. OGPU owns only the
 * native GPU surface and retains its instance independently of discovery. All
 * swapchains must be destroyed before the surface.
 * Serialize surface/native-window lifecycle and obey the window library's thread
 * rules. Destruction accepts NULL and never waits; create clears its output on
 * failure. Surfaces may be queried by any adapter in the SAME discovery, not by
 * a separately created discovery even for the same physical GPU.
 *
 * surface_query is explicitly a LIVE COLD-PATH query: unlike immutable adapter
 * queries it may allocate temporary enumeration storage and retry count changes.
 * It performs no GPU wait. The usual count/capacity/output-atomicity contract
 * applies. Capabilities can change immediately; success is not a future guarantee.
 * Query data schemas: CAPABILITIES->surface_capabilities (one), FORMATS->surface_format,
 * PRESENT_MODES->uint32_t, QUEUES->surface_queue (all adapter domains).
 * Unknown queries unsupported.
 *
 * This first profile is explicitly Vulkan Wayland: color_space, present-mode,
 * transform and alpha tokens use VkColorSpaceKHR/VkPresentModeKHR/
 * VkSurfaceTransformFlagBitsKHR/VkCompositeAlphaFlagBitsKHR numeric values. No
 * Vulkan headers are needed. SRGB_NONLINEAR=0; present IMMEDIATE=0, MAILBOX=1,
 * FIFO=2, FIFO_RELAXED=3; transform IDENTITY=1; alpha OPAQUE=1. These are exact
 * native capability tokens, not promises of Metal equivalents or HDR support.
 * format is an OGPU format or 0 if not representable; native_format always keeps
 * the original VkFormat value. No formats are silently removed/substituted.
 * max_image_count=0 means unbounded; current_extent=UINT32_MAX means caller-sized.
 * image_usage contains representable OGPU image bits; native_image_usage keeps
 * all native bits. The selected device profile will require both swapchain and
 * maintenance, enabled only when device_create explicitly requests PRESENTATION.
 *
 * surface_mode_query accepts a supported native present mode and only CAPABILITIES,
 * SCALING (surface_scaling) or COMPATIBLE_MODES (uint32_t[]). It rejects unsupported
 * modes and preserves the same output/error contract. Mode-specific image counts
 * require an explicit matching native mode list at swapchain creation; they must
 * not be used with an unqualified creation strategy. COMPATIBLE_MODES reports
 * native mode switching without recreation; it does not enable switching or choose
 * an allowed list. SCALING/gravity are native VkPresentScalingFlagsKHR and
 * VkPresentGravityFlagsKHR masks, including zero for unavailable capabilities.
 * Scaling ONE_TO_ONE=1, ASPECT_RATIO_STRETCH=2, STRETCH=4; gravity MIN=1, MAX=2,
 * CENTERED=4. No implicit image scaling, extent clamping or fallback policy.
 */
#define OGPU_NEXT_FEATURE_PRESENTATION UINT32_C(58)
typedef struct ogpu_next_surface ogpu_next_surface;
typedef struct ogpu_next_binary ogpu_next_binary;
typedef struct ogpu_next_fence ogpu_next_fence;
typedef struct ogpu_next_swapchain ogpu_next_swapchain;
enum {
    OGPU_NEXT_PRESENTATION_WAYLAND = 1u,
    OGPU_NEXT_WAYLAND_SURFACE_DESC = 125,
    OGPU_NEXT_SWAPCHAIN_DESC = 127, /* 128 retired */
    OGPU_NEXT_QUERY_SURFACE_CAPABILITIES = 1000,
    OGPU_NEXT_QUERY_SURFACE_FORMATS = 1001,
    OGPU_NEXT_QUERY_SURFACE_PRESENT_MODES = 1002,
    OGPU_NEXT_QUERY_SURFACE_QUEUES = 1003,
    OGPU_NEXT_QUERY_SURFACE_SCALING = 1004,
    OGPU_NEXT_QUERY_SURFACE_COMPATIBLE_MODES = 1005
};
typedef struct ogpu_next_wayland_surface_desc {
    ogpu_next_record header;
    void *display, *surface;
} ogpu_next_wayland_surface_desc;
typedef struct ogpu_next_surface_capabilities {
    uint32_t min_image_count, max_image_count;
    uint32_t current_extent[2], min_extent[2], max_extent[2];
    uint32_t max_layers, supported_transforms, current_transform, supported_alpha;
    uint64_t image_usage;
    uint32_t native_image_usage, reserved;
} ogpu_next_surface_capabilities;
typedef struct ogpu_next_surface_format {
    ogpu_next_format format;
    uint32_t native_format, color_space, reserved;
} ogpu_next_surface_format;
typedef struct ogpu_next_surface_queue {
    uint32_t domain, supported;
} ogpu_next_surface_queue;
typedef struct ogpu_next_surface_scaling {
    uint32_t scaling, gravity_x, gravity_y, reserved;
    uint32_t min_extent[2], max_extent[2];
} ogpu_next_surface_scaling;
typedef struct ogpu_next_presentation_support {
    uint32_t swapchain, swapchain_maintenance;
} ogpu_next_presentation_support;
/* Explicit native synchronization, also usable on headless devices: neither
 * PRESENTATION nor a surface is needed to create these objects. No implicit
 * frame ring, retention, host wait, state tracking or timeline conversion.
 * binary_create returns an unsignaled binary semaphore. Signals must target an
 * unsignaled binary; one ordered wait consumes each signal. The caller proves
 * valid pending signals/waits and retirement before reuse/destruction. A render
 * timeline signal does not by itself retire a presentation wait on a binary.
 * There is no binary host signal/reset/poll/wait operation.
 *
 * Each submit_desc carries binary arrays alongside its timeline arrays. Query
 * scratch with the complete descriptions and submit with one shared span and
 * explicit outcomes. Zero counts allow NULL arrays. No binary bridge submission,
 * extension chain or per-description scratch is needed. Native binary values
 * are zero; pending/order and duplicate-use validity remain caller obligations.
 */
typedef struct ogpu_next_binary_point {
    ogpu_next_binary *binary;
    ogpu_next_stages stages;
} ogpu_next_binary_point;
/* fence_create accepts signaled=0 or 1. poll returns OK or NOT_READY; wait returns
 * OK or TIMEOUT (or errors). reset is explicit and never waits; it requires no
 * pending signal or other host access and may reset an idle unsignaled fence.
 * A signaled fence retires only the native operation associated with it, not
 * unrelated rendering, scanout or image reacquisition. Acquire and present may
 * associate native signals; caller explicitly resets before reuse.
 * Device must outlive its binaries/fences and all uses. Independent objects may
 * be used concurrently; poll/wait on one fence may also run concurrently. Reset
 * and destruction exclude all other host access, and pending uses must be retired
 * before destruction. Queue access/reuse obeys the usual external synchronization.
 * Destroy accepts NULL and never waits. Creation clears output on failure. Fence timeout is not
 * completion and does not change ownership or permit destruction of pending work.
 */
/* Exact caller-selected creation; no clamp, implicit queue or fallback. Surface,
 * device and borrowed native window outlive the chain. PRESENTATION must be enabled.
 * format is an OGPU format; color_space/transform/alpha/present_mode are native
 * profile tokens. Initial profile supports modes 0..3, ordinary non-protected,
 * non-mutable-format images and no scaling; other profiles remain explicit gaps.
 * domain_count=0 selects exclusive sharing; >=2 unique domains selects concurrent.
 * mode_count=0 uses base capabilities. A nonempty compatible list includes
 * present_mode and selects per-mode limits for every listed mode. Runtime mode
 * switching is not yet implemented. min_image_count is a minimum, not exact count.
 * Creation may query/allocate/retry. Once native creation names old_swapchain,
 * that chain is retired EVEN ON FAILURE. Preflight rejection makes no native call;
 * no unconditional rollback promise. Retired chains may still present previously
 * acquired images, but must not acquire again. Destroy never waits.
 *
 * chain_images: capacity=0 writes count only; short capacity writes count and
 * CAPACITY, never a partial array. Other failures preserve outputs. Returned image
 * pointers borrow the chain and stay stable until its destruction. No independent
 * backing: image_bind/dedicated memory creation reject them; image_destroy is a
 * no-op. Views are independently owned and must be destroyed before the chain.
 * Ordinary image commands apply only while acquired with ordered dependencies.
 * STATE_PRESENT is a real transition, including with UNIFIED_IMAGES enabled.
 */
typedef struct ogpu_next_swapchain_desc {
    ogpu_next_record header;
    ogpu_next_surface *surface;
    ogpu_next_swapchain *old_swapchain;
    uint32_t format, color_space, extent[2], layer_count, min_image_count;
    uint64_t usage;
    uint32_t transform, alpha, present_mode, clipped, domain_count, mode_count;
    const uint32_t *domains, *modes;
} ogpu_next_swapchain_desc;
/* Borrowed frame lifecycle. See docs/minimal-core-presentation.md for the full
 * contract. Objects/arrays are trusted, live, externally synchronized and disjoint.
 * Copying a frame does not acquire it again. Its historical binary/fence identities
 * are not dereferenced by present/abandon. Generation identifies the image set.
 * acquire publishes only on OK/SUBOPTIMAL; other results preserve its output.
 * Vulkan requires >=1 idle same-device acquisition binary/fence and the caller must
 * obey native forward-progress/acquired-count limits. Order access after acquisition.
 * chain_create always reports previous_state and NULL on failure; a recognized full
 * record preserves the old state on local rejection. Native replacement retires the
 * old chain even on failure, but previously acquired frames may still be used.
 * resize is capability-dependent, never drains: release old views/recordings/native
 * references and retire uses first. Invalidation increments generation even if native
 * resize fails. Vulkan reports UNSUPPORTED/IMAGES_UNCHANGED. info is a host snapshot.
 * queue_present preflights the WHOLE batch, then uses one native Vulkan call. No
 * hidden submit/wait/transition/allocation. Native present may itself block. Frames
 * are acquired, STATE_PRESENT and owned/shared for this queue, with writes ordered
 * by explicit waits. No duplicate chain/wait/fence; retirement fences idle/unsignaled.
 * item_count>0; results required. Scratch, results and inputs must not overlap.
 * Local failure writes NONE effects and preserves item results. Native call writes
 * all results; absent data is error/UNKNOWN. Aggregate errors survive item success.
 * OUT_OF_DATE/SURFACE_LOST still enqueue queue operations; native call-wide OOM
 * changes no resource/synchronization state. UNKNOWN is not permission to reuse.
 * No waits/null fence => its effect is NONE. ENQUEUED is not completion/display.
 * A signaled Vulkan present fence retires native payloads/shared batch waits, not
 * scanout or image reacquisition. Retire independent render/acquire uses separately.
 * abandon accepts nonempty unique acquired frames from one chain, after acquisition
 * and GPU uses complete. No wait, layout change or binary-signal consumption. Local
 * rejection retains frames; success releases; unclassified native failure is UNKNOWN.
 * The caller owns all lifetime/pending-use accounting; no acquisition registry.
 */
enum {
    OGPU_NEXT_PRESENT_VULKAN = 1,
    OGPU_NEXT_PRESENT_DXGI = 2,
    OGPU_NEXT_PRESENTATION_WIN32 = 2,
    OGPU_NEXT_WIN32_SURFACE_DESC = 3000,
    OGPU_NEXT_DXGI_CHAIN_DESC = 3001
};
enum {
    OGPU_NEXT_OCCLUDED = 5,
    OGPU_NEXT_STILL_DRAWING = 6
};
enum {
    OGPU_NEXT_AVAILABILITY_SIGNALS = 1,
    OGPU_NEXT_AVAILABILITY_QUEUE_ORDERED = 2
};
enum {
    OGPU_NEXT_PRESENT_REPLACE = 1,
    OGPU_NEXT_PRESENT_RESIZE = 2,
    OGPU_NEXT_PRESENT_RETIRE_FENCE = 4,
    OGPU_NEXT_PRESENT_PACING_HANDLE = 8,
    OGPU_NEXT_PRESENT_BINARY_WAIT = 16
};
enum {
    OGPU_NEXT_CHAIN_NONE = 0,
    OGPU_NEXT_CHAIN_ACTIVE = 1,
    OGPU_NEXT_CHAIN_RETIRED = 2,
    OGPU_NEXT_CHAIN_NO_IMAGES = 3,
    OGPU_NEXT_CHAIN_LOST = 4
};
enum {
    OGPU_NEXT_IMAGES_UNCHANGED = 0,
    OGPU_NEXT_IMAGES_REPLACED = 1,
    OGPU_NEXT_IMAGES_INVALIDATED = 2,
    OGPU_NEXT_FRAME_RETAINED = 0,
    OGPU_NEXT_FRAME_RELEASED = 1,
    OGPU_NEXT_FRAME_UNKNOWN = 2,
    OGPU_NEXT_EFFECT_NONE = 0,
    OGPU_NEXT_EFFECT_ENQUEUED = 1,
    OGPU_NEXT_EFFECT_UNKNOWN = 2,
    OGPU_NEXT_WAITABLE_WIN32 = 1
};

typedef struct ogpu_next_win32_surface_desc {
    ogpu_next_record header;
    void *hwnd;
} ogpu_next_win32_surface_desc;

/* Native option numbers belong to DXGI, never to the Vulkan mode namespace.
 * Exact selection: no default queue, image count, mode or latency policy.
 */
typedef struct ogpu_next_dxgi_chain_desc {
    ogpu_next_record header;
    ogpu_next_surface *surface;
    ogpu_next_queue *queue;
    uint32_t format, extent[2], image_count;
    uint64_t usage;
    uint32_t swap_effect, scaling, alpha, color_space;
    uint32_t chain_flags, maximum_frame_latency;
} ogpu_next_dxgi_chain_desc;

typedef struct ogpu_next_presentation_contract {
    uint32_t profile, availability, capabilities, max_present_count;
} ogpu_next_presentation_contract;

typedef struct ogpu_next_chain_info {
    uint64_t generation;
    uint32_t state, image_count;
    ogpu_next_queue *ordered_queue; /* non-NULL only for QUEUE_ORDERED */
} ogpu_next_chain_info;

typedef struct ogpu_next_chain_create_outcome {
    ogpu_next_swapchain *chain; /* NULL on failure */
    uint32_t previous_state, reserved;
} ogpu_next_chain_create_outcome;

typedef struct ogpu_next_chain_resize_desc {
    uint32_t format, extent[2], image_count;
    uint32_t chain_flags, reserved;
} ogpu_next_chain_resize_desc;

typedef struct ogpu_next_chain_resize_outcome {
    uint64_t generation;
    uint32_t state, image_effect;
} ogpu_next_chain_resize_outcome;

typedef struct ogpu_next_acquire_desc {
    uint64_t timeout_ns;
    ogpu_next_binary *binary;
    ogpu_next_fence *fence;
} ogpu_next_acquire_desc;

/* Borrowed value, no allocation or reference count; copying confers no new lease.
 * Availability objects are caller-provided, never invented by acquisition.
 */
typedef struct ogpu_next_frame {
    ogpu_next_swapchain *chain;
    ogpu_next_image *image;
    uint64_t generation;
    uint32_t index, availability;
    ogpu_next_queue *ordered_queue;
    ogpu_next_binary *binary;
    ogpu_next_fence *fence;
} ogpu_next_frame;

typedef struct ogpu_next_present_item {
    ogpu_next_frame frame;
    ogpu_next_fence *retire_fence;
    uint32_t sync_interval, native_flags; /* DXGI only; zero on Vulkan */
} ogpu_next_present_item;

typedef struct ogpu_next_present_result {
    ogpu_next_status status;
    uint32_t frame_effect, fence_effect, reserved;
} ogpu_next_present_result;

typedef struct ogpu_next_present_desc {
    uint32_t wait_count, item_count;
    ogpu_next_binary *const *waits;
    const ogpu_next_present_item *items;
    ogpu_next_present_result *results; /* required, item_count entries */
    ogpu_next_host_span scratch;
} ogpu_next_present_desc;

typedef struct ogpu_next_present_outcome {
    uint32_t work_effect, wait_effect;
} ogpu_next_present_outcome;

typedef struct ogpu_next_pacing_handle {
    uint32_t kind, reserved;
    void *handle; /* borrowed; chain closes it, caller must not */
} ogpu_next_pacing_handle;

typedef struct ogpu_next_presentation_functions {
    ogpu_next_status (*discovery_create)(uint32_t platforms, ogpu_next_discovery **);
    ogpu_next_status (*adapter_support)(const ogpu_next_adapter *, ogpu_next_presentation_support *);
    ogpu_next_status (*surface_wayland_create)(const ogpu_next_adapter *, const ogpu_next_wayland_surface_desc *, ogpu_next_surface **);
    void (*surface_destroy)(ogpu_next_surface *);
    ogpu_next_status (*surface_query)(const ogpu_next_adapter *, const ogpu_next_surface *, ogpu_next_query *);
    ogpu_next_status (*surface_mode_query)(const ogpu_next_adapter *, const ogpu_next_surface *, uint32_t mode, ogpu_next_query *);
    ogpu_next_status (*binary_create)(ogpu_next_device *, ogpu_next_binary **);
    void (*binary_destroy)(ogpu_next_binary *);
    ogpu_next_status (*fence_create)(ogpu_next_device *, uint32_t signaled, ogpu_next_fence **);
    void (*fence_destroy)(ogpu_next_fence *);
    ogpu_next_status (*fence_poll)(const ogpu_next_fence *);
    ogpu_next_status (*fence_wait)(const ogpu_next_fence *, uint64_t timeout_ns);
    ogpu_next_status (*fence_reset)(ogpu_next_fence *);
    ogpu_next_status (*surface_win32_create)(const ogpu_next_adapter *, const ogpu_next_win32_surface_desc *, ogpu_next_surface **);
    ogpu_next_status (*presentation_contract_query)(const ogpu_next_adapter *, const ogpu_next_surface *, uint32_t profile, ogpu_next_presentation_contract *);

    ogpu_next_status (*chain_create)(ogpu_next_device *, const ogpu_next_record *, ogpu_next_chain_create_outcome *);
    void (*chain_destroy)(ogpu_next_swapchain *);
    void (*chain_info_query)(const ogpu_next_swapchain *, ogpu_next_chain_info *);

    ogpu_next_status (*chain_images)(const ogpu_next_swapchain *, uint32_t capacity, ogpu_next_image **, uint32_t *count);
    ogpu_next_status (*chain_resize)(ogpu_next_swapchain *, const ogpu_next_chain_resize_desc *, ogpu_next_chain_resize_outcome *);
    ogpu_next_status (*chain_pacing_handle)(const ogpu_next_swapchain *, ogpu_next_pacing_handle *);
    ogpu_next_status (*acquire)(ogpu_next_swapchain *, const ogpu_next_acquire_desc *, ogpu_next_frame *);
    ogpu_next_status (*abandon_scratch_requirements)(const ogpu_next_device *, uint32_t count, ogpu_next_host_requirements *);
    ogpu_next_status (*abandon)(ogpu_next_swapchain *, uint32_t count, const ogpu_next_frame *, ogpu_next_host_span scratch, uint32_t *frame_effect);
    ogpu_next_status (*present_scratch_requirements)(const ogpu_next_device *, uint32_t waits, uint32_t items, ogpu_next_host_requirements *);
    ogpu_next_status (*queue_present)(ogpu_next_queue *, const ogpu_next_present_desc *, ogpu_next_present_outcome *);
} ogpu_next_presentation_functions;
ogpu_next_status ogpu_next_presentation_api(uint32_t version, uint32_t byte_size, ogpu_next_presentation_functions *);
#ifdef __cplusplus
}
#endif
#endif
