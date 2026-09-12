//! Experimental C ABI. Ownership and pointer requirements are defined in include/ogpu.h.
#![deny(unsafe_op_in_unsafe_fn)]

mod compute;
mod execution_api;
pub use execution_api::*;
mod vulkan;

use std::{
    ffi::c_char,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
    sync::Arc,
};

pub type OgpuResult = i32;
pub const ABI_VERSION: u32 = 2;
pub const SUCCESS: OgpuResult = 0;
pub const INVALID_ARGUMENT: OgpuResult = -1;
pub const ABI_MISMATCH: OgpuResult = -2;
pub const LOADER_ERROR: OgpuResult = -3;
pub const UNSUPPORTED: OgpuResult = -4;
pub const VULKAN_ERROR: OgpuResult = -5;
pub const OUT_OF_RANGE: OgpuResult = -6;
pub const INTERNAL_ERROR: OgpuResult = -7;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OgpuError {
    pub vulkan_result: i32,
    pub message: [c_char; 256],
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct OgpuCapabilities {
    pub graphics_queue: u32,
    pub compute_queue: u32,
    pub buffer_device_address: u32,
    pub timeline_semaphore: u32,
    pub synchronization2: u32,
    pub descriptor_heap: u32,
    pub device_address_commands: u32,
    pub shader_untyped_pointers: u32,
    pub cooperative_matrix: u32,
    pub storage_buffer_8bit_access: u32,
    pub storage_buffer_16bit_access: u32,
    pub shader_float16: u32,
    pub shader_int8: u32,
    pub shader_int16: u32,
    pub shader_int64: u32,
    pub shader_float64: u32,
    pub shader_bfloat16: u32,
    pub shader_bfloat16_cooperative_matrix: u32,
    pub shader_float8: u32,
    pub shader_float8_cooperative_matrix: u32,
    pub shader_float4: u32,
    pub shader_float6: u32,
    pub shader_float8_unsigned_e8m0: u32,
    pub shader_mx_int8: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OgpuDeviceInfo {
    pub name: [c_char; 256],
    pub vendor_id: u32,
    pub device_id: u32,
    pub device_type: u32,
    pub vulkan_api_major: u32,
    pub vulkan_api_minor: u32,
    pub vulkan_api_patch: u32,
    pub capabilities: OgpuCapabilities,
}

/// Opaque C ownership handle. Device information is immutable after construction.
pub struct OgpuProbe {
    devices: Vec<OgpuDeviceInfo>,
    physical_devices: Vec<ogpu_vulkan_sys::VkPhysicalDevice>,
    // Keeps the instance and its dynamic library alive until the probe is destroyed.
    _vulkan: Arc<vulkan::Instance>,
}

#[derive(Debug)]
struct Error {
    status: OgpuResult,
    vk: i32,
    message: String,
}

impl Error {
    fn new(status: OgpuResult, message: impl Into<String>) -> Self {
        Self {
            status,
            vk: 0,
            message: message.into(),
        }
    }
    fn vulkan(operation: &str, vk: i32) -> Self {
        Self {
            status: VULKAN_ERROR,
            vk,
            message: format!("{operation}: VkResult {vk}"),
        }
    }
    fn diagnostic(&self) -> OgpuError {
        let mut result = OgpuError {
            vulkan_result: self.vk,
            message: [0; 256],
        };
        // Don't cut a UTF-8 character in half. Embedded NULs terminate a C message.
        let mut end = self.message.len().min(255);
        while !self.message.is_char_boundary(end) {
            end -= 1;
        }
        for (destination, source) in result
            .message
            .iter_mut()
            .zip(self.message.as_bytes()[..end].iter())
        {
            *destination = *source as c_char;
        }
        result
    }
}

fn boundary(f: impl FnOnce() -> OgpuResult) -> OgpuResult {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(INTERNAL_ERROR)
}

/// # Safety
/// See `ogpu_probe_create` in include/ogpu.h. Outputs must be valid and non-overlapping.
#[no_mangle]
pub unsafe extern "C" fn ogpu_probe_create(
    version: u32,
    out_probe: *mut *mut OgpuProbe,
    out_error: *mut OgpuError,
) -> OgpuResult {
    if out_probe.is_null() {
        return INVALID_ARGUMENT;
    }
    // SAFETY: the caller supplies a writable pointer-sized output.
    unsafe {
        out_probe.write(ptr::null_mut());
    }
    // Do not write a potentially different version's error layout.
    if version != ABI_VERSION {
        return ABI_MISMATCH;
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        let instance = Arc::new(vulkan::Instance::new()?);
        let physical_devices = instance.physical_devices()?;
        let devices = physical_devices
            .iter()
            .map(|&device| instance.device_info(device))
            .collect::<Result<_, _>>()?;
        Ok::<_, Error>(Box::new(OgpuProbe {
            devices,
            physical_devices,
            _vulkan: instance,
        }))
    }));
    let result = result.unwrap_or_else(|_| {
        Err(Error::new(
            INTERNAL_ERROR,
            "Rust panic contained at C boundary",
        ))
    });
    match result {
        Ok(probe) => {
            // SAFETY: outputs are valid by the C contract. Ownership transfers to C.
            unsafe {
                out_probe.write(Box::into_raw(probe));
                if !out_error.is_null() {
                    out_error.write(OgpuError {
                        vulkan_result: 0,
                        message: [0; 256],
                    });
                }
            }
            SUCCESS
        }
        Err(error) => {
            // SAFETY: an optional non-NULL diagnostic points to a writable OgpuError.
            if !out_error.is_null() {
                unsafe {
                    out_error.write(error.diagnostic());
                }
            }
            error.status
        }
    }
}

