//! C ownership boundary for synchronous dispatch and one-shot asynchronous batches.
use crate::{
    compute::{Batch, Buffer, Completion, Device, Kernel},
    Error, OgpuError, OgpuProbe, OgpuResult, INTERNAL_ERROR, INVALID_ARGUMENT, OUT_OF_RANGE,
    SUCCESS,
};
use std::{
    ffi::c_void,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
    rc::Rc,
};

pub struct OgpuDevice {
    inner: Rc<Device>,
}
pub struct OgpuBuffer {
    inner: Buffer,
}
pub struct OgpuKernel {
    inner: Rc<Kernel>,
}
pub struct OgpuBatch {
    inner: Batch,
}
pub struct OgpuCompletion {
    inner: Completion,
}

// SAFETY (for callers of these private helpers): error, when non-NULL, is writable
// and does not overlap inputs. The outer FFI functions document all pointer contracts.
unsafe fn call(error: *mut OgpuError, f: impl FnOnce() -> Result<(), Error>) -> OgpuResult {
    let result = catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| {
        Err(Error::new(
            INTERNAL_ERROR,
            "Rust panic contained at C boundary",
        ))
    });
    let (status, diagnostic) = match result {
        Ok(()) => (
            SUCCESS,
            OgpuError {
                vulkan_result: 0,
                message: [0; 256],
            },
        ),
        Err(e) => (e.status, e.diagnostic()),
    };
    if !error.is_null() {
        unsafe {
            error.write(diagnostic);
        }
    }
    status
}

unsafe fn create<T>(
    out: *mut *mut T,
    error: *mut OgpuError,
    f: impl FnOnce() -> Result<T, Error>,
) -> OgpuResult {
    unsafe {
        call(error, || {
            if out.is_null() {
                return Err(Error::new(INVALID_ARGUMENT, "NULL creation output"));
            }
            out.write(ptr::null_mut());
            let value = Box::new(f()?);
            out.write(Box::into_raw(value));
            Ok(())
        })
    }
}

fn required<T>(pointer: *const T) -> Result<(), Error> {
    if pointer.is_null() {
        Err(Error::new(INVALID_ARGUMENT, "NULL required argument"))
    } else {
        Ok(())
    }
}

/// # Safety
/// See include/ogpu.h: live probe, valid non-overlapping output objects.
#[no_mangle]
pub unsafe extern "C" fn ogpu_device_create(
    probe: *const OgpuProbe,
    index: u32,
    out_device: *mut *mut OgpuDevice,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out_device, error, || {
            required(probe)?;
            let probe = &*probe;
            let physical = *probe
                .physical_devices
                .get(index as usize)
                .ok_or_else(|| Error::new(OUT_OF_RANGE, "Device index out of range"))?;
            Ok(OgpuDevice {
                inner: Device::new(probe._vulkan.clone(), physical)?,
            })
        })
    }
}

/// # Safety
/// A live uniquely owned handle or NULL; no concurrent operations on this device/children.
#[no_mangle]
pub unsafe extern "C" fn ogpu_device_destroy(device: *mut OgpuDevice) {
    if !device.is_null() {
        unsafe {
            drop(Box::from_raw(device));
        }
    }
}

/// # Safety
/// See include/ogpu.h: live device and valid creation outputs, externally serialized.
#[no_mangle]
pub unsafe extern "C" fn ogpu_buffer_create(
    device: *mut OgpuDevice,
    size: u64,
    out_buffer: *mut *mut OgpuBuffer,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out_buffer, error, || {
            required(device)?;
            let size = usize::try_from(size)
                .map_err(|_| Error::new(INVALID_ARGUMENT, "Buffer too large"))?;
            Ok(OgpuBuffer {
                inner: Buffer::new((*device).inner.clone(), size)?,
            })
        })
    }
}

/// # Safety
/// A live uniquely owned handle or NULL; no concurrent operation may use this allocation.
#[no_mangle]
pub unsafe extern "C" fn ogpu_buffer_destroy(buffer: *mut OgpuBuffer) {
    if !buffer.is_null() {
        unsafe {
            drop(Box::from_raw(buffer));
        }
    }
}

