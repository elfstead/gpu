#ifndef OGPU_H
#define OGPU_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Experimental ABI. Any layout/signature change must increment this version. */
#define OGPU_ABI_VERSION UINT32_C(6)

typedef int32_t OgpuResult;
#define OGPU_SUCCESS INT32_C(0)
#define OGPU_ERROR_INVALID_ARGUMENT INT32_C(-1)
#define OGPU_ERROR_ABI_MISMATCH INT32_C(-2)
#define OGPU_ERROR_LOADER INT32_C(-3)
#define OGPU_ERROR_UNSUPPORTED INT32_C(-4)
#define OGPU_ERROR_VULKAN INT32_C(-5)
#define OGPU_ERROR_OUT_OF_RANGE INT32_C(-6)
#define OGPU_ERROR_INTERNAL INT32_C(-7)

typedef struct OgpuProbe OgpuProbe;

typedef struct OgpuError {
    int32_t vulkan_result; /* Zero unless a Vulkan call returned an error. */
    char message[256];    /* Always NUL-terminated, possibly truncated. */
} OgpuError;

/* Every field is exactly 0 or 1. Supported does not mean enabled.
 * Matrix flags do NOT guarantee particular shapes/types, speed, or compiler support.
 * Numeric shader types and storage access are deliberately separate capabilities. */
typedef struct OgpuCapabilities {
    uint32_t graphics_queue;
    uint32_t compute_queue;
    uint32_t buffer_device_address;
    uint32_t timeline_semaphore;
    uint32_t synchronization2;
    uint32_t descriptor_heap;
    uint32_t device_address_commands;
    uint32_t shader_untyped_pointers;
    uint32_t cooperative_matrix;
    uint32_t storage_buffer_8bit_access;
    uint32_t storage_buffer_16bit_access;
    uint32_t shader_float16;
    uint32_t shader_int8;
    uint32_t shader_int16;
    uint32_t shader_int64;
    uint32_t shader_float64;
    uint32_t shader_bfloat16;
    uint32_t shader_bfloat16_cooperative_matrix;
    uint32_t shader_float8;
    uint32_t shader_float8_cooperative_matrix;
    uint32_t shader_float4;
    uint32_t shader_float6;
    uint32_t shader_float8_unsigned_e8m0;
    uint32_t shader_mx_int8;
} OgpuCapabilities;

/* Device kinds are our values, not Vulkan enum values. */
#define OGPU_DEVICE_OTHER UINT32_C(0)
#define OGPU_DEVICE_INTEGRATED UINT32_C(1)
#define OGPU_DEVICE_DISCRETE UINT32_C(2)
#define OGPU_DEVICE_VIRTUAL UINT32_C(3)
#define OGPU_DEVICE_CPU UINT32_C(4)

typedef struct OgpuDeviceInfo {
    char name[256];
    uint32_t vendor_id;
    uint32_t device_id;
    uint32_t device_type;
    uint32_t vulkan_api_major;
    uint32_t vulkan_api_minor;
    uint32_t vulkan_api_patch;
    OgpuCapabilities capabilities;
} OgpuDeviceInfo;

/* Creates an immutable snapshot; no logical GPU device or GPU work is created.
 * Requires a Vulkan 1.1+ loader; optional device features are never required.
 * out_probe is required and set to NULL on failure. out_error is optional and
 * cleared on success. Zero devices is a successful empty snapshot.
 * Pass OGPU_ABI_VERSION: a mismatch is rejected before writing versioned structs.
 * Non-NULL pointers must be aligned, valid, and writable for their entire objects;
 * outputs must not overlap. Rust panics are contained; allocation failure may abort. */
OgpuResult ogpu_probe_create(uint32_t abi_version, OgpuProbe **out_probe, OgpuError *out_error);

/* NULL is a no-op. Otherwise destroys exactly once. No other calls may be in flight. */
void ogpu_probe_destroy(OgpuProbe *probe);

/* Queries may run concurrently on one live probe. Outputs are untouched on error.
 * All pointers are required. Device indices are stable until probe destruction.
 * No output contains borrowed pointers: copied information outlives the probe. */
