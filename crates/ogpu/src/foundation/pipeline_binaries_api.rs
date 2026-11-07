//! C adapters share the direct Rust binary preparation implementation.
use super::*;
use native::pipeline_binaries as b;
/// # Safety
/// Live device, disjoint writable output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_pipeline_binary_identity(
    device: *mut Device,
    out: *mut ArtifactKey,
) -> Status {
    boundary(|| {
        let d = unsafe { device.as_ref() }.ok_or(INVALID)?;
        let out = unsafe { out.as_mut() }.ok_or(INVALID)?;
        *out = b::identity(d)?;
        Ok(())
    })
}
/// # Safety
/// Live device, intact native binary data/keys and disjoint writable output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_pipeline_binary_import(
    device: *mut Device,
    identity: *const ArtifactKey,
    key: *const ArtifactKey,
    data: Bytes,
    out: *mut *mut PipelineBinary,
) -> Status {
    boundary(|| {
        let out = unsafe { out.as_mut() }.ok_or(INVALID)?;
        *out = ptr::null_mut();
        let d = unsafe { device.as_ref() }.ok_or(INVALID)?;
        let identity = unsafe { identity.as_ref() }.ok_or(INVALID)?;
        let key = unsafe { key.as_ref() }.ok_or(INVALID)?;
        *out = Box::into_raw(unsafe { PipelineBinary::import(d, identity, key, data)? });
        Ok(())
    })
}
/// # Safety
/// Live binary/device, writable disjoint key/size and declared data capacity.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_pipeline_binary_export(
    binary: *const PipelineBinary,
    key: *mut ArtifactKey,
    size: *mut usize,
    data: *mut std::ffi::c_void,
) -> Status {
    boundary(|| {
        let b = unsafe { binary.as_ref() }.ok_or(INVALID)?;
        let key = unsafe { key.as_mut() }.ok_or(INVALID)?;
        let size = unsafe { size.as_mut() }.ok_or(INVALID)?;
        unsafe { b.export(key, size, data) }
    })
}
/// # Safety
/// Null or live uniquely destroyed binary, no concurrent use.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_pipeline_binary_destroy(binary: *mut PipelineBinary) {
    if !binary.is_null() {
        drop(unsafe { Box::from_raw(binary) });
    }
}
/// # Safety
/// Live executable/device, exclusive capture/release/destruction; valid outputs.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_executable_binary_capture(
    executable: *const Executable,
    capacity: u32,
    out: *mut *mut PipelineBinary,
    count: *mut u32,
) -> Status {
    boundary(|| {
        let count = unsafe { count.as_mut() }.ok_or(INVALID)?;
        *count = 0;
        unsafe {
            b::clear_outputs(capacity, out)?;
        }
        let e = unsafe { executable.as_ref() }.ok_or(INVALID)?;
        unsafe { b::capture(e, capacity, out, count) }
    })
}
/// # Safety
/// Live executable/device, exclusive capture/release/destruction.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_executable_capture_release(
    executable: *const Executable,
) -> Status {
    boundary(|| b::release(unsafe { executable.as_ref() }.ok_or(INVALID)?))
}
/// # Safety
/// Live device/layout, valid source description, disjoint writable key.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_executable_key(
    device: *mut Device,
    desc: *const ExecutableDesc,
    layout: *mut ArgumentLayout,
    out: *mut ArtifactKey,
) -> Status {
    boundary(|| {
        let out = unsafe { out.as_mut() }.ok_or(INVALID)?;
        let d = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let l = unsafe { rust::ArgumentLayoutHandle::from_raw(layout) }.ok_or(INVALID)?;
        let desc = unsafe { desc.as_ref() }.ok_or(INVALID)?;
        *out = unsafe { rust::executable_key(d, desc, l)? };
        Ok(())
    })
}
/// # Safety
/// Same source descriptor/layout contract as executable_key; disjoint outputs.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_executable_binary_lookup(
    device: *mut Device,
    desc: *const ExecutableDesc,
    layout: *mut ArgumentLayout,
    capacity: u32,
    out: *mut *mut PipelineBinary,
    count: *mut u32,
) -> Status {
    boundary(|| {
        let count = unsafe { count.as_mut() }.ok_or(INVALID)?;
        *count = 0;
        unsafe {
            b::clear_outputs(capacity, out)?;
        }
        let d = unsafe { device.as_ref() }.ok_or(INVALID)?;
        let desc = unsafe { desc.as_ref() }.ok_or(INVALID)?;
        let l = unsafe { layout.as_ref() }.ok_or(INVALID)?;
        unsafe {
            Executable::prepare(
                d,
                desc,
                l,
                b::Action::Lookup {
                    capacity,
                    out,
                    count,
                },
            )?;
        }
        Ok(())
    })
}
