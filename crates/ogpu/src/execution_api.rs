//! C ownership boundary for synchronous dispatch and one-shot asynchronous batches.
#[cfg(test)]
use crate::SUCCESS;
use crate::{
    compute::{
        Batch, Buffer, Completion, Device, Image, ImageHeap, Kernel, Raster, RecordingStorage,
        SamplerHeap,
    },
    Error, OgpuDeviceLimits, OgpuError, OgpuProbe, OgpuResult, OgpuShaderDesc,
    OgpuSpecializationConstant, OgpuTimingInfo, INVALID_ARGUMENT, OUT_OF_RANGE,
};
use std::{ffi::c_void, ptr, rc::Rc};

pub struct OgpuDevice {
    inner: Rc<Device>,
}
pub struct OgpuRecordingStorage {
    inner: Rc<RecordingStorage>,
}

/// # Safety
/// Live device, independent writable outputs; externally serialized.
#[no_mangle]
pub unsafe extern "C" fn ogpu_recording_storage_create(
    device: *mut OgpuDevice,
    out: *mut *mut OgpuRecordingStorage,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out, error, || {
            required(device)?;
            Ok(OgpuRecordingStorage {
                inner: Rc::new(RecordingStorage::new((*device).inner.clone())?),
            })
        })
    }
}

/// # Safety
/// Live storage, independent writable error; externally serialized.
#[no_mangle]
pub unsafe extern "C" fn ogpu_recording_storage_trim(
    storage: *mut OgpuRecordingStorage,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(storage)?;
            (*storage).inner.trim()
        })
    }
}

/// # Safety
/// Live uniquely owned handle or NULL; externally serialized.
#[no_mangle]
pub unsafe extern "C" fn ogpu_recording_storage_destroy(storage: *mut OgpuRecordingStorage) {
    if !storage.is_null() {
        unsafe {
            drop(Box::from_raw(storage));
        }
    }
}

/// # Safety
/// Live storage and independent writable outputs; externally serialized.
#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_create_in(
    storage: *mut OgpuRecordingStorage,
    out: *mut *mut OgpuBatch,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out, error, || {
            required(storage)?;
            Ok(OgpuBatch {
                inner: Batch::new_in((*storage).inner.clone())?,
            })
        })
    }
}
pub struct OgpuBuffer {
    inner: Rc<Buffer>,
}
pub struct OgpuImage {
    inner: Rc<Image>,
}
pub struct OgpuImageHeap {
    inner: Rc<ImageHeap>,
}
pub struct OgpuSamplerHeap {
    inner: Rc<SamplerHeap>,
}
pub use crate::compute::ImageDesc as OgpuImageDesc;
pub use crate::compute::SamplerDesc as OgpuSamplerDesc;

/// # Safety
/// Live device and writable, non-overlapping outputs; externally serialized.
#[no_mangle]
pub unsafe extern "C" fn ogpu_device_capabilities(
    device: *const OgpuDevice,
    out_capabilities: *mut crate::OgpuCapabilities,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(device)?;
            required(out_capabilities)?;
            *out_capabilities = (*device).inner.enabled_capabilities();
            Ok(())
        })
    }
}

/// # Safety
/// Live device, readable description and independent writable error; serialized.
#[no_mangle]
pub unsafe extern "C" fn ogpu_image_check_support(
    device: *const OgpuDevice,
    desc: *const OgpuImageDesc,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(device)?;
            required(desc)?;
            Image::check_support(&(*device).inner, *desc).map(|_| ())
        })
    }
}

/// # Safety
/// Live device and writable, non-overlapping outputs; externally serialized.
#[no_mangle]
pub unsafe extern "C" fn ogpu_device_limits(
    device: *const OgpuDevice,
    out_limits: *mut OgpuDeviceLimits,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(device)?;
            required(out_limits)?;
            *out_limits = (*device).inner.execution_limits();
            Ok(())
        })
    }
}

unsafe fn shader<'a>(
    desc: *const OgpuShaderDesc,
) -> Result<(&'a [u32], &'a [OgpuSpecializationConstant]), Error> {
    let shader = unsafe { crate::shader::Shader::read(desc)? };
    Ok((shader.spirv()?, shader.constants))
}

/// # Safety
/// Live same-device batch/target, writable error; externally serialized.
#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_discard_image(
    batch: *mut OgpuBatch,
    target: *const OgpuImage,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(batch)?;
            required(target)?;
            (*batch).inner.discard_image((*target).inner.clone())
        })
    }
}

pub use crate::api_types::OgpuImageEntry;