OgpuResult ogpu_probe_device_count(const OgpuProbe *probe, uint32_t *out_count);
OgpuResult ogpu_probe_device_info(const OgpuProbe *probe, uint32_t index, OgpuDeviceInfo *out_info);

/* Experimental execution slice: Linux x86-64, Vulkan 1.4 + compute queue;
 * requires BDA, timelineSemaphore, synchronization2, maintenance5,
 * VK_EXT_descriptor_heap, VK_KHR_device_address_commands and
 * VK_KHR_shader_untyped_pointers (including their feature bits).
 * Discovery remains independent: unsupported execution devices are still listed.
 * These opaque objects have independent ownership. Buffers/kernels retain their
 * device; a device retains its instance. Batches/completions retain recorded kernels.
 * Destroying a probe or device handle does
 * not invalidate its surviving children. Each handle must still be destroyed once.
 *
 * All operations on ONE device and ALL its children must be externally serialized,
 * including destruction. Independent devices may operate concurrently. The same
 * pointer-validity, non-overlap, and panic/OOM rules as the probe API apply.
 * out_error is optional, written on failure and cleared on success. Creation
 * outputs are required and set to NULL on failure. Destruction accepts NULL. */
typedef struct OgpuDevice OgpuDevice;
typedef struct OgpuBuffer OgpuBuffer;
typedef struct OgpuKernel OgpuKernel;

OgpuResult ogpu_device_create(const OgpuProbe *probe, uint32_t index, OgpuDevice **out_device, OgpuError *out_error);
void ogpu_device_destroy(OgpuDevice *device);

/* Optional profile: requires one queue family supporting BOTH graphics and compute.
 * Other device creation/ownership rules match ogpu_device_create; UNSUPPORTED if
 * no such family exists. Ordinary creation still accepts compute-only devices. */
OgpuResult ogpu_device_create_graphics(const OgpuProbe *probe, uint32_t index,
    OgpuDevice **out_device, OgpuError *out_error);

/* Dedicated allocation with explicit placement (unknown values INVALID_ARGUMENT).
 * HOST permits CPU read/write; DEVICE requires device-local memory and rejects
 * CPU read/write (even zero bytes) with INVALID_ARGUMENT. Use recorded copies for
 * staging. No silent placement fallback; UNSUPPORTED if no compatible memory type.
 * HOST may also be device-local on UMA/BAR hardware; these are access contracts,
 * not a promise of separate physical heaps. The runtime selects the memory type.
 * size_bytes must be nonzero and <= INTPTR_MAX. Contents start unspecified.
 * buffer_write/read are checked CPU copies, not implicit GPU commands.
 * Wait for ALL submitted uses of this entire buffer before CPU reading/writing
 * (cache maintenance may touch the whole allocation). Destroying the public handle
 * releases its ownership; commands explicitly retaining a buffer delay deallocation.
 * Otherwise establish completion before destruction; addresses alone do not retain allocations.
 * Zero-length transfers allow NULL data and offset == size_bytes. Read destinations
 * are unchanged on error. No persistent host mapping is exposed. */
#define OGPU_MEMORY_HOST UINT32_C(0)
#define OGPU_MEMORY_DEVICE UINT32_C(1)
OgpuResult ogpu_buffer_create(OgpuDevice *device, uint64_t size_bytes, uint32_t placement,
    OgpuBuffer **out_buffer, OgpuError *out_error);
void ogpu_buffer_destroy(OgpuBuffer *buffer);
OgpuResult ogpu_buffer_write(OgpuBuffer *buffer, uint64_t offset, const void *data, uint64_t size_bytes, OgpuError *out_error);
OgpuResult ogpu_buffer_read(const OgpuBuffer *buffer, uint64_t offset, void *data, uint64_t size_bytes, OgpuError *out_error);

/* Returns a NON-OWNING GPU address, valid only on this buffer's device until its
 * underlying allocation is freed or the device is lost. Never dereference it on
 * the CPU. Output unchanged on error. Keep ownership (a public handle or documented
 * command retention) for every GPU operation that can reach this address. */
