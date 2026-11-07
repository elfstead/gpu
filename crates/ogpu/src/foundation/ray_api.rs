//! C adapters for prepared ray execution.
use super::*;
/// # Safety
/// Live executable/device and writable disjoint output of declared size.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_ray_group_handles(
    executable: *const Executable,
    first: u32,
    count: u32,
    output: HostSpan,
) -> Status {
    boundary(|| {
        let e = unsafe { executable.as_ref() }.ok_or(INVALID)?;
        unsafe { e.ray_group_handles(first, count, output) }
    })
}
/// # Safety
/// Live executable/device, valid group/member and disjoint writable output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_ray_group_stack_size(
    executable: *const Executable,
    group: u32,
    member: u32,
    out: *mut u64,
) -> Status {
    boundary(|| {
        let e = unsafe { executable.as_ref() }.ok_or(INVALID)?;
        let out = unsafe { out.as_mut() }.ok_or(INVALID)?;
        *out = e.ray_group_stack_size(group, member)?;
        Ok(())
    })
}

/// # Safety
/// Live capture-enabled executable/device; writable disjoint output of declared size.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_ray_capture_handles(
    executable: *const Executable,
    first: u32,
    count: u32,
    output: HostSpan,
) -> Status {
    boundary(|| {
        let e = unsafe { executable.as_ref() }.ok_or(INVALID)?;
        unsafe { e.ray_capture_handles(first, count, output) }
    })
}
