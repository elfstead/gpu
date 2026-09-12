//! C ownership boundary for synchronous dispatch and one-shot asynchronous batches.
use crate::{
    compute::{Batch, Buffer, Completion, Device, ImageTable, Kernel, Raster, Target},
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
    inner: Rc<Buffer>,
}
pub struct OgpuTarget {
    inner: Rc<Target>,
}
pub struct OgpuImageTable {
    inner: Rc<ImageTable>,
}

/// # Safety
/// Live same-device batch/target, writable error; externally serialized.
#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_discard_target(
    batch: *mut OgpuBatch,
    target: *const OgpuTarget,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(batch)?;
            required(target)?;
            (*batch).inner.discard_target((*target).inner.clone())
        })
    }
}

#[repr(C)]
pub struct OgpuImageEntry {
    pub target: *const OgpuTarget,
    pub kind: u32,
    pub reserved: u32,
}

/// # Safety
/// Live same-device targets; readable entries and writable, non-overlapping outputs.
#[no_mangle]
pub unsafe extern "C" fn ogpu_image_table_create(
    device: *mut OgpuDevice,
    entries: *const OgpuImageEntry,
    count: u32,
    out_table: *mut *mut OgpuImageTable,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(out_table)?;
            *out_table = ptr::null_mut();
            required(device)?;
            required(entries)?;
            if count == 0 {
                return Err(Error::new(INVALID_ARGUMENT, "Empty image table"));
            }
            let mut owned = Vec::new();
            for entry in std::slice::from_raw_parts(entries, count as usize) {
                required(entry.target)?;
                if entry.reserved != 0 {
                    return Err(Error::new(INVALID_ARGUMENT, "Reserved image entry field"));
                }
                owned.push(((*entry.target).inner.clone(), entry.kind));
            }
            let inner = Rc::new(ImageTable::new((*device).inner.clone(), owned)?);
            *out_table = Box::into_raw(Box::new(OgpuImageTable { inner }));
            Ok(())
        })
    }
}

/// # Safety
/// Live uniquely owned handle or NULL; externally serialized.
#[no_mangle]
pub unsafe extern "C" fn ogpu_image_table_destroy(table: *mut OgpuImageTable) {
    if !table.is_null() {
        unsafe {
            drop(Box::from_raw(table));
        }
    }
}

/// # Safety
/// Live same-device batch/table; writable error; externally serialized.
#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_bind_image_table(
    batch: *mut OgpuBatch,
    table: *const OgpuImageTable,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(batch)?;
            required(table)?;
            (*batch).inner.bind_images((*table).inner.clone())
        })
    }
}
pub struct OgpuRaster {
    inner: Rc<Raster>,
}

#[repr(C)]
pub struct OgpuDrawArguments {
    pub vertex_count: u32,
    pub instance_count: u32,
    pub first_vertex: u32,
    pub first_instance: u32,
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

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct OgpuTimingInfo {
    pub timestamp_period_ns: f64,
    pub timestamp_valid_bits: u32,
    pub reserved: u32,
}

/// # Safety
/// Live device, writable non-overlapping outputs; externally serialized host calls.
#[no_mangle]
pub unsafe extern "C" fn ogpu_device_timing_info(
    device: *mut OgpuDevice,
    out_info: *mut OgpuTimingInfo,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(out_info)?;
            out_info.write(OgpuTimingInfo::default());
            required(device)?;
            let (timestamp_period_ns, timestamp_valid_bits) = (*device).inner.timing_info()?;
            out_info.write(OgpuTimingInfo {
                timestamp_period_ns,
                timestamp_valid_bits,
                reserved: 0,
            });
            Ok(())
        })
    }
}

/// # Safety
/// Live batch, writable error if supplied, externally serialized device access.
#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_enable_timing(
    batch: *mut OgpuBatch,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(batch)?;
            (*batch).inner.enable_timing()
        })
    }
}

/// # Safety
/// Live completion and writable non-overlapping outputs; externally serialized access.
#[no_mangle]
pub unsafe extern "C" fn ogpu_completion_elapsed_ns(
    completion: *mut OgpuCompletion,
    out_nanoseconds: *mut f64,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(out_nanoseconds)?;
            out_nanoseconds.write(0.0);
            required(completion)?;
            out_nanoseconds.write((*completion).inner.elapsed_ns()?);
            Ok(())
        })
    }
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
                inner: Rc::new(Buffer::new((*device).inner.clone(), size)?),
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
            let buffer = &(*buffer).inner;
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
/// See include/ogpu.h: live completion, writable outputs, external serialization.
#[no_mangle]
pub unsafe extern "C" fn ogpu_completion_poll(
    completion: *mut OgpuCompletion,
    out_complete: *mut u32,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(out_complete)?;
            *out_complete = 0;
            required(completion)?;
            *out_complete = u32::from((*completion).inner.poll()?);
            Ok(())
        })
    }
}