OgpuResult ogpu_buffer_device_address(const OgpuBuffer *buffer, uint64_t *out_address, OgpuError *out_error);

/* words is a 4-byte-aligned SPIR-V module, copied/consumed before return. The caller
 * must provide VALID SPIR-V for the enabled modern Vulkan baseline with a compute
 * entry named "main" and no descriptor-set bindings. Core capabilities, BDA,
 * untyped pointers and native descriptor-heap access are supported. Heap shaders
 * require bound image/sampler heaps with matching descriptor kinds, formats and valid indices.
 * Legacy descriptor-free Vulkan 1.2-targeted modules remain valid inputs.
 * push_size_bytes must be a multiple of 4 within maxPushDataSize; zero is legal.
 * All shader push accesses must fit this range. Header checks are NOT validation
 * or sandboxing; malformed/incompatible shaders may cause driver faults. */
OgpuResult ogpu_kernel_create(OgpuDevice *device, const uint32_t *words, uint64_t word_count,
    uint32_t push_size_bytes, OgpuKernel **out_kernel, OgpuError *out_error);
void ogpu_kernel_destroy(OgpuKernel *kernel);

/* Dispatch a grid of groups_x × groups_y × groups_z workgroups. The shader defines
 * local size; these counts are NOT invocation counts. Set unused axes to 1.
 * This convenience call has no heap bindings; use a batch for heap shaders.
 * Each count must be nonzero and within its per-axis device limit. Argument byte
 * count must exactly match the kernel's push size (NULL allowed only for zero bytes).
 * The caller defines the argument layout, including initialized padding bytes.
 * Arguments are copied into the command buffer. Every referenced GPU allocation
 * must be live, on this kernel's device, and accessed in bounds with correct
 * alignment and race-free shader behavior. The runtime cannot inspect pointers
 * embedded in arbitrary arguments to enforce these requirements.
 *
 * This call blocks until completion or device loss, with no timeout. Host writes
 * from buffer_write are visible to the shader; completed shader writes are visible
 * to subsequent buffer_read or dispatch calls. Even on error, submitted work is
 * drained/lost before return so resources can be destroyed. Non-loss wait errors
 * are retried until draining is established; persistent failures may block forever.
 * After device loss, only destruction and draining existing completions are supported.
 * This ordered convenience call uses a batch and completion internally. */
OgpuResult ogpu_dispatch_wait(OgpuKernel *kernel, uint32_t groups_x, uint32_t groups_y,
    uint32_t groups_z, const void *arguments, uint32_t argument_bytes, OgpuError *out_error);

/* One-shot asynchronous recordings on the device's single queue. All calls remain
 * externally serialized with this device and its children; GPU execution may run
 * between calls. These access bits are ours, not Vulkan flags. Graphics access bits
 * require a graphics-capable execution queue. */
typedef struct OgpuBatch OgpuBatch;
typedef struct OgpuCompletion OgpuCompletion;
#define OGPU_ACCESS_COMPUTE_READ UINT32_C(1)
#define OGPU_ACCESS_COMPUTE_WRITE UINT32_C(2)
#define OGPU_ACCESS_VERTEX_READ UINT32_C(4)
#define OGPU_ACCESS_INDIRECT_READ UINT32_C(8)
#define OGPU_ACCESS_COLOR_WRITE UINT32_C(16)
#define OGPU_ACCESS_TRANSFER_READ UINT32_C(32)
#define OGPU_ACCESS_TRANSFER_WRITE UINT32_C(64)
#define OGPU_ACCESS_FRAGMENT_READ UINT32_C(128)

OgpuResult ogpu_batch_create(OgpuDevice *device, OgpuBatch **out_batch, OgpuError *out_error);
/* Discards an unsubmitted recording; never waits. NULL is a no-op. */
void ogpu_batch_destroy(OgpuBatch *batch);

/* Copies arguments during recording and retains the kernel, which must belong to
 * the batch's device. Other argument/grid/shader rules match dispatch_wait.
 * Allocation addresses are NOT retained: keep all reachable allocations alive from
 * recording through completion, or until the unsubmitted recording is discarded.
 * Invalid recording arguments leave the recording unchanged. */
