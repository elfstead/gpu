//! Trusted artifact and argument ABI boundary. Native compilation is preparation.
use super::*;

/// # Safety
/// Live parent, valid initial native cache bytes, disjoint output; see C header.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_executable_cache_create(
    device: *mut Device,
    desc: *const ExecutableCacheDesc,
    out: *mut *mut ExecutableCache,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let device = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let desc = unsafe { native::executable_record(desc.cast(), EXECUTABLE_CACHE_DESC)? };
        let cache = unsafe { rust::executable_cache_create(device, desc)? };
        unsafe {
            out.write(cache.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// Live cache; exclusive against cache mutation, disjoint writable size/data.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_executable_cache_data(
    cache: *mut ExecutableCache,
    size: *mut usize,
    data: *mut std::ffi::c_void,
) -> Status {
    boundary(|| unsafe {
        rust::executable_cache_data_raw(
            rust::ExecutableCacheHandle::from_raw(cache).ok_or(INVALID)?,
            size,
            data,
        )
    })
}
/// # Safety
/// Live caches and source array; exclusive destination, no source mutation/destruction.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_executable_cache_merge(
    destination: *mut ExecutableCache,
    count: u32,
    sources: *const *mut ExecutableCache,
) -> Status {
    boundary(|| unsafe {
        rust::executable_cache_merge_raw(
            rust::ExecutableCacheHandle::from_raw(destination).ok_or(INVALID)?,
            count,
            sources,
        )
    })
}
/// # Safety
/// Live device; exclusive destruction excludes all cache calls and preparation.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_executable_cache_destroy(cache: *mut ExecutableCache) {
    if let Some(cache) = unsafe { rust::ExecutableCacheHandle::from_raw(cache) } {
        unsafe {
            rust::executable_cache_destroy(cache);
        }
    }
}

/// # Safety
/// See header: valid artifact/metadata and live parent device, disjoint output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_executable_create(
    device: *mut Device,
    desc: *const ExecutableDesc,
    layout: *mut ArgumentLayout,
    out: *mut *mut Executable,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let d = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let desc = unsafe { native::executable_record(desc.cast(), EXECUTABLE_DESC)? };
        let layout = unsafe { rust::ArgumentLayoutHandle::from_raw(layout) }.ok_or(INVALID)?;
        let executable = unsafe { rust::executable_create(d, desc, layout)? };
        unsafe {
            out.write(executable.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// No recorded future/pending use; live device, exclusive destruction. Never waits.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_executable_destroy(executable: *mut Executable) {
    if let Some(executable) = unsafe { rust::ExecutableHandle::from_raw(executable) } {
        unsafe {
            rust::executable_destroy(executable);
        }
    }
}

/// # Safety
/// Live device, readable fields, disjoint writable output. See the C header.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_argument_layout_create(
    device: *mut Device,
    desc: *const ArgumentLayoutDesc,
    out: *mut *mut ArgumentLayout,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let device = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let desc = unsafe { desc.as_ref() }.ok_or(INVALID)?;
        let layout = unsafe { rust::argument_layout_create(device, desc)? };
        unsafe {
            out.write(layout.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// NULL or uniquely owned layout; live device; no borrowing executables/recorded uses.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_argument_layout_destroy(layout: *mut ArgumentLayout) {
    if let Some(layout) = unsafe { rust::ArgumentLayoutHandle::from_raw(layout) } {
        unsafe {
            rust::argument_layout_destroy(layout);
        }
    }
}
/// # Safety
/// Live layout/device, disjoint writable output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_argument_layout_info(
    layout: *const ArgumentLayout,
    out: *mut ArgumentStateInfo,
) {
    unsafe {
        out.write((&*layout).state_info());
    }
}
