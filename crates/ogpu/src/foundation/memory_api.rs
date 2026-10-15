//! C boundary for backing and ranges; no ownership conferred by a Span.
use super::*;

unsafe fn description<'a>(desc: *const MemoryDesc) -> Result<&'a MemoryDesc, Status> {
    if desc.is_null() {
        return Err(INVALID);
    }
    // SAFETY: C caller supplies at least a Record; validate before reading more.
    unsafe { desc.cast::<Record>().read() }.validate::<MemoryDesc>(MEMORY_DESC)?;
    Ok(unsafe { &*desc })
}

/// # Safety
/// See include/ogpu_next.h: live device, readable description/arrays, writable output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_memory_requirements(
    device: *const Device,
    desc: *const MemoryDesc,
    out: *mut Requirements,
) -> Status {
    boundary(|| {
        let d = unsafe { rust::DeviceHandle::from_raw(device.cast_mut()) }.ok_or(INVALID)?;
        let desc = unsafe { description(desc)? };
        let out = unsafe { out.as_mut() }.ok_or(INVALID)?;
        unsafe { rust::memory_requirements(d, desc, out) }
    })
}
/// # Safety
/// Device outlives the memory. See include/ogpu_next.h for description/output rules.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_memory_create(
    device: *mut Device,
    desc: *const MemoryDesc,
    out: *mut *mut Memory,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let d = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let desc = unsafe { description(desc)? };
        let memory = unsafe { rust::memory_create(d, desc)? };
        unsafe {
            out.write(memory.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// No views, recorded or pending uses may remain; device is live. Null is allowed.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_memory_destroy(memory: *mut Memory) {
    if let Some(memory) = unsafe { rust::MemoryHandle::from_raw(memory) } {
        unsafe {
            rust::memory_destroy(memory);
        }
    }
}
/// # Safety
/// Live backing and writable output; see include/ogpu_next.h. Address owns nothing.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_memory_address(span: Span, out: *mut u64) -> Status {
    if out.is_null() {
        return INVALID;
    }
    boundary(|| {
        let address = unsafe { rust::memory_address(span)? };
        unsafe {
            out.write(address);
        }
        Ok(())
    })
}
/// # Safety
/// Caller externally serializes mapping changes and retains backing for every view.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_memory_map(span: Span, out: *mut Mapping) -> Status {
    if out.is_null() {
        return INVALID;
    }
    boundary(|| {
        let mapping = unsafe { rust::memory_map(span)? };
        unsafe {
            out.write(mapping);
        }
        Ok(())
    })
}
/// # Safety
/// See include/ogpu_next.h. No view/cache user may race this operation. Null allowed.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_memory_unmap(memory: *mut Memory) {
    if let Some(memory) = unsafe { rust::MemoryHandle::from_raw(memory) } {
        unsafe {
            rust::memory_unmap(memory);
        }
    }
}
/// # Safety
/// Caller orders CPU/GPU access including neighboring bytes in cache atoms.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_memory_flush(span: Span) -> Status {
    boundary(|| unsafe { rust::memory_flush(span) })
}
/// # Safety
/// Caller orders CPU/GPU access including neighboring bytes in cache atoms.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_memory_invalidate(span: Span) -> Status {
    boundary(|| unsafe { rust::memory_invalidate(span) })
}
