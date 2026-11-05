//! Thin C adapters for explicit descriptor storage and native-unit locations.
use super::*;

/// # Safety
/// Live device and writable disjoint output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_descriptor_scratch_requirements(
    device: *const Device,
    operation: u32,
    count: u32,
    out: *mut HostRequirements,
) -> Status {
    boundary(|| {
        if out.is_null() {
            return Err(INVALID);
        }
        let requirements = unsafe {
            rust::descriptor_scratch_requirements(
                rust::DeviceHandle::from_raw(device.cast_mut()).ok_or(INVALID)?,
                operation,
                count,
            )?
        };
        unsafe {
            out.write(requirements);
        }
        Ok(())
    })
}
/// # Safety
/// See header: live device/resources, readable arrays and disjoint writable bytes/scratch.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_write_resource_descriptors(
    device: *const Device,
    count: u32,
    inputs: *const ResourceDescriptor,
    outputs: *const HeapLocation,
    scratch: HostSpan,
) -> Status {
    boundary(|| unsafe {
        rust::write_resource_descriptors_raw(
            rust::DeviceHandle::from_raw(device.cast_mut()).ok_or(INVALID)?,
            count,
            inputs,
            outputs,
            scratch,
        )
    })
}
/// # Safety
/// See header: live device, readable arrays and disjoint writable bytes/scratch.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_write_sampler_descriptors(
    device: *const Device,
    count: u32,
    inputs: *const SamplerDesc,
    outputs: *const HeapLocation,
    scratch: HostSpan,
) -> Status {
    boundary(|| unsafe {
        rust::write_sampler_descriptors_raw(
            rust::DeviceHandle::from_raw(device.cast_mut()).ok_or(INVALID)?,
            count,
            inputs,
            outputs,
            scratch,
        )
    })
}

/// # Safety
/// Live device/resources, readable description and disjoint writable output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_heap_requirements_query(
    device: *const Device,
    desc: *const HeapRequest,
    out: *mut HeapRequirements,
) -> Status {
    boundary(|| unsafe {
        if out.is_null() {
            return Err(INVALID);
        }
        let device = rust::DeviceHandle::from_raw(device.cast_mut()).ok_or(INVALID)?;
        let value = rust::heap_requirements_query(device, desc.as_ref().ok_or(INVALID)?)?;
        out.write(value);
        Ok(())
    })
}

/// # Safety
/// Live device/resources, readable description and disjoint writable output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_resource_descriptor_shape(
    device: *const Device,
    desc: *const ResourceDescriptor,
    out: *mut DescriptorShape,
) -> Status {
    boundary(|| unsafe {
        if out.is_null() {
            return Err(INVALID);
        }
        let device = rust::DeviceHandle::from_raw(device.cast_mut()).ok_or(INVALID)?;
        let value = rust::resource_descriptor_shape(device, desc.as_ref().ok_or(INVALID)?)?;
        out.write(value);
        Ok(())
    })
}

/// # Safety
/// Live device/resources, readable description and disjoint writable output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_sampler_descriptor_shape(
    device: *const Device,
    desc: *const SamplerDesc,
    out: *mut DescriptorShape,
) -> Status {
    boundary(|| unsafe {
        if out.is_null() {
            return Err(INVALID);
        }
        let device = rust::DeviceHandle::from_raw(device.cast_mut()).ok_or(INVALID)?;
        let value = rust::sampler_descriptor_shape(device, desc.as_ref().ok_or(INVALID)?)?;
        out.write(value);
        Ok(())
    })
}

/// # Safety
/// Live device/backing, readable description and disjoint writable output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_heap_create(
    device: *mut Device,
    desc: *const HeapDesc,
    out: *mut *mut DescriptorHeap,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| unsafe {
        let device = rust::DeviceHandle::from_raw(device).ok_or(INVALID)?;
        let value = rust::heap_create(device, desc.as_ref().ok_or(INVALID)?)?;
        out.write(value.as_raw());
        Ok(())
    })
}
/// # Safety
/// NULL or live uniquely owned heap with no remaining recorded/pending uses.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_heap_destroy(heap: *mut DescriptorHeap) {
    if let Some(heap) = unsafe { rust::DescriptorHeapHandle::from_raw(heap) } {
        unsafe {
            rust::heap_destroy(heap);
        }
    }
}
/// # Safety
/// Live heap; disjoint writable outputs.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_heap_info(
    heap: *const DescriptorHeap,
    desc: *mut HeapDesc,
    requirements: *mut HeapRequirements,
) {
    let (d, r) = unsafe { (&*heap).info() };
    unsafe {
        desc.write(d);
        requirements.write(r);
    }
}
/// # Safety
/// Live heaps/resources, readable arrays and disjoint writable scratch; ordered mutation.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_copy_descriptors(
    device: *const Device,
    count: u32,
    copies: *const DescriptorCopy,
    scratch: HostSpan,
) -> Status {
    boundary(|| unsafe {
        device
            .as_ref()
            .ok_or(INVALID)?
            .copy_descriptors(count, copies, scratch)
    })
}
