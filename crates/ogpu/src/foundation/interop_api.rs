//! C adapters for typed Vulkan interoperability.
use super::*;
/// # Safety
/// Live object/device and writable nonoverlapping output. Handles remain borrowed.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_device_native_vulkan(
    object: *const Device,
    out: *mut VulkanDevice,
) -> Status {
    boundary(|| {
        let object = unsafe { object.as_ref() }.ok_or(INVALID)?;
        let out = unsafe { out.as_mut() }.ok_or(INVALID)?;
        *out = object.native_vulkan()?;
        Ok(())
    })
}
/// # Safety
/// Live object/device and writable nonoverlapping output. Handles remain borrowed.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_queue_native_vulkan(
    object: *const Queue,
    out: *mut VulkanQueue,
) -> Status {
    boundary(|| {
        let object = unsafe { object.as_ref() }.ok_or(INVALID)?;
        let out = unsafe { out.as_mut() }.ok_or(INVALID)?;
        *out = object.native_vulkan()?;
        Ok(())
    })
}
/// # Safety
/// Live object/device and writable nonoverlapping output. Handles remain borrowed.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_memory_native_vulkan(
    object: *const Memory,
    out: *mut VulkanMemory,
) -> Status {
    boundary(|| {
        let object = unsafe { object.as_ref() }.ok_or(INVALID)?;
        let out = unsafe { out.as_mut() }.ok_or(INVALID)?;
        *out = object.native_vulkan()?;
        Ok(())
    })
}
/// # Safety
/// Live object/device and writable nonoverlapping output. Handles remain borrowed.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_image_native_vulkan(
    object: *const Image,
    out: *mut VulkanImage,
) -> Status {
    boundary(|| {
        let object = unsafe { object.as_ref() }.ok_or(INVALID)?;
        let out = unsafe { out.as_mut() }.ok_or(INVALID)?;
        *out = object.native_vulkan()?;
        Ok(())
    })
}
/// # Safety
/// Live object/device and writable nonoverlapping output. Handles remain borrowed.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_timeline_native_vulkan(
    object: *const Timeline,
    out: *mut VulkanSemaphore,
) -> Status {
    boundary(|| {
        let object = unsafe { object.as_ref() }.ok_or(INVALID)?;
        let out = unsafe { out.as_mut() }.ok_or(INVALID)?;
        *out = object.native_vulkan()?;
        Ok(())
    })
}
/// # Safety
/// Live object/device and writable nonoverlapping output. Handles remain borrowed.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_binary_native_vulkan(
    object: *const Binary,
    out: *mut VulkanSemaphore,
) -> Status {
    boundary(|| {
        let object = unsafe { object.as_ref() }.ok_or(INVALID)?;
        let out = unsafe { out.as_mut() }.ok_or(INVALID)?;
        *out = object.native_vulkan()?;
        Ok(())
    })
}
/// # Safety
/// Exact same-device native metadata, live parent/handles, readable records and
/// writable nonoverlapping output. Destruction ownership transfers only on OK.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_memory_import_vulkan(
    device: *mut Device,
    desc: *const MemoryDesc,
    value: *const VulkanMemory,
    ownership: u32,
    out: *mut *mut Memory,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let device = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        if desc.is_null() {
            return Err(INVALID);
        }
        unsafe { desc.cast::<Record>().read() }.validate::<MemoryDesc>(MEMORY_DESC)?;
        let desc = unsafe { &*desc };
        let value = unsafe { value.as_ref() }.ok_or(INVALID)?;
        let result = unsafe { rust::memory_import_vulkan(device, desc, value, ownership)? };
        unsafe {
            out.write(result.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// Exact same-device native metadata, live parent/handles, readable records and
/// writable nonoverlapping output. Destruction ownership transfers only on OK.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_image_import_vulkan(
    device: *mut Device,
    desc: *const ImageDesc,
    value: *const VulkanImage,
    ownership: u32,
    out: *mut *mut Image,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let device = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        if desc.is_null() {
            return Err(INVALID);
        }
        unsafe { desc.cast::<Record>().read() }.validate::<ImageDesc>(IMAGE_DESC)?;
        let desc = unsafe { &*desc };
        let value = unsafe { value.as_ref() }.ok_or(INVALID)?;
        let result = unsafe { rust::image_import_vulkan(device, desc, value, ownership)? };
        unsafe {
            out.write(result.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// Exact same-device native metadata, live parent/handles, readable records and
/// writable nonoverlapping output. Destruction ownership transfers only on OK.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_timeline_import_vulkan(
    device: *mut Device,
    value: *const VulkanSemaphore,
    ownership: u32,
    out: *mut *mut Timeline,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let device = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let value = unsafe { value.as_ref() }.ok_or(INVALID)?;
        let result = unsafe { rust::timeline_import_vulkan(device, value, ownership)? };
        unsafe {
            out.write(result.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// Exact same-device native metadata, live parent/handles, readable records and
/// writable nonoverlapping output. Destruction ownership transfers only on OK.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_binary_import_vulkan(
    device: *mut Device,
    value: *const VulkanSemaphore,
    ownership: u32,
    out: *mut *mut Binary,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let device = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let value = unsafe { value.as_ref() }.ok_or(INVALID)?;
        let result = unsafe { rust::binary_import_vulkan(device, value, ownership)? };
        unsafe {
            out.write(result.as_raw());
        }
        Ok(())
    })
}

/// # Safety
/// Exact native instance metadata, selected loader and writable output. Instance
/// remains borrowed and must outlive all resulting discovery/device children.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_discovery_import_vulkan(
    value: *const VulkanInstance,
    out: *mut *mut Discovery,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let value = unsafe { value.as_ref() }.ok_or(INVALID)?;
        let discovery = unsafe { rust::discovery_import_vulkan(value)? };
        unsafe {
            out.write(Box::into_raw(discovery));
        }
        Ok(())
    })
}
/// # Safety
/// Trusted native device metadata and enabled-feature/queue declarations, live
/// parents, readable input arrays and writable nonoverlapping output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_device_import_vulkan(
    adapter: *const Adapter,
    desc: *const DeviceDesc,
    value: *const VulkanDevice,
    ownership: u32,
    out: *mut *mut Device,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let adapter = unsafe { adapter.as_ref() }.ok_or(INVALID)?;
        let value = unsafe { value.as_ref() }.ok_or(INVALID)?;
        if desc.is_null() {
            return Err(INVALID);
        }
        unsafe { desc.cast::<Record>().read() }.validate::<DeviceDesc>(DEVICE_DESC)?;
        let desc = unsafe { &*desc };
        if desc.reserved != 0
            || desc.queue_request_count == 0
            || desc.queues.is_null()
            || desc.queue_request_count as usize > adapter.snapshot.queues.len()
        {
            return Err(INVALID);
        }
        let requests =
            unsafe { std::slice::from_raw_parts(desc.queues, desc.queue_request_count as usize) };
        let device = unsafe {
            rust::device_import_vulkan(
                adapter,
                requests,
                desc.required_features.as_slice()?,
                value,
                ownership,
            )?
        };
        unsafe {
            out.write(device.as_raw());
        }
        Ok(())
    })
}