/// # Safety
/// `probe` must be NULL or a live handle returned by create, with no concurrent users.
#[no_mangle]
pub unsafe extern "C" fn ogpu_probe_destroy(probe: *mut OgpuProbe) {
    if !probe.is_null() {
        // SAFETY: the caller returns unique ownership exactly once. Drop only calls Vulkan
        // destruction and releases Rust allocations; it has no panicking Rust operations.
        unsafe {
            drop(Box::from_raw(probe));
        }
    }
}

/// # Safety
/// See include/ogpu.h: a live probe and writable, non-overlapping output are required.
#[no_mangle]
pub unsafe extern "C" fn ogpu_probe_device_count(
    probe: *const OgpuProbe,
    out_count: *mut u32,
) -> OgpuResult {
    boundary(|| {
        if probe.is_null() || out_count.is_null() {
            return INVALID_ARGUMENT;
        }
        // SAFETY: the probe remains live and immutable for this call; output is writable.
        unsafe {
            out_count.write((*probe).devices.len() as u32);
        }
        SUCCESS
    })
}

/// # Safety
/// See include/ogpu.h: a live probe and writable, non-overlapping output are required.
#[no_mangle]
pub unsafe extern "C" fn ogpu_probe_device_info(
    probe: *const OgpuProbe,
    index: u32,
    out_info: *mut OgpuDeviceInfo,
) -> OgpuResult {
    boundary(|| {
        if probe.is_null() || out_info.is_null() {
            return INVALID_ARGUMENT;
        }
        // SAFETY: the probe remains live and immutable, and the output is writable.
        unsafe {
            let probe = &*probe;
            let Some(info) = probe.devices.get(index as usize) else {
                return OUT_OF_RANGE;
            };
            out_info.write(*info);
        }
        SUCCESS
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_arguments_and_version_do_not_load_vulkan() {
        unsafe {
            let mut handle = ptr::dangling_mut::<OgpuProbe>();
            let mut error = OgpuError {
                vulkan_result: 42,
                message: [1; 256],
            };
            assert_eq!(
                ogpu_probe_create(ABI_VERSION + 1, &mut handle, &mut error),
                ABI_MISMATCH
            );
            assert!(handle.is_null());
            assert_eq!(error.vulkan_result, 42);
            assert_eq!(error.message, [1; 256]);
            assert_eq!(
                ogpu_probe_create(ABI_VERSION, ptr::null_mut(), ptr::null_mut()),
                INVALID_ARGUMENT
            );
            let mut count = 42;
            assert_eq!(
                ogpu_probe_device_count(ptr::null(), &mut count),
                INVALID_ARGUMENT
            );
            assert_eq!(count, 42);
            assert_eq!(
                ogpu_probe_device_info(ptr::null(), 0, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            ogpu_probe_destroy(ptr::null_mut());
        }
    }

    #[test]
    fn panic_is_contained() {
        assert_eq!(boundary(|| panic!("test panic")), INTERNAL_ERROR);
    }

    #[test]
    fn diagnostic_is_terminated_and_preserves_utf8() {
        let error = Error::new(LOADER_ERROR, "é".repeat(200)).diagnostic();
        assert_eq!(error.message[254], 0);
        assert_eq!(error.message[255], 0);
        let bytes: Vec<_> = error
            .message
            .iter()
            .take_while(|&&v| v != 0)
            .map(|&v| v as u8)
            .collect();
        assert!(std::str::from_utf8(&bytes).is_ok());
    }
}