OgpuResult ogpu_batch_dispatch(OgpuBatch *batch, OgpuKernel *kernel, uint32_t groups_x,
    uint32_t groups_y, uint32_t groups_z, const void *arguments,
    uint32_t argument_bytes, OgpuError *out_error);

/* Global dependency from earlier to later commands on this queue, including
 * earlier submissions. Each mask must be a nonzero combination of the access bits
 * above. No dependency is inferred from GPU pointers or dispatch order. */
OgpuResult ogpu_batch_barrier(OgpuBatch *batch, uint32_t source_access,
    uint32_t destination_access, OgpuError *out_error);

/* Records a byte-granular GPU copy between in-bounds ranges on this batch's device.
 * Both buffers are retained until discard, failed submission cleanup or completion
 * destruction. Either placement is legal. Same-buffer ranges must not overlap;
 * this is not memmove. Zero size is a validated no-op (end offsets legal), retaining
 * nothing. Invalid arguments leave recording unchanged. No implicit GPU dependency:
 * order producers/consumers with TRANSFER_READ/WRITE and the existing barriers.
 * CPU access to HOST buffers still requires completion of ALL their submitted uses. */
OgpuResult ogpu_batch_copy_buffer(OgpuBatch *batch, const OgpuBuffer *source,
    uint64_t source_offset, const OgpuBuffer *destination, uint64_t destination_offset,
    uint64_t size_bytes, OgpuError *out_error);

/* Attempts submission once; success means accepted, NOT completed. Empty batches
 * are legal. After an attempt (even preparation/submission failure), the batch is
 * terminal: only destruction is valid. Invalid required pointers do not consume it.
 * out_completion is required, NULL on failure. On success the completion retains
 * command resources and kernels; the original batch/kernel/device handles may be
 * destroyed. Referenced buffers must still outlive GPU access.
 * Host writes before submission become visible to GPU commands, and completed GPU
 * writes to subsequent host reads. GPU-to-GPU dependencies remain explicit.
 * Failed submission does not establish completion of other outstanding work. */
OgpuResult ogpu_batch_submit(OgpuBatch *batch, OgpuCompletion **out_completion, OgpuError *out_error);

/* Waits for this submission, not queue idle. No timeout. Repeated waits preserve
 * the wait outcome. Non-loss wait errors are reported only AFTER draining; device
 * loss also permits cleanup. Persistent wait failures can block indefinitely.
 * Completion does not imply correctness of the shader or its output. */
OgpuResult ogpu_completion_wait(OgpuCompletion *completion, OgpuError *out_error);
/* Does not wait for GPU progress. SUCCESS writes 0 (pending) or 1 (complete) to
 * required out_complete. Errors write 0; a transient error leaves resources pending
 * and can be retried. Device loss is an error, never successful completion.
 * SUCCESS + 1 establishes the same completion/visibility guarantee as wait and
 * permits timing retrieval. It grants no permission over later uses of an allocation.
 * Recorded wait errors remain errors on subsequent polls. No resources are released
 * until completion destruction. Same external serialization rules as wait. */
OgpuResult ogpu_completion_poll(OgpuCompletion *completion, uint32_t *out_complete,
    OgpuError *out_error);
/* Optional whole-allocation ownership assistance for addresses in shader roots.
 * Retains buffer (same device) until batch discard, failed submission cleanup, or
 * completion-handle destruction (which drains pending work). Duplicates are harmless.
 * The public buffer handle may then be destroyed; its address remains backed.
 * This declares NO access, adds NO barrier, and does NOT prevent an allocator from
 * reusing a range too early. Declare every reachable allocation or own it elsewhere.
 * Only valid while batch is recording. It does not retain pointer-reachable buffers. */
OgpuResult ogpu_batch_retain_buffer(OgpuBatch *batch, const OgpuBuffer *buffer,
    OgpuError *out_error);
