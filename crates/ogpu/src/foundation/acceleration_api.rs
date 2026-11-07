//! C adapters for the same explicit acceleration operations as native Rust.
use super::*;
/// # Safety
/// Disjoint writable output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_acceleration_scratch_requirements(
    builds: u32,
    geometries: u32,
    out: *mut HostRequirements,
) -> Status {
    boundary(|| {
        let out = unsafe { out.as_mut() }.ok_or(INVALID)?;
        *out = rust::acceleration_scratch_requirements(builds, geometries)?;
        Ok(())
    })
}
/// # Safety
/// Live device, readable versioned description/arrays and disjoint output/scratch.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_acceleration_requirements(
    device: *mut Device,
    desc: *const AccelerationBuildDesc,
    scratch: HostSpan,
    out: *mut AccelerationRequirements,
) -> Status {
    boundary(|| {
        let d = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let desc = unsafe { native::executable_record(desc.cast(), ACCELERATION_BUILD_DESC)? };
        let out = unsafe { out.as_mut() }.ok_or(INVALID)?;
        *out = unsafe { rust::acceleration_requirements(d, desc, scratch)? };
        Ok(())
    })
}
/// # Safety
/// Live backing/device and disjoint output, retained for all object/recorded uses.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_acceleration_create(
    device: *mut Device,
    desc: *const AccelerationDesc,
    out: *mut *mut Acceleration,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let d = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let desc = unsafe { native::executable_record(desc.cast(), ACCELERATION_DESC)? };
        unsafe {
            out.write(rust::acceleration_create(d, desc)?.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// Null or a live uniquely destroyed object with all uses retired. Never waits.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_acceleration_destroy(object: *mut Acceleration) {
    if let Some(h) = unsafe { rust::AccelerationHandle::from_raw(object) } {
        unsafe {
            rust::acceleration_destroy(h);
        }
    }
}
/// # Safety
/// Live object and disjoint writable output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_acceleration_address(
    object: *const Acceleration,
    out: *mut u64,
) -> Status {
    boundary(|| {
        let h = unsafe { rust::AccelerationHandle::from_raw(object.cast_mut()) }.ok_or(INVALID)?;
        let out = unsafe { out.as_mut() }.ok_or(INVALID)?;
        *out = unsafe { rust::acceleration_address(h)? };
        Ok(())
    })
}
/// # Safety
/// Live device, readable native 32-byte version header and disjoint writable output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_acceleration_compatibility(
    device: *mut Device,
    version: Bytes,
    out: *mut u32,
) -> Status {
    boundary(|| {
        let d = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let out = unsafe { out.as_mut() }.ok_or(INVALID)?;
        if version.size != 32 || version.data.is_null() {
            return Err(INVALID);
        }
        *out = u32::from(unsafe {
            rust::acceleration_compatibility(
                d,
                std::slice::from_raw_parts(version.data.cast(), 32),
            )?
        });
        Ok(())
    })
}