/// # Safety
/// Live batch/buffer on one device, writable error, external serialization.
#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_retain_buffer(
    batch: *mut OgpuBatch,
    buffer: *const OgpuBuffer,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(batch)?;
            required(buffer)?;
            (*batch).inner.retain_buffer((*buffer).inner.clone())
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

/// # Safety
/// Same pointer/ownership rules as device_create; requires a graphics+compute queue.
#[no_mangle]
pub unsafe extern "C" fn ogpu_device_create_graphics(
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
                inner: Device::new_graphics(probe._vulkan.clone(), physical)?,
            })
        })
    }
}

/// # Safety
/// See include/ogpu.h: live device, writable independent outputs, external serialization.
#[no_mangle]
pub unsafe extern "C" fn ogpu_target_create_rgba8(
    device: *mut OgpuDevice,
    width: u32,
    height: u32,
    out_target: *mut *mut OgpuTarget,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out_target, error, || {
            required(device)?;
            Ok(OgpuTarget {
                inner: Rc::new(Target::new((*device).inner.clone(), width, height)?),
            })
        })
    }
}

/// # Safety
/// Live uniquely owned handle or NULL, externally serialized. Recorded uses retain it.
#[no_mangle]
pub unsafe extern "C" fn ogpu_target_destroy(target: *mut OgpuTarget) {
    if !target.is_null() {
        unsafe {
            drop(Box::from_raw(target));
        }
    }
}

// SAFETY: caller supplies word_count readable words with a lifetime covering the call.
unsafe fn shader_words<'a>(words: *const u32, word_count: u64) -> Result<&'a [u32], Error> {
    required(words)?;
    if word_count < 5 || word_count > (isize::MAX as u64) / 4 || words as usize % 4 != 0 {
        return Err(Error::new(
            INVALID_ARGUMENT,
            "Invalid SPIR-V word count/alignment",
        ));
    }
    unsafe { Ok(std::slice::from_raw_parts(words, word_count as usize)) }
}

/// # Safety
/// See include/ogpu.h: valid matching vertex/fragment SPIR-V with only enabled features,
/// live device, readable word arrays, writable independent outputs, external serialization.
#[no_mangle]
pub unsafe extern "C" fn ogpu_raster_create(
    device: *mut OgpuDevice,
    vertex_words: *const u32,
    vertex_count: u64,
    fragment_words: *const u32,
    fragment_count: u64,
    push_size: u32,
    out_raster: *mut *mut OgpuRaster,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out_raster, error, || {
            required(device)?;
            let vertex = shader_words(vertex_words, vertex_count)?;
            let fragment = shader_words(fragment_words, fragment_count)?;
            Ok(OgpuRaster {
                inner: Rc::new(Raster::new(
                    (*device).inner.clone(),
                    vertex,
                    fragment,
                    push_size,
                )?),
            })
        })
    }
}

/// # Safety
/// Live uniquely owned handle or NULL, externally serialized. Recorded uses retain it.
#[no_mangle]
pub unsafe extern "C" fn ogpu_raster_destroy(raster: *mut OgpuRaster) {
    if !raster.is_null() {
        unsafe {
            drop(Box::from_raw(raster));
        }
    }
}

/// # Safety
/// See include/ogpu.h: valid handles/arguments, valid GPU-produced indirect contents,
/// live reachable addresses through completion, and explicit race-free dependencies.
#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_draw_indirect(
    batch: *mut OgpuBatch,
    raster: *mut OgpuRaster,
    target: *mut OgpuTarget,
    indirect: *mut OgpuBuffer,
    offset: u64,
    arguments: *const c_void,
    argument_bytes: u32,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(batch)?;
            required(raster)?;
            required(target)?;
            required(indirect)?;
            let offset = usize::try_from(offset)
                .map_err(|_| Error::new(OUT_OF_RANGE, "Offset too large"))?;
            let root = if argument_bytes == 0 {
                &[]
            } else {
                required(arguments)?;
                std::slice::from_raw_parts(arguments.cast(), argument_bytes as usize)
            };
            (*batch).inner.draw(
                (*raster).inner.clone(),
                (*target).inner.clone(),
                (*indirect).inner.clone(),
                offset,
                root,
            )
        })
    }
}

