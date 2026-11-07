//! Native Rust acceleration operations. Handles never retain backing or wait.
use super::*;
pub fn acceleration_scratch_requirements(builds: u32, geometries: u32) -> Result<HostRequirements> {
    native::acceleration::scratch_requirements(builds, geometries)
}
/// # Safety
/// Live device, readable description/arrays and disjoint suitably aligned scratch.
pub unsafe fn acceleration_requirements(
    device: DeviceHandle,
    desc: &AccelerationBuildDesc,
    scratch: HostSpan,
) -> Result<AccelerationRequirements> {
    unsafe { native::acceleration::requirements(device.get(), desc, scratch) }
}
/// # Safety
/// Live device and matching backing retained through the object and recorded uses.
pub unsafe fn acceleration_create(
    device: DeviceHandle,
    desc: &AccelerationDesc,
) -> Result<AccelerationHandle> {
    Ok(AccelerationHandle(NonNull::from(Box::leak(unsafe {
        super::super::Acceleration::create(device.get(), desc)?
    }))))
}
/// # Safety
/// No recorded/pending or host use of the object; backing and device remain live.
pub unsafe fn acceleration_destroy(object: AccelerationHandle) {
    unsafe {
        drop(Box::from_raw(object.as_raw()));
    }
}
/// # Safety
/// Live structure/device. Address is borrowed and remains subject to GPU lifetime.
pub unsafe fn acceleration_address(object: AccelerationHandle) -> Result<u64> {
    unsafe { object.get() }.address()
}
/// # Safety
/// Live device and the first 32 bytes of a native serialized version header.
pub unsafe fn acceleration_compatibility(device: DeviceHandle, version: &[u8]) -> Result<bool> {
    let d = unsafe { device.get() };
    native::acceleration::enabled(d)?;
    if version.len() != 32 {
        return Err(INVALID);
    }
    unsafe { d.acceleration_compatibility(version.as_ptr()) }
}
