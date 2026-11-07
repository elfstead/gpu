//! Explicit Vulkan handoff; the native caller owns state and lifetime proofs.
use super::*;
/// # Safety
/// Live object/device; native use obeys shared synchronization and lifetime rules.
pub unsafe fn device_native_vulkan(object: DeviceHandle) -> Result<VulkanDevice> {
    unsafe { object.get() }.native_vulkan()
}
/// # Safety
/// Live object/device; native use obeys shared synchronization and lifetime rules.
pub unsafe fn queue_native_vulkan(object: QueueHandle) -> Result<VulkanQueue> {
    unsafe { object.get() }.native_vulkan()
}
/// # Safety
/// Live object/device; native use obeys shared synchronization and lifetime rules.
pub unsafe fn memory_native_vulkan(object: MemoryHandle) -> Result<VulkanMemory> {
    unsafe { object.get() }.native_vulkan()
}
/// # Safety
/// Live object/device; native use obeys shared synchronization and lifetime rules.
pub unsafe fn image_native_vulkan(object: ImageHandle) -> Result<VulkanImage> {
    unsafe { object.get() }.native_vulkan()
}
/// # Safety
/// Live object/device; native use obeys shared synchronization and lifetime rules.
pub unsafe fn timeline_native_vulkan(object: TimelineHandle) -> Result<VulkanSemaphore> {
    unsafe { object.get() }.native_vulkan()
}
/// # Safety
/// Live object/device; native use obeys shared synchronization and lifetime rules.
pub unsafe fn binary_native_vulkan(object: BinaryHandle) -> Result<VulkanSemaphore> {
    unsafe { object.get() }.native_vulkan()
}
/// # Safety
/// Same-device native handles with exact declared metadata. Ownership transfers
/// only on success; native parents outlive wrapper and every recorded/pending use.
pub unsafe fn memory_import_vulkan(
    device: DeviceHandle,
    desc: &MemoryDesc,
    value: &VulkanMemory,
    ownership: u32,
) -> Result<MemoryHandle> {
    Ok(MemoryHandle(NonNull::from(Box::leak(unsafe {
        super::super::Memory::import_vulkan(device.get(), desc, *value, ownership)?
    }))))
}
/// # Safety
/// Same-device native handles with exact declared metadata. Ownership transfers
/// only on success; native parents outlive wrapper and every recorded/pending use.
pub unsafe fn image_import_vulkan(
    device: DeviceHandle,
    desc: &ImageDesc,
    value: &VulkanImage,
    ownership: u32,
) -> Result<ImageHandle> {
    Ok(ImageHandle(NonNull::from(Box::leak(unsafe {
        super::super::Image::import_vulkan(device.get(), desc, *value, ownership)?
    }))))
}
/// # Safety
/// Same-device native handles with exact declared metadata. Ownership transfers
/// only on success; native parents outlive wrapper and every recorded/pending use.
pub unsafe fn timeline_import_vulkan(
    device: DeviceHandle,
    value: &VulkanSemaphore,
    ownership: u32,
) -> Result<TimelineHandle> {
    Ok(TimelineHandle(NonNull::from(Box::leak(unsafe {
        super::super::Timeline::import_vulkan(device.get(), *value, ownership)?
    }))))
}
/// # Safety
/// Same-device native handles with exact declared metadata. Ownership transfers
/// only on success; native parents outlive wrapper and every recorded/pending use.
pub unsafe fn binary_import_vulkan(
    device: DeviceHandle,
    value: &VulkanSemaphore,
    ownership: u32,
) -> Result<BinaryHandle> {
    Ok(BinaryHandle(NonNull::from(Box::leak(unsafe {
        super::super::Binary::import_vulkan(device.get(), *value, ownership)?
    }))))
}

/// Borrow a caller-created instance. No native ownership transfers, even when
/// the returned discovery or its devices are dropped.
/// # Safety
/// Instance belongs to the selected loader; API/extension metadata is exact.
/// Native instance outlives discovery and all devices/objects derived from it.
pub unsafe fn discovery_import_vulkan(
    value: &VulkanInstance,
) -> Result<Box<super::super::Discovery>> {
    if value.instance.is_null() || value.flags & !VULKAN_INSTANCE_WAYLAND != 0 {
        return Err(INVALID);
    }
    let instance = unsafe {
        Instance::import_vulkan(
            value.instance.cast(),
            value.api_version,
            value.flags & VULKAN_INSTANCE_WAYLAND != 0,
        )
    }
    .map_err(super::super::loader_error)?;
    super::discover_instance(Arc::new(instance))
}
/// Wrap a caller-created device. There are no native creation or enabling calls.
/// # Safety
/// Device belongs to the supplied adapter/instance with OGPU's complete Vulkan
/// baseline and declared optional features already enabled, and declared queues
/// already created with ordinary (unprotected) flags. Metadata must be exact.
/// Ownership transfers only on success; the instance outlives all device uses.
pub unsafe fn device_import_vulkan(
    adapter: &super::super::Adapter,
    requests: &[QueueRequest],
    enabled: &[u32],
    value: &VulkanDevice,
    ownership: u32,
) -> Result<DeviceHandle> {
    validate_requests(&adapter.snapshot.queues, requests)?;
    let enabled = features::encode(enabled)?;
    validate_feature_dependencies(enabled)?;
    if adapter.snapshot.features.baseline_supported == 0
        || enabled & !adapter.snapshot.features.available != 0
    {
        return Err(UNSUPPORTED);
    }
    Ok(DeviceHandle(NonNull::from(Box::leak(
        native::Device::import_vulkan(adapter, requests, enabled, *value, ownership)?,
    ))))
}
