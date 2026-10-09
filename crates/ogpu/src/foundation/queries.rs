//! Explicit query storage, with no automatic timers, receipts or CPU result cache.
use super::*;

pub struct QueryPool {
    pub(super) device: *const Device,
    pub(super) handle: vk::VkQueryPool,
    pub(super) kind: u32,
    count: u32,
}
impl QueryPool {
    pub(in crate::foundation) fn create(
        d: &Device,
        desc: &QueryPoolDesc,
    ) -> Result<Box<Self>, Status> {
        d.ready()?;
        desc.header.validate::<QueryPoolDesc>(QUERY_POOL_DESC)?;
        if desc.count == 0 {
            return Err(INVALID);
        }
        if desc.statistics != 0 {
            return Err(UNSUPPORTED);
        }
        let query_type = match desc.kind {
            1 if d
                .snapshot
                .queues
                .iter()
                .any(|q| q.count > 0 && q.timestamp_bits != 0) =>
            {
                vk::VkQueryType_VK_QUERY_TYPE_TIMESTAMP
            }
            2 if d.snapshot.features.enabled & RASTER != 0 => {
                vk::VkQueryType_VK_QUERY_TYPE_OCCLUSION
            }
            _ => return Err(UNSUPPORTED),
        };
        let info = vk::VkQueryPoolCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_QUERY_POOL_CREATE_INFO,
            queryType: query_type,
            queryCount: desc.count,
            ..Default::default()
        };
        let mut handle = ptr::null_mut();
        // Failure outputs are unspecified; adopt only on native success.
        unsafe {
            d.result((d.f.vkCreateQueryPool.unwrap())(
                d.handle,
                &info,
                ptr::null(),
                &mut handle,
            ))?;
        }
        Ok(Box::new(Self {
            device: d,
            handle,
            kind: desc.kind,
            count: desc.count,
        }))
    }
    pub(super) fn range(&self, d: *const Device, first: u32, count: u32) -> Result<(), Status> {
        if self.device != d || count == 0 || first >= self.count || count > self.count - first {
            return Err(INVALID);
        }
        Ok(())
    }
}
impl Drop for QueryPool {
    fn drop(&mut self) {
        let d = unsafe { &*self.device };
        unsafe {
            (d.f.vkDestroyQueryPool.unwrap())(d.handle, self.handle, ptr::null());
        }
    }
}