/* Waits if pending, then destroys; does NOT cancel. Explicitly wait first for error
 * diagnostics. Keep referenced allocations alive, either through retained ownership
 * or their public handles, until this returns. NULL is a no-op. */
void ogpu_completion_destroy(OgpuCompletion *completion);

/* Optional timing of the selected execution queue; not a new requirement for
 * ordinary execution. Existing structures/signatures remain unchanged. */
typedef struct OgpuTimingInfo {
    double timestamp_period_ns;
    uint32_t timestamp_valid_bits;
    uint32_t reserved; /* Always zero. */
} OgpuTimingInfo;
/* UNSUPPORTED when this queue has no usable clock. Required output, zero on error. */
OgpuResult ogpu_device_timing_info(OgpuDevice *device, OgpuTimingInfo *out_info, OgpuError *out_error);
/* Opt a recording batch into whole-batch timing. Idempotent, empty batches legal.
 * UNSUPPORTED leaves the batch unchanged; after any submission attempt, invalid. */
OgpuResult ogpu_batch_enable_timing(OgpuBatch *batch, OgpuError *out_error);
/* Requires a timed completion and SUCCESS from completion_wait or poll with 1.
 * Does not poll or wait. Untimed/unconfirmed reads are INVALID_ARGUMENT. Required
 * output is zero on error. Successful reads are cached; query failures can be
 * retried unless device-lost, and do not change the previous wait outcome.
 * Approximate device-side batch duration, including barriers and scheduling;
 * not a CPU timestamp, isolated shader time, or a memory dependency. Caller must
 * keep intervals below 2^timestamp_valid_bits * timestamp_period_ns: subtraction
 * handles a counter-boundary crossing, not additional full wraps. Zero is valid.
 * Conversion to double may lose low-bit precision for large tick differences. */
OgpuResult ogpu_completion_elapsed_ns(OgpuCompletion *completion, double *out_nanoseconds,
    OgpuError *out_error);

/* Narrow offscreen graphics profile; all existing pointer/error/serialization rules
 * apply. Requires dynamicRendering and a shared graphics/compute queue.
 * Uses GENERAL layouts; VK_KHR_unified_image_layouts is enabled when supported
 * for its layout-efficiency guarantee, but is not required. Ownership rules
 * apply. Both objects retain their device. Images use specialized storage, NOT
 * addressable allocations. No window, presentation, depth, or blending. */
typedef struct OgpuImage OgpuImage;
typedef struct OgpuRaster OgpuRaster;

/* Copied description, one mip/layer/sample, no CPU mapping. 1D requires height=1.
 * Extents and usage must be nonzero; reserved=0. COLOR requires 2D RGBA8.
 * Device support is checked for the requested format/dimension/usage combination.
 * SAMPLED promises nearest and linear filtering. Operations require matching usage.
 * Initial contents/layout are undefined: discard or CLEAR before first use. */
#define OGPU_IMAGE_1D 1u
#define OGPU_IMAGE_2D 2u
#define OGPU_FORMAT_RGBA8_UNORM 0u
#define OGPU_FORMAT_R32_FLOAT 1u
#define OGPU_IMAGE_USAGE_SAMPLED 1u
#define OGPU_IMAGE_USAGE_STORAGE 2u
#define OGPU_IMAGE_USAGE_COLOR 4u
#define OGPU_IMAGE_USAGE_COPY_SRC 8u
#define OGPU_IMAGE_USAGE_COPY_DST 16u
typedef struct OgpuImageDesc {
    uint32_t dimension, width, height, format, usage, reserved;
} OgpuImageDesc;
OgpuResult ogpu_image_create(OgpuDevice *device, const OgpuImageDesc *desc,
    OgpuImage **out_target, OgpuError *out_error);
void ogpu_image_destroy(OgpuImage *target);

