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
    OGPU_NEXT_DEVICE_DESC = 100, OGPU_NEXT_MEMORY_DESC = 101
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
#ifdef __cplusplus
}
#endif
#endif