/// # Safety
/// Live device; writable, non-overlapping outputs; external serialization.
#[no_mangle]
pub unsafe extern "C" fn ogpu_image_heap_create(
    device: *mut OgpuDevice,
    capacity: u32,
    out_heap: *mut *mut OgpuImageHeap,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(out_heap)?;
            *out_heap = ptr::null_mut();
            required(device)?;
            let inner = Rc::new(ImageHeap::new((*device).inner.clone(), capacity)?);
            *out_heap = Box::into_raw(Box::new(OgpuImageHeap { inner }));
            Ok(())
        })
    }
}

/// # Safety
/// Live uniquely owned handle or NULL; externally serialized.
#[no_mangle]
pub unsafe extern "C" fn ogpu_image_heap_destroy(heap: *mut OgpuImageHeap) {
    if !heap.is_null() {
        unsafe {
            drop(Box::from_raw(heap));
        }
    }
}

/// # Safety
/// Live same-device batch/heap; writable error; externally serialized.
#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_bind_image_heap(
    batch: *mut OgpuBatch,
    heap: *const OgpuImageHeap,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(batch)?;
            required(heap)?;
            (*batch).inner.bind_images((*heap).inner.clone())
        })
    }
}

fn exclusive<T>(heap: &mut Rc<T>) -> Result<&mut T, Error> {
    Rc::get_mut(heap).ok_or_else(|| Error::new(INVALID_ARGUMENT, "Heap is retained by a recording or unretired submission; discard recordings or observe completion before editing"))
}

/// # Safety
/// Live heap and target handles; readable entries and writable error; external serialization.
#[no_mangle]
pub unsafe extern "C" fn ogpu_image_heap_write(
    heap: *mut OgpuImageHeap,
    first: u32,
    entries: *const OgpuImageEntry,
    count: u32,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(heap)?;
            let heap = exclusive(&mut (*heap).inner)?;
            let entries = if count == 0 {
                &[]
            } else {
                required(entries)?;
                std::slice::from_raw_parts(entries, count as usize)
            };
            let mut owned = Vec::new();
            for entry in entries {
                required(entry.image)?;
                if entry.reserved != 0 {
                    return Err(Error::new(INVALID_ARGUMENT, "Reserved image entry field"));
                }
                owned.push(((*entry.image).inner.clone(), entry.kind));
            }
            heap.write(first, owned)
        })
    }
}

/// # Safety
/// Live heap; writable error; external serialization.
#[no_mangle]
pub unsafe extern "C" fn ogpu_image_heap_clear(
    heap: *mut OgpuImageHeap,
    first: u32,
    count: u32,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(heap)?;
            exclusive(&mut (*heap).inner)?.clear(first, count)
        })
    }
}

/// # Safety
/// Live device; writable non-overlapping outputs; external serialization.
#[no_mangle]
pub unsafe extern "C" fn ogpu_sampler_heap_create(
    device: *mut OgpuDevice,
    capacity: u32,
    out_heap: *mut *mut OgpuSamplerHeap,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(out_heap)?;
            *out_heap = ptr::null_mut();
            required(device)?;
            let inner = Rc::new(SamplerHeap::new((*device).inner.clone(), capacity)?);
            *out_heap = Box::into_raw(Box::new(OgpuSamplerHeap { inner }));
            Ok(())
        })
    }
}

/// # Safety
/// Live uniquely owned handle or NULL; external serialization.
#[no_mangle]
pub unsafe extern "C" fn ogpu_sampler_heap_destroy(heap: *mut OgpuSamplerHeap) {
    if !heap.is_null() {
        unsafe {
            drop(Box::from_raw(heap));
        }
    }
}

/// # Safety
/// Live heap, readable sampler descriptions and writable error; external serialization.
#[no_mangle]
pub unsafe extern "C" fn ogpu_sampler_heap_write(
    heap: *mut OgpuSamplerHeap,
    first: u32,
    entries: *const OgpuSamplerDesc,
    count: u32,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(heap)?;
            let heap = exclusive(&mut (*heap).inner)?;
            let entries = if count == 0 {
                &[]
            } else {
                required(entries)?;
                std::slice::from_raw_parts(entries, count as usize)
            };
            heap.write(first, entries)
        })
    }
}