/* Independent owning image/sampler heaps, scoped to the graphics image profile.
 * Capacity is nonzero. Slots begin INVALID, not readable null descriptors. Indices
 * are separate uint32 namespaces relative to the bound heap, never ownership tokens.
 * Image entries retain their target until replaced/cleared/destroyed. Descriptions
 * are copied; no sampler/view handles or raw descriptor bytes escape. Every shader
 * access requires a written slot with matching kind/format and an initialized image.
 * No recursive resource tracing, implicit data dependencies, or index validation.
 * Mutation requires exclusive ownership: rejected while ANY recording/completion
 * retains the heap, including completed-but-live completions and earlier bindings
 * superseded by another bind. Destroy those references first; waiting is insufficient.
 * Validate/generate before commit: argument/descriptor-generation errors preserve
 * old entries. A flush failure after commit poisons the heap (only destruction then
 * succeeds). Device loss follows the device-wide terminal contract. */
typedef struct OgpuImageHeap OgpuImageHeap;
typedef struct OgpuSamplerHeap OgpuSamplerHeap;
#define OGPU_IMAGE_SAMPLED 0u
#define OGPU_IMAGE_STORAGE 1u
typedef struct OgpuImageEntry {
    const OgpuImage *image;
    uint32_t kind;
    uint32_t reserved;
} OgpuImageEntry;
OgpuResult ogpu_image_heap_create(OgpuDevice *device, uint32_t capacity,
    OgpuImageHeap **out_heap, OgpuError *out_error);
void ogpu_image_heap_destroy(OgpuImageHeap *heap);
/* first+count must fit; count zero allows NULL entries and first==capacity.
 * reserved must be zero; entries may repeat a target with different kinds. */
OgpuResult ogpu_image_heap_write(OgpuImageHeap *heap, uint32_t first,
    const OgpuImageEntry *entries, uint32_t count, OgpuError *out_error);
/* Clears ownership and invalidates slots; does NOT install readable null descriptors. */
OgpuResult ogpu_image_heap_clear(OgpuImageHeap *heap, uint32_t first,
    uint32_t count, OgpuError *out_error);
#define OGPU_FILTER_NEAREST 0u
#define OGPU_FILTER_LINEAR 1u
#define OGPU_ADDRESS_CLAMP 0u
#define OGPU_ADDRESS_REPEAT 1u
typedef struct OgpuSamplerDesc {
    uint32_t min_filter, mag_filter;
    uint32_t address_u, address_v;
} OgpuSamplerDesc;
/* Normalized coordinates, mip 0 only, no comparison/anisotropy; W clamps to edge.
 * Linear filtering support is checked when creating each sampled image. */
OgpuResult ogpu_sampler_heap_create(OgpuDevice *device, uint32_t capacity,
    OgpuSamplerHeap **out_heap, OgpuError *out_error);
void ogpu_sampler_heap_destroy(OgpuSamplerHeap *heap);
OgpuResult ogpu_sampler_heap_write(OgpuSamplerHeap *heap, uint32_t first,
    const OgpuSamplerDesc *entries, uint32_t count, OgpuError *out_error);
/* Recording-only; retains heap through discard/failed submit/completion destruction.
 * Affects subsequent draws/dispatches until replaced; no binding-layout compatibility.
 * Does NOT initialize images: each accessed target must first be cleared/drawn or
 * discarded in this or an earlier successfully submitted batch. Later uses preserve
 * GENERAL and contents until explicit clear/discard, with
 * explicit barriers for real dependencies. Do not access the active draw attachment
 * from a shader. Public heap/target handles can be released after retention.
 * Image and sampler bindings are independent; neither changes the other. */
OgpuResult ogpu_batch_bind_image_heap(OgpuBatch *batch, const OgpuImageHeap *heap,
    OgpuError *out_error);
OgpuResult ogpu_batch_bind_sampler_heap(OgpuBatch *batch, const OgpuSamplerHeap *heap,
    OgpuError *out_error);
/* Retains target and discards prior contents, ordering earlier uses and preparing
 * GENERAL for shader writes. This does not clear texels: write before reading them.
 * Prepares an image for an initial upload or compute writes. */
OgpuResult ogpu_batch_discard_image(OgpuBatch *batch, const OgpuImage *target,
    OgpuError *out_error);