pub(super) fn result_layout(
    kind: u32,
    count: u32,
    stride: u64,
    flags: u32,
) -> Result<(u64, u64, u32), Status> {
    if flags & !15 != 0 {
        return Err(UNSUPPORTED);
    }
    if count == 0 || (kind == 1 && flags & 8 != 0) {
        return Err(INVALID);
    }
    let width = if flags & 1 != 0 { 8 } else { 4 };
    let record = width * if flags & 2 != 0 { 2 } else { 1 };
    if count > 1 && (stride == 0 || stride % width != 0) {
        return Err(INVALID);
    }
    let size = u64::from(count - 1)
        .checked_mul(stride)
        .and_then(|v| v.checked_add(record))
        .ok_or(INVALID)?;
    let native = (if flags & 1 != 0 {
        vk::VkQueryResultFlagBits_VK_QUERY_RESULT_64_BIT
    } else {
        0
    }) | (if flags & 2 != 0 {
        vk::VkQueryResultFlagBits_VK_QUERY_RESULT_WITH_AVAILABILITY_BIT
    } else {
        0
    }) | (if flags & 4 != 0 {
        vk::VkQueryResultFlagBits_VK_QUERY_RESULT_WAIT_BIT
    } else {
        0
    }) | (if flags & 8 != 0 {
        vk::VkQueryResultFlagBits_VK_QUERY_RESULT_PARTIAL_BIT
    } else {
        0
    });
    Ok((size, width, native))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    thread_local! {
        static FAILURE: Cell<u32> = const { Cell::new(0) };
        static CREATE: Cell<vk::PFN_vkCreateQueryPool> = const { Cell::new(None) };
        static DESTROY: Cell<vk::PFN_vkDestroyQueryPool> = const { Cell::new(None) };
        static DROPS: Cell<u32> = const { Cell::new(0) };
        static CALLS: Cell<u32> = const { Cell::new(0) };
    }
    unsafe extern "C" fn create(
        d: vk::VkDevice,
        info: *const vk::VkQueryPoolCreateInfo,
        a: *const vk::VkAllocationCallbacks,
        out: *mut vk::VkQueryPool,
    ) -> vk::VkResult {
        CALLS.set(CALLS.get() + 1);
        if FAILURE.get() != 0 {
            unsafe {
                out.write(ptr::dangling_mut());
            }
            return if FAILURE.get() == 1 {
                vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
            } else {
                vk::VkResult_VK_ERROR_DEVICE_LOST
            };
        }
        unsafe { CREATE.get().unwrap()(d, info, a, out) }
    }
    unsafe extern "C" fn destroy(
        d: vk::VkDevice,
        pool: vk::VkQueryPool,
        a: *const vk::VkAllocationCallbacks,
    ) {
        DROPS.set(DROPS.get() + 1);
        unsafe {
            DESTROY.get().unwrap()(d, pool, a);
        }
    }
    #[test]
    #[ignore = "requires modern Vulkan; query allocation failure cleanup and synthetic sticky loss"]
    fn gpu_foundation_queries() {
        use crate::foundation::{ogpu_next_query_pool_create, ogpu_next_query_pool_destroy};
        let instance = Arc::new(Instance::new().unwrap());
        let mut tested = 0;
        for physical in instance.physical_devices().unwrap() {
            let snapshot = snapshot(&instance, physical).unwrap();
            if snapshot.features.baseline_supported == 0 {
                continue;
            }
            let Some(q) = snapshot
                .queues
                .iter()
                .find(|q| q.count > 0 && q.timestamp_bits > 0 && q.flags & COMPUTE != 0)
            else {
                continue;
            };
            let domain = q.domain;
            let adapter = Adapter {
                instance: instance.clone(),
                physical,
                snapshot,
            };
            let mut device = Device::create(
                &adapter,
                &[QueueRequest {
                    domain,
                    count: 1,
                    priority: 0.5,
                }],
                0,
            )
            .unwrap();
            CREATE.set(device.f.vkCreateQueryPool);
            device.f.vkCreateQueryPool = Some(create);
            DESTROY.set(device.f.vkDestroyQueryPool);
            device.f.vkDestroyQueryPool = Some(destroy);
            let d = ptr::from_ref(&*device).cast_mut();
            let mut desc = QueryPoolDesc {
                header: Record::new::<QueryPoolDesc>(QUERY_POOL_DESC),
                kind: 1,
                count: 0,
                statistics: 0,
            };
            CALLS.set(0);
            DROPS.set(0);
            let mut out = ptr::dangling_mut();
            assert_eq!(
                unsafe { ogpu_next_query_pool_create(d, &desc, &mut out) },
                INVALID
            );
            assert!(out.is_null());
            assert_eq!(CALLS.get(), 0);
            desc.count = 2;
            for mode in [1, 0, 2] {
                FAILURE.set(mode);
                assert_eq!(
                    unsafe { ogpu_next_query_pool_create(d, &desc, &mut out) },
                    match mode {
                        1 => OUT_OF_MEMORY,
                        2 => DEVICE_LOST,
                        _ => OK,
                    }
                );
                assert_eq!(out.is_null(), mode != 0);
                unsafe {
                    ogpu_next_query_pool_destroy(out);
                }
            }
            assert_eq!(DROPS.get(), 1);
            assert_eq!(CALLS.get(), 3);
            assert_eq!(
                unsafe { ogpu_next_query_pool_create(d, &desc, &mut out) },
                DEVICE_LOST
            );
            assert!(out.is_null());
            assert_eq!(CALLS.get(), 3);
            tested += 1;
        }
        assert!(tested > 0, "No suitable device; not a passing skip");
    }
    #[test]
    fn result_ranges_and_flags_are_explicit() {
        assert_eq!(result_layout(1, 2, 24, 7).unwrap().0, 40);
        assert_eq!(result_layout(2, 1, 0, 2).unwrap().0, 8);
        assert_eq!(result_layout(2, 1, 0, 0).unwrap().0, 4);
        assert_eq!(result_layout(2, 2, 8, 3).unwrap().0, 24); // Preserve native overlapping strides.
        assert_eq!(result_layout(1, 1, 0, 8), Err(INVALID));
        assert_eq!(result_layout(2, 2, 4, 3), Err(INVALID));
        assert_eq!(result_layout(2, 2, 18, 3), Err(INVALID));
        assert_eq!(result_layout(2, 2, u64::MAX - 7, 3), Err(INVALID));
        assert_eq!(result_layout(2, 1, 0, 16), Err(UNSUPPORTED));
    }
}
