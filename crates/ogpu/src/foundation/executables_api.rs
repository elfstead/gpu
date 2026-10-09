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
        let device = unsafe { device.as_ref() }.ok_or(INVALID)?;
        let desc = unsafe { native::executable_record(desc.cast(), EXECUTABLE_CACHE_DESC)? };
        let cache = unsafe { ExecutableCache::create(device, desc)? };
        unsafe {
            out.write(Box::into_raw(cache));
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
    boundary(|| unsafe { cache.as_ref().ok_or(INVALID)?.data(size, data) })
}
/// # Safety
/// Live caches and source array; exclusive destination, no source mutation/destruction.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_executable_cache_merge(
    destination: *mut ExecutableCache,
    count: u32,
    sources: *const *mut ExecutableCache,
) -> Status {
    boundary(|| unsafe { destination.as_ref().ok_or(INVALID)?.merge(count, sources) })
}
/// # Safety
/// Live device; exclusive destruction excludes all cache calls and preparation.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_executable_cache_destroy(cache: *mut ExecutableCache) {
    if !cache.is_null() {
        unsafe {
            drop(Box::from_raw(cache));
        }
    }
}

/// # Safety
/// See header: valid artifact/metadata and live parent device, disjoint output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_executable_create(
    device: *mut Device,
    desc: *const ExecutableDesc,
    out: *mut *mut Executable,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let d = unsafe { device.as_ref() }.ok_or(INVALID)?;
        let desc = unsafe { native::executable_record(desc.cast(), EXECUTABLE_DESC)? };
        let executable = unsafe { Executable::create(d, desc)? };
        unsafe {
            out.write(Box::into_raw(executable));
        }
        Ok(())
    })
}
/// # Safety
/// No recorded future/pending use; live device, exclusive destruction. Never waits.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_executable_destroy(executable: *mut Executable) {
    if !executable.is_null() {
        unsafe {
            drop(Box::from_raw(executable));
        }
    }
}