/* Valid matching vertex/fragment SPIR-V main entries; same baseline/heap contract
 * as kernel_create. Storage reads only in these stages;
 * vertex/fragment stores/atomics are NOT enabled. Vertex positions must be written
 * by the vertex shader; fragment location 0 is a floating-point RGBA output.
 * Fixed triangle-list/fill/no-cull state, full-target viewport/scissor, one sample,
 * no depth/stencil or blending. Root range is shared by vertex AND fragment stages;
 * size/alignment/trusted-shader rules match kernel_create. No source compiler. */
OgpuResult ogpu_raster_create(OgpuDevice *device,
    const uint32_t *vertex_words, uint64_t vertex_word_count,
    const uint32_t *fragment_words, uint64_t fragment_word_count,
    uint32_t push_size_bytes, OgpuRaster **out_raster, OgpuError *out_error);
void ogpu_raster_destroy(OgpuRaster *raster);

/* GPU-readable draw record, four consecutive uint32 values. first_instance MUST
 * be zero (optional indirect-first-instance is not enabled). GPU-produced contents
 * are trusted, not inspected on the CPU; shader addresses must fit every invocation. */
typedef struct OgpuDrawArguments {
    uint32_t vertex_count;
    uint32_t instance_count;
    uint32_t first_vertex;
    uint32_t first_instance;
} OgpuDrawArguments;

#define OGPU_ATTACHMENT_CLEAR 0u
#define OGPU_ATTACHMENT_LOAD 1u
#define OGPU_ACCESS_COLOR_READ 256u
/* Execute ONE non-indexed indirect draw. CLEAR discards and clears to opaque black;
 * LOAD preserves prior contents and requires initialized, written texels plus an
 * explicit dependency to COLOR_READ | COLOR_WRITE. Both store final contents. All
 * objects must belong to the batch device. Indirect offset is 4-byte aligned and
 * a full 16-byte record must fit. Arguments are copied; their size must match raster.
 * Records retain raster, target, and indirect buffer, but NOT pointees embedded
 * in arguments. Explicit barriers must order compute-produced vertex/draw data.
 * Image operations manage image layouts and attachment/copy dependencies; every
 * CLEAR draw discards previous target contents. Public handles may be destroyed after
 * recording; retained resources are released only after discard/completion cleanup. */
OgpuResult ogpu_batch_draw_indirect(OgpuBatch *batch, OgpuRaster *raster,
    OgpuImage *target, OgpuBuffer *indirect, uint64_t indirect_offset,
    const void *arguments, uint32_t argument_bytes, uint32_t load, OgpuError *out_error);

/* Caller must initialize GENERAL and write every copied texel in this or an earlier
 * successfully submitted batch. No hidden initialization or host-side layout tracker.
 * Requires COPY_SRC usage; orders earlier GPU writes before readback.
 * Copies whole images as tightly packed rows preserving format bits (four bytes
 * per texel for both supported formats). Destination offset must be 4-byte
 * aligned; width*height*4 bytes must fit. Retains target and destination. Wait before
 * CPU access; explicit TRANSFER_WRITE dependencies precede subsequent GPU consumers.
 * Invalid draw/copy arguments leave the recording unchanged. Destruction is NULL-safe. */
OgpuResult ogpu_batch_copy_image_to_buffer(OgpuBatch *batch, OgpuImage *target,
    OgpuBuffer *destination, uint64_t destination_offset, OgpuError *out_error);

/* Requires COPY_DST and initialized GENERAL (discard_image before first use).
 * Copies the full image from a tightly packed, 4-byte-aligned, in-bounds buffer
 * range, without conversion. Orders prior GPU accesses before the copy; explicit
 * TRANSFER_WRITE dependencies precede later shader/attachment consumers.
 * Retains source and image. HOST or DEVICE buffers are legal. Finish host writes
 * before submission; do not modify/reuse the source until completion or discard
 * the unsubmitted batch. Invalid arguments leave the recording unchanged. */
OgpuResult ogpu_batch_copy_buffer_to_image(OgpuBatch *batch, OgpuBuffer *source,
    uint64_t source_offset, OgpuImage *image, OgpuError *out_error);

#ifdef __cplusplus
}
#endif
#endif
