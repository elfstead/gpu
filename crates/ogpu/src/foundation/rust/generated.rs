//! Native generated-command operations, sharing the C contract without calling C.
use super::*;
/// # Safety
/// Live device, argument layout and readable description/token array. The device
/// and borrowed argument layout outlive the generated layout and every recorded,
/// preprocessed or pending use. No token array is retained.
pub unsafe fn generated_layout_create(
    device: DeviceHandle,
    desc: &GeneratedLayoutDesc,
) -> Result<GeneratedLayoutHandle> {
    Ok(GeneratedLayoutHandle(NonNull::from(Box::leak(unsafe {
        native::GeneratedLayout::create(device.get(), desc)?
    }))))
}
/// # Safety
/// Live device and initial executable, externally synchronized as specified in the header.
pub unsafe fn generated_set_create(
    device: DeviceHandle,
    desc: &GeneratedSetDesc,
) -> Result<GeneratedSetHandle> {
    Ok(GeneratedSetHandle(NonNull::from(Box::leak(unsafe {
        native::GeneratedSet::create(device.get(), desc)?
    }))))
}
/// # Safety
/// Live set/executables; exclusive set access, no pending reference to updated slots.
/// Caller guarantees native pipeline compatibility and re-queries storage after updates.
pub unsafe fn generated_set_update(
    set: GeneratedSetHandle,
    writes: &[GeneratedSetWrite],
) -> Result<()> {
    count(writes.len())?;
    unsafe { set.get().update(writes) }
}
/// # Safety
/// Live device/layout/argument layout/set/executable; excludes set updates and
/// destruction. Query each distinct work pairing before recording: this checks
/// native argument mapping compatibility, which the recording path trusts.
pub unsafe fn generated_requirements(
    device: DeviceHandle,
    work: &GeneratedWork,
) -> Result<GeneratedRequirements> {
    unsafe { device.get().generated_requirements(work) }
}
/// # Safety
/// No recorded future/pending use; live parent. Destruction never waits.
pub unsafe fn generated_layout_destroy(layout: GeneratedLayoutHandle) {
    unsafe {
        drop(Box::from_raw(layout.as_raw()));
    }
}
/// # Safety
/// No recorded future/pending use; live parent and borrowed initial executable.
pub unsafe fn generated_set_destroy(set: GeneratedSetHandle) {
    unsafe {
        drop(Box::from_raw(set.as_raw()));
    }
}
