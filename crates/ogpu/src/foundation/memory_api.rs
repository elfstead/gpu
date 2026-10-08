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
        let d = unsafe { device.as_ref() }.ok_or(INVALID)?;
        let desc = unsafe { description(desc)? };
        let out = unsafe { out.as_mut() }.ok_or(INVALID)?;
        unsafe { d.memory_requirements(desc, out) }
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
        let d = unsafe { device.as_ref() }.ok_or(INVALID)?;
        let desc = unsafe { description(desc)? };
        let memory = unsafe { Memory::create(d, desc)? };
        unsafe {
            out.write(Box::into_raw(memory));
        }
        Ok(())
    })
}
/// # Safety
/// No views, recorded or pending uses may remain; device is live. Null is allowed.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_memory_destroy(memory: *mut Memory) {
    if !memory.is_null() {
        unsafe {
            drop(Box::from_raw(memory));
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
        let m = unsafe { span.memory.as_ref() }.ok_or(INVALID)?;
        let address = m.address(span.offset, span.size)?;
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
        let m = unsafe { span.memory.as_ref() }.ok_or(INVALID)?;
        let mapping = m.map(span.offset, span.size)?;
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
    if let Some(memory) = unsafe { memory.as_ref() } {
        memory.unmap();
    }
}
/// # Safety
/// Caller orders CPU/GPU access including neighboring bytes in cache atoms.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_memory_flush(span: Span) -> Status {
    boundary(|| {
        unsafe { span.memory.as_ref() }
            .ok_or(INVALID)?
            .cache(span.offset, span.size, true)
    })
}
/// # Safety
/// Caller orders CPU/GPU access including neighboring bytes in cache atoms.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_memory_invalidate(span: Span) -> Status {
    boundary(|| {
        unsafe { span.memory.as_ref() }
            .ok_or(INVALID)?
            .cache(span.offset, span.size, false)
    })
}