/// # Safety
/// Live same-device batch/heap; writable error; external serialization.
#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_bind_sampler_heap(
    batch: *mut OgpuBatch,
    heap: *const OgpuSamplerHeap,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(batch)?;
            required(heap)?;
            (*batch).inner.bind_samplers((*heap).inner.clone())
        })
    }
}
pub struct OgpuRaster {
    inner: Rc<Raster>,
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

use crate::boundary::{call, create, required};

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
    placement: u32,
    out_buffer: *mut *mut OgpuBuffer,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out_buffer, error, || {
            required(device)?;
            let size = usize::try_from(size)
                .map_err(|_| Error::new(INVALID_ARGUMENT, "Buffer too large"))?;
            let placement = match placement {
                0 => crate::compute::Placement::Host,
                1 => crate::compute::Placement::Device,
                _ => return Err(Error::new(INVALID_ARGUMENT, "Invalid buffer placement")),
            };
            Ok(OgpuBuffer {
                inner: Rc::new(Buffer::placed((*device).inner.clone(), size, placement)?),
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
    desc: *const OgpuShaderDesc,
    push_size: u32,
    out_kernel: *mut *mut OgpuKernel,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out_kernel, error, || {
            required(device)?;
            let (words, constants) = shader(desc)?;
            Ok(OgpuKernel {
                inner: Rc::new(Kernel::new(
                    (*device).inner.clone(),
                    words,
                    push_size,
                    constants,
                )?),
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
    groups_x: u32,
    groups_y: u32,
    groups_z: u32,
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
            (*kernel)
                .inner
                .dispatch_wait([groups_x, groups_y, groups_z], root)
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
    groups_x: u32,
    groups_y: u32,
    groups_z: u32,
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
            (*batch).inner.dispatch(
                (*kernel).inner.clone(),
                [groups_x, groups_y, groups_z],
                root,
            )
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
/// Live batch and buffers, writable error, externally serialized; see include/ogpu.h.
#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_copy_buffer(
    batch: *mut OgpuBatch,
    source: *const OgpuBuffer,
    source_offset: u64,
    destination: *const OgpuBuffer,
    destination_offset: u64,
    size: u64,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(batch)?;
            required(source)?;
            required(destination)?;
            let convert = |value| {
                usize::try_from(value).map_err(|_| Error::new(OUT_OF_RANGE, "Copy range too large"))
            };
            (*batch).inner.copy_buffer(
                (*source).inner.clone(),
                convert(source_offset)?,
                (*destination).inner.clone(),
                convert(destination_offset)?,
                convert(size)?,
            )
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
pub unsafe extern "C" fn ogpu_image_create(
    device: *mut OgpuDevice,
    desc: *const OgpuImageDesc,
    out_target: *mut *mut OgpuImage,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out_target, error, || {
            required(device)?;
            required(desc)?;
            Ok(OgpuImage {
                inner: Rc::new(Image::new((*device).inner.clone(), *desc)?),
            })
        })
    }
}

/// # Safety
/// Live uniquely owned handle or NULL, externally serialized. Recorded uses retain it.
#[no_mangle]
pub unsafe extern "C" fn ogpu_image_destroy(target: *mut OgpuImage) {
    if !target.is_null() {
        unsafe {
            drop(Box::from_raw(target));
        }
    }
}

/// # Safety
/// See include/ogpu.h: valid matching vertex/fragment SPIR-V with only enabled features,
/// live device, readable word arrays, writable independent outputs, external serialization.
#[no_mangle]
pub unsafe extern "C" fn ogpu_raster_create(
    device: *mut OgpuDevice,
    vertex_desc: *const OgpuShaderDesc,
    fragment_desc: *const OgpuShaderDesc,
    push_size: u32,
    topology: u32,
    target_format: u32,
    out_raster: *mut *mut OgpuRaster,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out_raster, error, || {
            required(device)?;
            let (vertex, vertex_constants) = shader(vertex_desc)?;
            let (fragment, fragment_constants) = shader(fragment_desc)?;
            Ok(OgpuRaster {
                inner: Rc::new(Raster::new(
                    (*device).inner.clone(),
                    vertex,
                    fragment,
                    push_size,
                    [vertex_constants, fragment_constants],
                    topology,
                    target_format,
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
    target: *mut OgpuImage,
    indirect: *mut OgpuBuffer,
    offset: u64,
    arguments: *const c_void,
    argument_bytes: u32,
    load: u32,
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
                load,
            )
        })
    }
}

/// # Safety
/// See include/ogpu.h: live same-device objects, independent writable error output,
/// external serialization, and no host access to pending GPU resources.
#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_copy_image_to_buffer(
    batch: *mut OgpuBatch,
    target: *mut OgpuImage,
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
            (*batch).inner.copy_image_to_buffer(
                (*target).inner.clone(),
                (*destination).inner.clone(),
                offset,
            )
        })
    }
}

/// # Safety
/// Same-device live objects; valid outputs; no host access while a copy is pending.
#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_copy_buffer_to_image(
    batch: *mut OgpuBatch,
    source: *mut OgpuBuffer,
    offset: u64,
    image: *mut OgpuImage,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(batch)?;
            required(source)?;
            required(image)?;
            let offset = usize::try_from(offset)
                .map_err(|_| Error::new(OUT_OF_RANGE, "Offset too large"))?;
            (*batch).inner.copy_buffer_to_image(
                (*source).inner.clone(),
                offset,
                (*image).inner.clone(),
            )
        })
    }
}