/// # Safety
/// See include/ogpu.h: data is readable for size bytes, outputs non-overlapping.
#[no_mangle]
pub unsafe extern "C" fn ogpu_buffer_write(
    buffer: *mut OgpuBuffer,
    offset: u64,
    data: *const c_void,
    size: u64,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(buffer)?;
            let buffer = &mut (*buffer).inner;
            let offset = usize::try_from(offset)
                .map_err(|_| Error::new(OUT_OF_RANGE, "Offset too large"))?;
            let size = usize::try_from(size)
                .map_err(|_| Error::new(OUT_OF_RANGE, "Transfer too large"))?;
            buffer.range(offset, size)?; // Validate before constructing the caller's slice.
            let bytes = if size == 0 {
                &[]
            } else {
                required(data)?;
                std::slice::from_raw_parts(data.cast(), size)
            };
            buffer.write(offset, bytes)
        })
    }
}

/// # Safety
/// See include/ogpu.h: data is writable for size bytes and does not overlap other arguments.
#[no_mangle]
pub unsafe extern "C" fn ogpu_buffer_read(
    buffer: *const OgpuBuffer,
    offset: u64,
    data: *mut c_void,
    size: u64,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(buffer)?;
            let buffer = &(*buffer).inner;
            let offset = usize::try_from(offset)
                .map_err(|_| Error::new(OUT_OF_RANGE, "Offset too large"))?;
            let size = usize::try_from(size)
                .map_err(|_| Error::new(OUT_OF_RANGE, "Transfer too large"))?;
            buffer.range(offset, size)?;
            if size != 0 {
                required(data)?;
            }
            buffer.read(offset, data.cast(), size)
        })
    }
}

/// # Safety
/// See include/ogpu.h: live buffer and writable, non-overlapping outputs.
#[no_mangle]
pub unsafe extern "C" fn ogpu_buffer_device_address(
    buffer: *const OgpuBuffer,
    out_address: *mut u64,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(buffer)?;
            required(out_address)?;
            let address = (*buffer).inner.address()?;
            out_address.write(address);
            Ok(())
        })
    }
}

/// # Safety
/// See include/ogpu.h: readable aligned SPIR-V words, valid shader semantics, valid outputs.
#[no_mangle]
pub unsafe extern "C" fn ogpu_kernel_create(
    device: *mut OgpuDevice,
    words: *const u32,
    word_count: u64,
    push_size: u32,
    out_kernel: *mut *mut OgpuKernel,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out_kernel, error, || {
            required(device)?;
            required(words)?;
            if word_count < 5 || word_count > (isize::MAX as u64) / 4 || words as usize % 4 != 0 {
                return Err(Error::new(
                    INVALID_ARGUMENT,
                    "Invalid SPIR-V word count/alignment",
                ));
            }
            let words = std::slice::from_raw_parts(words, word_count as usize);
            Ok(OgpuKernel {
                inner: Rc::new(Kernel::new((*device).inner.clone(), words, push_size)?),
            })
        })
    }
}

/// # Safety
/// A live uniquely owned handle or NULL; no concurrent operations on this kernel/device.
#[no_mangle]
pub unsafe extern "C" fn ogpu_kernel_destroy(kernel: *mut OgpuKernel) {
    if !kernel.is_null() {
        unsafe {
            drop(Box::from_raw(kernel));
        }
    }
}

/// # Safety
/// See include/ogpu.h: readable argument bytes, live in-bounds device addresses, race-free
/// shader execution, and external serialization of this device and every child object.
#[no_mangle]
pub unsafe extern "C" fn ogpu_dispatch_wait(
    kernel: *mut OgpuKernel,
    groups: u32,
    arguments: *const c_void,
    argument_bytes: u32,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(kernel)?;
            let root = if argument_bytes == 0 {
                &[]
            } else {
                required(arguments)?;
                std::slice::from_raw_parts(arguments.cast(), argument_bytes as usize)
            };
            (*kernel).inner.dispatch_wait(groups, root)
        })
    }
}

/// # Safety
/// See include/ogpu.h: live device, valid creation outputs, external serialization.
#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_create(
    device: *mut OgpuDevice,
    out_batch: *mut *mut OgpuBatch,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out_batch, error, || {
            required(device)?;
            Ok(OgpuBatch {
                inner: Batch::new((*device).inner.clone())?,
            })
        })
    }
}

/// # Safety
/// Live uniquely owned handle or NULL; no concurrent calls on this device/children.
#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_destroy(batch: *mut OgpuBatch) {
    if !batch.is_null() {
        unsafe {
            drop(Box::from_raw(batch));
        }
    }
}

/// # Safety
/// See include/ogpu.h: readable arguments; referenced allocations must outlive their
/// recorded use. Handles and outputs must be valid, non-overlapping, and serialized.
#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_dispatch(
    batch: *mut OgpuBatch,
    kernel: *mut OgpuKernel,
    groups: u32,
    arguments: *const c_void,
    argument_bytes: u32,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(batch)?;
            required(kernel)?;
            let root = if argument_bytes == 0 {
                &[]
            } else {
                required(arguments)?;
                std::slice::from_raw_parts(arguments.cast(), argument_bytes as usize)
            };
            (*batch)
                .inner
                .dispatch((*kernel).inner.clone(), groups, root)
        })
    }
}

