//! Shared C boundary: panic containment, diagnostics and cleared creation outputs.
use crate::{Error, OgpuError, OgpuResult, INTERNAL_ERROR, INVALID_ARGUMENT, SUCCESS};
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
};

pub(crate) fn native_scope<T>(f: impl FnOnce() -> T) -> T {
    #[cfg(target_os = "macos")]
    {
        objc::rc::autoreleasepool(f)
    }
    #[cfg(not(target_os = "macos"))]
    {
        f()
    }
}
// SAFETY (for callers of these private helpers): error, when non-NULL, is writable
// and does not overlap inputs. The outer FFI functions document all pointer contracts.
pub(crate) unsafe fn call(
    error: *mut OgpuError,
    f: impl FnOnce() -> Result<(), Error>,
) -> OgpuResult {
    let result = native_scope(|| catch_unwind(AssertUnwindSafe(f))).unwrap_or_else(|_| {
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

pub(crate) unsafe fn create<T>(
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

pub(crate) fn required<T>(pointer: *const T) -> Result<(), Error> {
    if pointer.is_null() {
        Err(Error::new(INVALID_ARGUMENT, "NULL required argument"))
    } else {
        Ok(())
    }
}
