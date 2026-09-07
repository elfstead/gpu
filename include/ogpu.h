#ifndef OGPU_H
#define OGPU_H

#include <stdint.h>

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

#ifdef __cplusplus
}
#endif
#endif
