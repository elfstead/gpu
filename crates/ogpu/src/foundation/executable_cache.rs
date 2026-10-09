//! Explicit cold preparation storage. Never consulted during command recording.
use super::*;

pub struct ExecutableCache {
    device: *const Device,
    handle: vk::VkPipelineCache,
}

fn compatible(data: &[u8], vendor: u32, device: u32, uuid: &[u8; 16]) -> Result<(), Status> {
    if data.len() < 32 {
        return Err(INVALID);
    }
    let word = |offset| u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
    if word(4) != 1 {
        return Err(UNSUPPORTED);
    }
    if word(0) != 32 {
        return Err(INVALID);
    }
    if word(8) != vendor || word(12) != device || data[16..32] != *uuid {
        return Err(UNSUPPORTED);
    }
    Ok(())
}

impl ExecutableCache {
    pub(in crate::foundation) unsafe fn create(
        d: &Device,
        desc: &ExecutableCacheDesc,
    ) -> Result<Box<Self>, Status> {
        d.ready()?;
        desc.header
            .validate::<ExecutableCacheDesc>(EXECUTABLE_CACHE_DESC)?;
        if desc.synchronization > 1 || desc.reserved != 0 {
            return Err(UNSUPPORTED);
        }
        if desc.synchronization == 1 && d.snapshot.features.enabled & CACHE_CONTROL == 0 {
            return Err(UNSUPPORTED);
        }
        if desc.initial.size > isize::MAX as usize
            || (desc.initial.size != 0 && desc.initial.data.is_null())
        {
            return Err(INVALID);
        }
        if desc.initial.size != 0 {
            let data = unsafe {
                std::slice::from_raw_parts(desc.initial.data.cast::<u8>(), desc.initial.size)
            };
            compatible(
                data,
                d.snapshot.info.vendor_id,
                d.snapshot.info.device_id,
                &d.snapshot.cache_uuid,
            )?;
        }
        let info = vk::VkPipelineCacheCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_CACHE_CREATE_INFO,
            flags: if desc.synchronization == 1 {
                vk::VkPipelineCacheCreateFlagBits_VK_PIPELINE_CACHE_CREATE_EXTERNALLY_SYNCHRONIZED_BIT
            } else {
                0
            },
            initialDataSize: desc.initial.size,
            pInitialData: desc.initial.data,
            ..Default::default()
        };
        let mut handle = ptr::null_mut();
        // Ordinary failed outputs are unspecified, unlike pipeline partial results.
        unsafe {
            d.result((d.f.vkCreatePipelineCache.unwrap())(
                d.handle,
                &info,
                ptr::null(),
                &mut handle,
            ))?;
        }
        Ok(Box::new(Self { device: d, handle }))
    }

    pub(super) unsafe fn optional(
        d: &Device,
        cache: *mut Self,
    ) -> Result<vk::VkPipelineCache, Status> {
        if let Some(cache) = unsafe { cache.as_ref() } {
            if !ptr::eq(cache.device, d) {
                return Err(INVALID);
            }
            Ok(cache.handle)
        } else {
            Ok(ptr::null_mut())
        }
    }

    pub(in crate::foundation) unsafe fn data(
        &self,
        size: *mut usize,
        data: *mut std::ffi::c_void,
    ) -> Result<(), Status> {
        if size.is_null() {
            return Err(INVALID);
        }
        let d = unsafe { &*self.device };
        d.ready()?;
        let result =
            unsafe { (d.f.vkGetPipelineCacheData.unwrap())(d.handle, self.handle, size, data) };
        if result == vk::VkResult_VK_INCOMPLETE {
            Err(CAPACITY)
        } else {
            d.result(result)
        }
    }
    pub(in crate::foundation) unsafe fn merge(
        &self,
        count: u32,
        sources: *const *mut Self,
    ) -> Result<(), Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        if count == 0 {
            return Ok(());
        }
        if sources.is_null() || count as u64 > isize::MAX as u64 / size_of::<*mut Self>() as u64 {
            return Err(INVALID);
        }
        let sources = unsafe { std::slice::from_raw_parts(sources, count as usize) };
        let mut native = Vec::new();
        native
            .try_reserve_exact(sources.len())
            .map_err(|_| OUT_OF_MEMORY)?;
        for &source in sources {
            let source = unsafe { source.as_ref() }.ok_or(INVALID)?;
            if source.device != self.device || source.handle == self.handle {
                return Err(INVALID);
            }
            native.push(source.handle);
        }
        unsafe {
            d.result((d.f.vkMergePipelineCaches.unwrap())(
                d.handle,
                self.handle,
                count,
                native.as_ptr(),
            ))
        }
    }
}
impl Drop for ExecutableCache {
    fn drop(&mut self) {
        let d = unsafe { &*self.device };
        unsafe {
            (d.f.vkDestroyPipelineCache.unwrap())(d.handle, self.handle, ptr::null());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    thread_local! {
        static FAILURE: Cell<u32> = const { Cell::new(0) };
        static CREATE: Cell<vk::PFN_vkCreatePipelineCache> = const { Cell::new(None) };
        static DESTROY: Cell<vk::PFN_vkDestroyPipelineCache> = const { Cell::new(None) };
        static DATA: Cell<vk::PFN_vkGetPipelineCacheData> = const { Cell::new(None) };
        static DROPS: Cell<u32> = const { Cell::new(0) };
        static CALLS: Cell<u32> = const { Cell::new(0) };
    }
    unsafe extern "C" fn create(
        d: vk::VkDevice,
        info: *const vk::VkPipelineCacheCreateInfo,
        a: *const vk::VkAllocationCallbacks,
        out: *mut vk::VkPipelineCache,
    ) -> vk::VkResult {
        CALLS.set(CALLS.get() + 1);
        if FAILURE.get() == 1 {
            unsafe {
                out.write(ptr::dangling_mut());
            }
            return vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY;
        }
        unsafe { CREATE.get().unwrap()(d, info, a, out) }
    }
    unsafe extern "C" fn destroy(
        d: vk::VkDevice,
        cache: vk::VkPipelineCache,
        a: *const vk::VkAllocationCallbacks,
    ) {
        DROPS.set(DROPS.get() + 1);
        unsafe {
            DESTROY.get().unwrap()(d, cache, a);
        }
    }
    unsafe extern "C" fn data(
        d: vk::VkDevice,
        cache: vk::VkPipelineCache,
        size: *mut usize,
        bytes: *mut std::ffi::c_void,
    ) -> vk::VkResult {
        CALLS.set(CALLS.get() + 1);
        match FAILURE.get() {
            2 => vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY,
            3 => vk::VkResult_VK_ERROR_DEVICE_LOST,
            _ => unsafe { DATA.get().unwrap()(d, cache, size, bytes) },
        }
    }
    fn description(synchronization: u32) -> ExecutableCacheDesc {
        ExecutableCacheDesc {
            header: Record::new::<ExecutableCacheDesc>(EXECUTABLE_CACHE_DESC),
            synchronization,
            reserved: 0,
            initial: Bytes {
                data: ptr::null(),
                size: 0,
            },
        }
    }
    fn concurrent_preparation(d: &Device) {
        let cache = unsafe { ExecutableCache::create(d, &description(0)).unwrap() };
        // Scope keeps cache/device/artifacts alive. Only native-synchronized cache is shared.
        let shared = ptr::from_ref(&*cache) as usize;
        let bytes = include_bytes!("../../../../examples/shaders/foundation-compute.comp.spv");
        let code: Vec<u32> = bytes
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        std::thread::scope(|scope| {
            for worker in 0..4 {
                let code = &code;
                scope.spawn(move || {
                    let local = if worker % 2 != 0 {
                        Some(unsafe {
                            ExecutableCache::create(
                                d,
                                &description(u32::from(
                                    d.snapshot.features.enabled & CACHE_CONTROL != 0,
                                )),
                            )
                            .unwrap()
                        })
                    } else {
                        None
                    };
                    let roots = [
                        RootSlot {
                            stages: 4,
                            offset: 0,
                            alignment: 8,
                        },
                        RootSlot {
                            stages: 4,
                            offset: 8,
                            alignment: 8,
                        },
                    ];
                    let abi = ArgumentInterface {
                        header: Record::new::<ArgumentInterface>(ARGUMENT_INTERFACE),
                        byte_size: 24,
                        root_count: 2,
                        roots: roots.as_ptr(),
                    };
                    let req = ShaderRequirements {
                        header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
                        features: 0,
                        local_size: [64, 1, 1],
                        shared_memory: 0,
                    };
                    let shader = Shader {
                        stage: 4,
                        format: 0,
                        code: Bytes {
                            data: code.as_ptr().cast(),
                            size: size_of_val(code.as_slice()),
                        },
                        entry: c"transform".as_ptr(),
                        interface_metadata: &abi.header,
                        specialization: ptr::null(),
                        subgroup: ptr::null(),
                    };
                    let desc = ExecutableDesc {
                        header: Record::new::<ExecutableDesc>(EXECUTABLE_DESC),
                        kind: 1,
                        shader_count: 1,
                        shaders: &shader,
                        static_state: ptr::null(),
                        dynamic_state: 0,
                        requirements: &req.header,
                        cache: local.as_ref().map_or(shared as *mut ExecutableCache, |c| {
                            ptr::from_ref(&**c).cast_mut()
                        }),
                        compile_flags: 0,
                        reserved: 0,
                    };
                    let executable = unsafe { Executable::create(d, &desc).unwrap() };
                    drop(local); // Executable does not retain even the local cache.
                    drop(executable);
                });
            }
        });
    }
    #[test]
    #[ignore = "requires modern Vulkan; cache concurrency, failed-output ownership and sticky export loss"]
    fn gpu_foundation_caches() {
        use crate::foundation::{
            ogpu_next_executable_cache_create, ogpu_next_executable_cache_data,
            ogpu_next_executable_cache_destroy,
        };
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
                .find(|q| q.count > 0 && q.flags & COMPUTE != 0)
            else {
                continue;
            };
            let requests = [QueueRequest {
                domain: q.domain,
                count: 1,
                priority: 0.5,
            }];
            let enabled = snapshot.features.available & CACHE_CONTROL;
            let adapter = Adapter {
                instance: instance.clone(),
                physical,
                snapshot,
            };
            let disabled = Device::create(&adapter, &requests, 0).unwrap();
            assert!(matches!(
                unsafe { ExecutableCache::create(&disabled, &description(1)) },
                Err(UNSUPPORTED)
            ));
            let mut device = Device::create(&adapter, &requests, enabled).unwrap();
            concurrent_preparation(&device);
            // All prior children are destroyed before replacing function pointers.
            CREATE.set(device.f.vkCreatePipelineCache);
            device.f.vkCreatePipelineCache = Some(create);
            DESTROY.set(device.f.vkDestroyPipelineCache);
            device.f.vkDestroyPipelineCache = Some(destroy);
            DATA.set(device.f.vkGetPipelineCacheData);
            device.f.vkGetPipelineCacheData = Some(data);
            CALLS.set(0);
            DROPS.set(0);
            FAILURE.set(1);
            let d = ptr::from_ref(&*device).cast_mut();
            let desc = description(0);
            let mut cache = ptr::dangling_mut();
            assert_eq!(
                unsafe { ogpu_next_executable_cache_create(d, &desc, &mut cache) },
                OUT_OF_MEMORY
            );
            assert!(cache.is_null());
            assert_eq!(DROPS.get(), 0);
            FAILURE.set(0);
            assert_eq!(
                unsafe { ogpu_next_executable_cache_create(d, &desc, &mut cache) },
                OK
            );
            assert_eq!(
                unsafe { ExecutableCache::optional(&disabled, cache) },
                Err(INVALID)
            );
            let mut size = 0;
            for (mode, expected) in [
                (2, OUT_OF_MEMORY),
                (0, OK),
                (3, DEVICE_LOST),
                (0, DEVICE_LOST),
            ] {
                FAILURE.set(mode);
                assert_eq!(
                    unsafe { ogpu_next_executable_cache_data(cache, &mut size, ptr::null_mut()) },
                    expected
                );
            }
            assert_eq!(CALLS.get(), 5); // Last export rejected before entering driver.
            let mut another = ptr::dangling_mut();
            assert_eq!(
                unsafe { ogpu_next_executable_cache_create(d, &desc, &mut another) },
                DEVICE_LOST
            );
            assert!(another.is_null());
            assert_eq!(CALLS.get(), 5);
            unsafe {
                ogpu_next_executable_cache_destroy(cache);
            }
            assert_eq!(DROPS.get(), 1);
            tested += 1;
        }
        assert!(tested > 0, "No suitable device; not a passing skip");
    }
    #[test]
    fn cache_headers_are_checked_without_casts_or_driver_calls() {
        let mut bytes = [0u8; 33];
        let data = &mut bytes[1..]; // Deliberately unaligned native blob.
        data[0..4].copy_from_slice(&32u32.to_le_bytes());
        data[4..8].copy_from_slice(&1u32.to_le_bytes());
        data[8..12].copy_from_slice(&17u32.to_le_bytes());
        data[12..16].copy_from_slice(&29u32.to_le_bytes());
        data[16..].fill(7);
        assert_eq!(compatible(data, 17, 29, &[7; 16]), Ok(()));
        assert_eq!(compatible(&data[..31], 17, 29, &[7; 16]), Err(INVALID));
        assert_eq!(compatible(data, 18, 29, &[7; 16]), Err(UNSUPPORTED));
        assert_eq!(compatible(data, 17, 28, &[7; 16]), Err(UNSUPPORTED));
        assert_eq!(compatible(data, 17, 29, &[8; 16]), Err(UNSUPPORTED));
        data[0] = 31;
        assert_eq!(compatible(data, 17, 29, &[7; 16]), Err(INVALID));
        data[0] = 32;
        data[4] = 2;
        assert_eq!(compatible(data, 17, 29, &[7; 16]), Err(UNSUPPORTED));
    }
}