/// # Safety
/// See include/ogpu.h: valid batch/error pointers and external serialization.
#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_barrier(
    batch: *mut OgpuBatch,
    source: u32,
    destination: u32,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(batch)?;
            (*batch).inner.barrier(source, destination)
        })
    }
}

/// # Safety
/// See include/ogpu.h: valid shader addresses and explicit race-free dependencies;
/// allocations remain live without host access until GPU uses complete. Valid outputs
/// and external serialization are required. Submission does not wait for completion.
#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_submit(
    batch: *mut OgpuBatch,
    out_completion: *mut *mut OgpuCompletion,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out_completion, error, || {
            required(batch)?;
            Ok(OgpuCompletion {
                inner: (*batch).inner.submit()?,
            })
        })
    }
}

/// # Safety
/// See include/ogpu.h: live completion, writable error, external serialization.
#[no_mangle]
pub unsafe extern "C" fn ogpu_completion_wait(
    completion: *mut OgpuCompletion,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(completion)?;
            (*completion).inner.wait()
        })
    }
}

/// # Safety
/// Live uniquely owned handle or NULL, externally serialized. Referenced allocations
/// must remain alive until this call returns; pending work is drained, not cancelled.
#[no_mangle]
pub unsafe extern "C" fn ogpu_completion_destroy(completion: *mut OgpuCompletion) {
    if !completion.is_null() {
        unsafe {
            drop(Box::from_raw(completion));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_batch_arguments_need_no_driver() {
        unsafe {
            let mut batch = ptr::dangling_mut::<OgpuBatch>();
            let mut completion = ptr::dangling_mut::<OgpuCompletion>();
            assert_eq!(
                ogpu_batch_create(ptr::null_mut(), &mut batch, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert!(batch.is_null());
            assert_eq!(
                ogpu_batch_dispatch(
                    ptr::null_mut(),
                    ptr::null_mut(),
                    1,
                    ptr::null(),
                    0,
                    ptr::null_mut()
                ),
                INVALID_ARGUMENT
            );
            assert_eq!(
                ogpu_batch_barrier(ptr::null_mut(), 2, 1, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert_eq!(
                ogpu_batch_submit(ptr::null_mut(), &mut completion, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert!(completion.is_null());
            assert_eq!(
                ogpu_completion_wait(ptr::null_mut(), ptr::null_mut()),
                INVALID_ARGUMENT
            );
            // A NULL creation output must be checked before dereferencing input handles.
            assert_eq!(
                ogpu_batch_submit(ptr::dangling_mut(), ptr::null_mut(), ptr::null_mut()),
                INVALID_ARGUMENT
            );
            ogpu_batch_destroy(ptr::null_mut());
            ogpu_completion_destroy(ptr::null_mut());
        }
    }

    #[test]
    fn invalid_execution_arguments_need_no_driver() {
        unsafe {
            let mut device = ptr::dangling_mut::<OgpuDevice>();
            let mut buffer = ptr::dangling_mut::<OgpuBuffer>();
            let mut kernel = ptr::dangling_mut::<OgpuKernel>();
            let mut error = OgpuError {
                vulkan_result: 42,
                message: [1; 256],
            };
            assert_eq!(
                ogpu_device_create(ptr::null(), 0, &mut device, &mut error),
                INVALID_ARGUMENT
            );
            assert!(device.is_null());
            assert_eq!(error.vulkan_result, 0);
            assert_eq!(error.message[255], 0);
            assert_eq!(
                ogpu_buffer_create(ptr::null_mut(), 1, &mut buffer, &mut error),
                INVALID_ARGUMENT
            );
            assert!(buffer.is_null());
            assert_eq!(
                ogpu_kernel_create(ptr::null_mut(), ptr::null(), 0, 0, &mut kernel, &mut error),
                INVALID_ARGUMENT
            );
            assert!(kernel.is_null());
            assert_eq!(
                ogpu_dispatch_wait(ptr::null_mut(), 1, ptr::null(), 0, &mut error),
                INVALID_ARGUMENT
            );
            ogpu_buffer_destroy(ptr::null_mut());
            ogpu_kernel_destroy(ptr::null_mut());
            ogpu_device_destroy(ptr::null_mut());
        }
    }
}