#[cfg(test)]
#[path = "dispatch_tests.rs"]
mod dispatch_tests;

#[cfg(test)]
#[path = "image_tests.rs"]
mod image_tests;

#[cfg(test)]
#[path = "executable_tests.rs"]
mod executable_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_graphics_arguments_need_no_driver() {
        unsafe {
            let mut table = ptr::dangling_mut::<OgpuImageHeap>();
            assert_eq!(
                ogpu_image_heap_create(ptr::null_mut(), 0, &mut table, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert!(table.is_null());
            assert_eq!(
                ogpu_batch_bind_image_heap(ptr::null_mut(), ptr::null(), ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert_eq!(
                ogpu_batch_discard_image(ptr::null_mut(), ptr::null(), ptr::null_mut()),
                INVALID_ARGUMENT
            );
            ogpu_image_heap_destroy(ptr::null_mut());
            ogpu_sampler_heap_destroy(ptr::null_mut());
            assert_eq!(
                ogpu_image_heap_write(ptr::null_mut(), 0, ptr::null(), 0, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert_eq!(
                ogpu_image_heap_clear(ptr::null_mut(), 0, 0, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert_eq!(
                ogpu_sampler_heap_write(ptr::null_mut(), 0, ptr::null(), 0, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert_eq!(
                ogpu_batch_bind_sampler_heap(ptr::null_mut(), ptr::null(), ptr::null_mut()),
                INVALID_ARGUMENT
            );
            let mut sampler = ptr::dangling_mut::<OgpuSamplerHeap>();
            assert_eq!(
                ogpu_sampler_heap_create(ptr::null_mut(), 1, &mut sampler, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert!(sampler.is_null());
            let mut device = ptr::dangling_mut::<OgpuDevice>();
            let mut target = ptr::dangling_mut::<OgpuImage>();
            let mut raster = ptr::dangling_mut::<OgpuRaster>();
            assert_eq!(
                ogpu_device_create_graphics(ptr::null(), 0, &mut device, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert!(device.is_null());
            assert_eq!(
                ogpu_image_create(ptr::null_mut(), ptr::null(), &mut target, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert!(target.is_null());
            assert_eq!(
                ogpu_raster_create(
                    ptr::null_mut(),
                    ptr::null(),
                    ptr::null(),
                    0,
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
                    0,
                    ptr::null_mut()
                ),
                INVALID_ARGUMENT
            );
            assert_eq!(
                ogpu_batch_copy_image_to_buffer(
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    0,
                    ptr::null_mut()
                ),
                INVALID_ARGUMENT
            );
            ogpu_image_destroy(ptr::null_mut());
            ogpu_raster_destroy(ptr::null_mut());
        }
    }
    #[test]
    fn invalid_batch_arguments_need_no_driver() {
        unsafe {
            let mut storage = ptr::dangling_mut::<OgpuRecordingStorage>();
            assert_eq!(
                ogpu_recording_storage_create(ptr::null_mut(), &mut storage, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert!(storage.is_null());
            assert_eq!(
                ogpu_recording_storage_create(ptr::null_mut(), ptr::null_mut(), ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert_eq!(
                ogpu_recording_storage_trim(ptr::null_mut(), ptr::null_mut()),
                INVALID_ARGUMENT
            );
            let mut batch = ptr::dangling_mut::<OgpuBatch>();
            assert_eq!(
                ogpu_batch_create_in(ptr::null_mut(), &mut batch, ptr::null_mut()),
                INVALID_ARGUMENT
            );
            assert!(batch.is_null());
            ogpu_recording_storage_destroy(ptr::null_mut());
        }
        assert_eq!(
            unsafe {
                ogpu_batch_copy_buffer(
                    ptr::null_mut(),
                    ptr::null(),
                    0,
                    ptr::null(),
                    0,
                    0,
                    ptr::null_mut(),
                )
            },
            INVALID_ARGUMENT
        );
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
                    1,
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
                ogpu_buffer_create(ptr::null_mut(), 1, 0, &mut buffer, &mut error),
                INVALID_ARGUMENT
            );
            assert!(buffer.is_null());
            assert_eq!(
                ogpu_kernel_create(ptr::null_mut(), ptr::null(), 0, &mut kernel, &mut error),
                INVALID_ARGUMENT
            );
            assert!(kernel.is_null());
            assert_eq!(
                ogpu_dispatch_wait(ptr::null_mut(), 1, 1, 1, ptr::null(), 0, &mut error),
                INVALID_ARGUMENT
            );
            ogpu_buffer_destroy(ptr::null_mut());
            ogpu_kernel_destroy(ptr::null_mut());
            ogpu_device_destroy(ptr::null_mut());
        }
    }
}