/// # Safety
/// See include/ogpu.h: live same-device objects, independent writable error output,
/// external serialization, and no host access to pending GPU resources.
#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_copy_target(
    batch: *mut OgpuBatch,
    target: *mut OgpuTarget,
    destination: *mut OgpuBuffer,
    offset: u64,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(batch)?;
            required(target)?;
            required(destination)?;
            let offset = usize::try_from(offset)
                .map_err(|_| Error::new(OUT_OF_RANGE, "Offset too large"))?;
            (*batch).inner.copy_target(
                (*target).inner.clone(),
                (*destination).inner.clone(),
                offset,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_graphics_arguments_need_no_driver() {
        unsafe {
            let mut table = ptr::dangling_mut::<OgpuImageTable>();
            assert_eq!(
                ogpu_image_table_create(
                    ptr::null_mut(),
                    ptr::null(),
                    0,
                    &mut table,
                    ptr::null_mut()
                ),
                INVALID_ARGUMENT
            );
            assert!(table.is_null());
            assert_eq!(
                ogpu_batch_bind_image_table(ptr::null_mut(), ptr::null(), ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert_eq!(
                ogpu_batch_discard_target(ptr::null_mut(), ptr::null(), ptr::null_mut()),
                INVALID_ARGUMENT
            );
            ogpu_image_table_destroy(ptr::null_mut());
            let mut device = ptr::dangling_mut::<OgpuDevice>();
            let mut target = ptr::dangling_mut::<OgpuTarget>();
            let mut raster = ptr::dangling_mut::<OgpuRaster>();
            assert_eq!(
                ogpu_device_create_graphics(ptr::null(), 0, &mut device, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert!(device.is_null());
            assert_eq!(
                ogpu_target_create_rgba8(ptr::null_mut(), 64, 64, &mut target, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert!(target.is_null());
            assert_eq!(
                ogpu_raster_create(
                    ptr::null_mut(),
                    ptr::null(),
                    0,
                    ptr::null(),
                    0,
                    0,
                    &mut raster,
                    ptr::null_mut()
                ),
                INVALID_ARGUMENT
            );
            assert!(raster.is_null());
            assert_eq!(
                ogpu_batch_draw_indirect(
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    0,
                    ptr::null(),
                    0,
                    ptr::null_mut()
                ),
                INVALID_ARGUMENT
            );
            assert_eq!(
                ogpu_batch_copy_target(
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    0,
                    ptr::null_mut()
                ),
                INVALID_ARGUMENT
            );
            ogpu_target_destroy(ptr::null_mut());
            ogpu_raster_destroy(ptr::null_mut());
        }
    }
    #[test]
    fn invalid_batch_arguments_need_no_driver() {
        unsafe {
            let mut complete = 99;
            assert_eq!(
                ogpu_completion_poll(ptr::null_mut(), &mut complete, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert_eq!(complete, 0);
            assert_eq!(
                ogpu_completion_poll(ptr::dangling_mut(), ptr::null_mut(), ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert_eq!(
                ogpu_batch_retain_buffer(ptr::null_mut(), ptr::null(), ptr::null_mut()),
                INVALID_ARGUMENT
            );
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
    fn invalid_timing_arguments_need_no_driver() {
        unsafe {
            let mut info = OgpuTimingInfo {
                timestamp_period_ns: 1.0,
                timestamp_valid_bits: 64,
                reserved: 9,
            };
            assert_eq!(
                ogpu_device_timing_info(ptr::null_mut(), &mut info, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert_eq!(
                (
                    info.timestamp_period_ns,
                    info.timestamp_valid_bits,
                    info.reserved
                ),
                (0.0, 0, 0)
            );
            assert_eq!(
                ogpu_device_timing_info(ptr::dangling_mut(), ptr::null_mut(), ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert_eq!(
                ogpu_batch_enable_timing(ptr::null_mut(), ptr::null_mut()),
                INVALID_ARGUMENT
            );
            let mut elapsed = 42.0;
            assert_eq!(
                ogpu_completion_elapsed_ns(ptr::null_mut(), &mut elapsed, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert_eq!(elapsed, 0.0);
            assert_eq!(
                ogpu_completion_elapsed_ns(ptr::dangling_mut(), ptr::null_mut(), ptr::null_mut()),
                INVALID_ARGUMENT
            );
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
