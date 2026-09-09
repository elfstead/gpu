#ifndef OGPU_H
#define OGPU_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Experimental ABI. Any layout/signature change must increment this version. */
#define OGPU_ABI_VERSION UINT32_C(1)

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

/* Experimental execution slice: Linux x86-64, Vulkan 1.2 device + BDA + compute queue.
 * Discovery remains independent: unsupported execution devices are still listed.
 * These opaque objects have independent ownership. Buffers/kernels retain their
 * device; a device retains its instance. Destroying a probe or device handle does
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

/* Dedicated host-visible allocation. size_bytes must be nonzero and <= INTPTR_MAX.
 * Contents start unspecified. Transfers are checked CPU copies, not GPU commands.
 * Zero-length transfers allow NULL data and offset == size_bytes. Read destinations
 * are unchanged on error. No persistent host mapping is exposed. */
OgpuResult ogpu_buffer_create(OgpuDevice *device, uint64_t size_bytes, OgpuBuffer **out_buffer, OgpuError *out_error);
void ogpu_buffer_destroy(OgpuBuffer *buffer);
OgpuResult ogpu_buffer_write(OgpuBuffer *buffer, uint64_t offset, const void *data, uint64_t size_bytes, OgpuError *out_error);
OgpuResult ogpu_buffer_read(const OgpuBuffer *buffer, uint64_t offset, void *data, uint64_t size_bytes, OgpuError *out_error);

/* Returns a NON-OWNING GPU address, valid only on this buffer's device until buffer
 * destruction or device loss. Never dereference it on the CPU. Output unchanged
 * on error. Keep the buffer alive for every dispatch that can reach this address. */
OgpuResult ogpu_buffer_device_address(const OgpuBuffer *buffer, uint64_t *out_address, OgpuError *out_error);

/* words is a 4-byte-aligned SPIR-V module, copied/consumed before return. The caller
 * must provide VALID Vulkan 1.2 SPIR-V with a compute entry named "main", no
 * descriptors, and only core-required capabilities plus bufferDeviceAddress.
 * push_size_bytes must be a multiple of 4 within maxPushConstantsSize; zero is legal.
 * All shader push accesses must fit this range. Header checks are NOT validation
 * or sandboxing; malformed/incompatible shaders may cause driver faults. */
OgpuResult ogpu_kernel_create(OgpuDevice *device, const uint32_t *words, uint64_t word_count,
    uint32_t push_size_bytes, OgpuKernel **out_kernel, OgpuError *out_error);
void ogpu_kernel_destroy(OgpuKernel *kernel);

/* One-dimensional dispatch: groups_x workgroups; the shader defines local size.
 * groups_x must be nonzero and within the device limit. Argument byte count must
 * exactly match the kernel's push size (NULL allowed only for zero bytes).
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
 * After device loss, only destruction is supported. This is NOT an async API. */
OgpuResult ogpu_dispatch_wait(OgpuKernel *kernel, uint32_t groups_x, const void *arguments,
    uint32_t argument_bytes, OgpuError *out_error);

#ifdef __cplusplus
}
#endif
#endif
