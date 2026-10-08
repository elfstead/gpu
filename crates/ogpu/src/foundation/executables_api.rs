//! Trusted artifact and argument ABI boundary. Native compilation is preparation.
use super::*;

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
