//! Unsafe, explicitly native handles. No retaining, submission or state restoration.
use super::*;
impl Device {
    pub(in crate::foundation) fn native_vulkan(&self) -> Result<VulkanDevice, Status> {
        self.ready()?;
        Ok(VulkanDevice {
            instance: self._instance.handle().cast(),
            physical_device: self.physical.cast(),
            device: self.handle.cast(),
            instance_api_version: self._instance.api_version(),
            instance_flags: if self._instance.wayland_enabled() {
                VULKAN_INSTANCE_WAYLAND
            } else {
                0
            },
        })
    }
    pub(in crate::foundation) fn import_vulkan(
        adapter: &Adapter,
        requests: &[QueueRequest],
        enabled: FeatureBits,
        value: VulkanDevice,
        ownership: u32,
    ) -> Result<Box<Self>, Status> {
        if value.instance != adapter.instance.handle().cast()
            || value.physical_device != adapter.physical.cast()
            || value.device.is_null()
            || ownership > NATIVE_OWN
            || value.instance_api_version != adapter.instance.api_version()
            || value.instance_flags
                != if adapter.instance.wayland_enabled() {
                    VULKAN_INSTANCE_WAYLAND
                } else {
                    0
                }
        {
            return Err(INVALID);
        }
        let mut result = Self::prepare(adapter, requests, enabled)?;
        result.owned = false;
        result.handle = value.device.cast();
        result.attach_queues(requests);
        if result.queues.iter().any(|q| q.handle.is_null()) {
            return Err(BACKEND_ERROR);
        }
        result.owned = ownership == NATIVE_OWN;
        Ok(result)
    }
}
impl Queue {
    pub(in crate::foundation) fn native_vulkan(&self) -> Result<VulkanQueue, Status> {
        unsafe { &*self.device.load(Ordering::Relaxed) }.ready()?;
        Ok(VulkanQueue {
            queue: self.handle.cast(),
            domain: self.domain,
            index: self.index,
        })
    }
}
impl Timeline {
    pub(in crate::foundation) fn native_vulkan(&self) -> Result<VulkanSemaphore, Status> {
        unsafe { &*self.device }.ready()?;
        Ok(VulkanSemaphore {
            semaphore: self.handle as u64,
        })
    }
    pub(in crate::foundation) fn import_vulkan(
        d: &Device,
        value: VulkanSemaphore,
        ownership: u32,
    ) -> Result<Box<Self>, Status> {
        d.ready()?;
        if value.semaphore == 0 || ownership > NATIVE_OWN {
            return Err(INVALID);
        }
        Ok(Box::new(Self {
            device: d,
            handle: value.semaphore as vk::VkSemaphore,
            owned: ownership == NATIVE_OWN,
        }))
    }
}
