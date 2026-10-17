//! Caller-owned device-only events. No implicit reset, endpoint tracking or waits.
use super::*;

pub struct Split {
    device: *const Device,
    handle: vk::VkEvent,
}
impl Split {
    pub(in crate::foundation) fn create(d: &Device) -> Result<Box<Self>, Status> {
        d.ready()?;
        let info = vk::VkEventCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_EVENT_CREATE_INFO,
            flags: vk::VkEventCreateFlagBits_VK_EVENT_CREATE_DEVICE_ONLY_BIT,
            ..Default::default()
        };
        let mut handle = ptr::null_mut();
        unsafe {
            d.result((d.f.vkCreateEvent.unwrap())(
                d.handle,
                &info,
                ptr::null(),
                &mut handle,
            ))?;
        }
        // A failed native creation's output is unspecified; adopt only on success.
        Ok(Box::new(Self { device: d, handle }))
    }
    fn check(&self, d: &Device) -> Result<(), Status> {
        if self.device != ptr::from_ref(d) {
            return Err(INVALID);
        }
        Ok(())
    }
}
impl Drop for Split {
    fn drop(&mut self) {
        let d = unsafe { &*self.device };
        unsafe {
            (d.f.vkDestroyEvent.unwrap())(d.handle, self.handle, ptr::null());
        }
    }
}
fn split_layout(count: u32) -> Result<(Layout, usize, usize), Status> {
    let (layout, info) = array_layout::<vk::VkEvent>(count)?
        .extend(array_layout::<vk::VkDependencyInfo>(count)?)
        .map_err(|_| INVALID)?;
    let (layout, global) = layout
        .extend(array_layout::<vk::VkMemoryBarrier2>(count)?)
        .map_err(|_| INVALID)?;
    Ok((layout.pad_to_align(), info, global))
}
pub(in crate::foundation) fn split_scratch(count: u32) -> Result<HostRequirements, Status> {
    let (layout, _, _) = split_layout(count)?;
    Ok(HostRequirements {
        size: layout.size() as u64,
        alignment: layout.align() as u64,
    })
}
fn scopes(dep: &Dependency) -> Result<(), Status> {
    // Device-only event operations never include the host in either scope.
    if (dep.before | dep.after) & 256 != 0 {
        return Err(INVALID);
    }
    Ok(())
}
impl List {
    fn split_recording(&self) -> Result<&Device, Status> {
        let d = self.outside_render()?;
        stages(d, self.domain, 32, false)?; // event commands need graphics/compute, not transfer-only
        Ok(d)
    }
    pub(in crate::foundation) unsafe fn split_release(
        &self,
        split: &Split,
        dep: &Dependency,
    ) -> Result<(), Status> {
        let d = self.split_recording()?;
        split.check(d)?;
        scopes(dep)?;
        let mut global = vk::VkMemoryBarrier2::default();
        let info = unsafe { self.translate_dependency(dep, &mut global)? };
        unsafe {
            (d.f.vkCmdSetEvent2.unwrap())(self.command, split.handle, &info);
        }
        Ok(())
    }
    pub(in crate::foundation) fn split_reset(
        &self,
        split: &Split,
        before: u64,
    ) -> Result<(), Status> {
        let d = self.split_recording()?;
        split.check(d)?;
        let before = stages(d, self.domain, before, true)?;
        unsafe {
            (d.f.vkCmdResetEvent2.unwrap())(self.command, split.handle, before);
        }
        Ok(())
    }
    pub(in crate::foundation) unsafe fn split_acquire(
        &self,
        count: u32,
        waits: *const SplitDependency,
        storage: HostSpan,
    ) -> Result<(), Status> {
        let d = self.split_recording()?;
        if count == 0 {
            return Ok(());
        }
        let waits = unsafe { slice(waits, count)? };
        let (layout, info_offset, global_offset) = split_layout(count)?;
        let base = scratch(
            storage.data,
            storage.size,
            HostRequirements {
                size: layout.size() as u64,
                alignment: layout.align() as u64,
            },
        )?
        .cast::<u8>();
        let events = base.cast::<vk::VkEvent>();
        let infos = base
            .wrapping_add(info_offset)
            .cast::<vk::VkDependencyInfo>();
        let globals = base
            .wrapping_add(global_offset)
            .cast::<vk::VkMemoryBarrier2>();
        for (i, wait) in waits.iter().enumerate() {
            let split = unsafe { wait.split.as_ref() }.ok_or(INVALID)?;
            let dep =
                unsafe { executable_record::<Dependency>(wait.dependency.cast(), DEPENDENCY)? };
            split.check(d)?;
            scopes(dep)?;
            unsafe {
                globals.add(i).write(vk::VkMemoryBarrier2::default());
                let info = self.translate_dependency(dep, &mut *globals.add(i))?;
                events.add(i).write(split.handle);
                infos.add(i).write(info);
            }
        }
        // All records and their scratch translations validate before one native wait.
        unsafe {
            (d.f.vkCmdWaitEvents2.unwrap())(self.command, count, events, infos);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    thread_local! {
        static CREATE: Cell<vk::PFN_vkCreateEvent> = const { Cell::new(None) };
        static DESTROY: Cell<vk::PFN_vkDestroyEvent> = const { Cell::new(None) };
        static WAIT: Cell<vk::PFN_vkCmdWaitEvents2> = const { Cell::new(None) };
        static FAILURE: Cell<u32> = const { Cell::new(0) };
        static CREATES: Cell<u32> = const { Cell::new(0) };
        static DROPS: Cell<u32> = const { Cell::new(0) };
        static WAITS: Cell<u32> = const { Cell::new(0) };
        static TRANSLATED: Cell<bool> = const { Cell::new(false) };
    }
    unsafe extern "C" fn create(
        d: vk::VkDevice,
        info: *const vk::VkEventCreateInfo,
        a: *const vk::VkAllocationCallbacks,
        out: *mut vk::VkEvent,
    ) -> vk::VkResult {
        CREATES.set(CREATES.get() + 1);
        if FAILURE.get() != 0 {
            unsafe { out.write(ptr::dangling_mut()) };
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
        event: vk::VkEvent,
        a: *const vk::VkAllocationCallbacks,
    ) {
        DROPS.set(DROPS.get() + 1);
        unsafe { DESTROY.get().unwrap()(d, event, a) };
    }
    unsafe extern "C" fn wait(
        command: vk::VkCommandBuffer,
        count: u32,
        events: *const vk::VkEvent,
        infos: *const vk::VkDependencyInfo,
    ) {
        WAITS.set(WAITS.get() + 1);
        if count == 2 {
            let a = unsafe { &*infos };
            let b = unsafe { &*infos.add(1) };
            if a.memoryBarrierCount == 1 && b.memoryBarrierCount == 1 {
                let ga = unsafe { &*a.pMemoryBarriers };
                let gb = unsafe { &*b.pMemoryBarriers };
                TRANSLATED.set(
                    unsafe { *events != *events.add(1) }
                        && a.pMemoryBarriers != b.pMemoryBarriers
                        && ga.srcStageMask == vk::VK_PIPELINE_STAGE_2_COPY_BIT
                        && gb.srcStageMask == vk::VK_PIPELINE_STAGE_2_CLEAR_BIT
                        && ga.dstStageMask == vk::VK_PIPELINE_STAGE_2_COPY_BIT
                        && gb.dstStageMask == ga.dstStageMask
                        && ga.srcAccessMask == vk::VK_ACCESS_2_TRANSFER_WRITE_BIT
                        && gb.srcAccessMask == ga.srcAccessMask
                        && ga.dstAccessMask == vk::VK_ACCESS_2_TRANSFER_READ_BIT
                        && gb.dstAccessMask == ga.dstAccessMask,
                );
            }
        }
        unsafe { WAIT.get().unwrap()(command, count, events, infos) };
    }
    #[test]
    #[ignore = "requires modern Vulkan; event failure ownership, batch atomicity and native lowering"]
    fn gpu_foundation_splits() {
        use crate::foundation::{ogpu_next_split_create, ogpu_next_split_destroy};
        let instance = Arc::new(Instance::new().unwrap());
        let mut tested = 0;
        for physical in instance.physical_devices().unwrap() {
            let snapshot = snapshot(&instance, physical).unwrap();
            if snapshot.features.baseline_supported == 0 {
                continue;
            }
            let domain = snapshot
                .queues
                .iter()
                .find(|q| q.count > 0 && q.flags & COMPUTE != 0)
                .unwrap()
                .domain;
            let adapter = Adapter {
                state: AdapterState {
                    instance: instance.clone(),
                    physical,
                },
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
            CREATE.set(device.f.vkCreateEvent);
            DESTROY.set(device.f.vkDestroyEvent);
            WAIT.set(device.f.vkCmdWaitEvents2);
            device.f.vkCreateEvent = Some(create);
            device.f.vkDestroyEvent = Some(destroy);
            device.f.vkCmdWaitEvents2 = Some(wait);
            CREATES.set(0);
            DROPS.set(0);
            let d = ptr::from_ref(&*device).cast_mut();
            let mut out = ptr::dangling_mut();
            FAILURE.set(1);
            assert_eq!(
                unsafe { ogpu_next_split_create(d, &mut out) },
                OUT_OF_MEMORY
            );
            assert!(out.is_null());
            assert_eq!(DROPS.get(), 0);
            FAILURE.set(0);
            let a = Split::create(&device).unwrap();
            let b = Split::create(&device).unwrap();
            let dep = Dependency {
                header: Record::new::<Dependency>(DEPENDENCY),
                before: 512,
                after: 512,
                global_before: 8,
                global_after: 4,
                memory_count: 0,
                image_count: 0,
                flags: 0,
                memory: ptr::null(),
                images: ptr::null(),
                scratch: ptr::null_mut(),
                scratch_size: 0,
            };
            let dep_b = Dependency {
                before: 16384,
                ..dep
            };
            let mut bad = Dependency { ..dep };
            bad.header.version = 2;
            let waits = [
                SplitDependency {
                    split: &*a,
                    dependency: &dep,
                },
                SplitDependency {
                    split: &*b,
                    dependency: &dep_b,
                },
            ];
            let requirements = split_scratch(2).unwrap();
            let mut data = vec![0u64; requirements.size.div_ceil(8) as usize];
            let storage = HostSpan {
                data: data.as_mut_ptr().cast(),
                size: requirements.size,
            };
            for level in [0, 1] {
                let mut arena = Arena::create(
                    &device,
                    &ArenaDesc {
                        header: Record::new::<ArenaDesc>(ARENA_DESC),
                        domain,
                        list_capacity: 1,
                        level,
                        reserved: 0,
                    },
                )
                .unwrap();
                let list = arena
                    .begin(&RecordingDesc {
                        header: Record::new::<RecordingDesc>(RECORDING_DESC),
                        replay_mode: 1,
                        level,
                        inheritance: ptr::null(),
                    })
                    .unwrap();
                let list = unsafe { &*list };
                WAITS.set(0);
                TRANSLATED.set(false);
                assert_eq!(
                    unsafe {
                        list.split_acquire(
                            0,
                            ptr::null(),
                            HostSpan {
                                data: ptr::null_mut(),
                                size: 0,
                            },
                        )
                    },
                    Ok(())
                );
                assert_eq!(
                    unsafe {
                        list.split_acquire(
                            2,
                            waits.as_ptr(),
                            HostSpan {
                                size: storage.size - 1,
                                ..storage
                            },
                        )
                    },
                    Err(CAPACITY)
                );
                let invalid = [
                    SplitDependency { ..waits[0] },
                    SplitDependency {
                        dependency: &bad,
                        ..waits[1]
                    },
                ];
                assert_eq!(
                    unsafe { list.split_acquire(2, invalid.as_ptr(), storage) },
                    Err(UNSUPPORTED)
                );
                assert_eq!(WAITS.get(), 0); // No valid prefix leaks a native wait.
                assert_eq!(list.split_reset(&a, 256), Err(INVALID));
                assert_eq!(
                    unsafe { list.split_release(&a, &Dependency { before: 256, ..dep }) },
                    Err(INVALID)
                );
                unsafe {
                    list.split_release(&a, &dep).unwrap();
                    list.split_release(&b, &dep_b).unwrap();
                    list.split_acquire(2, waits.as_ptr(), storage).unwrap();
                }
                assert_eq!(WAITS.get(), 1);
                assert!(TRANSLATED.get());
                list.end().unwrap();
                // No submission: the C consumer checks actual cross-list GPU execution.
            }
            drop(a);
            drop(b);
            assert_eq!(DROPS.get(), 2);
            FAILURE.set(2);
            assert_eq!(unsafe { ogpu_next_split_create(d, &mut out) }, DEVICE_LOST);
            assert!(out.is_null());
            unsafe { ogpu_next_split_destroy(out) };
            assert_eq!(unsafe { ogpu_next_split_create(d, &mut out) }, DEVICE_LOST);
            assert!(out.is_null());
            assert_eq!(CREATES.get(), 4);
            assert_eq!(DROPS.get(), 2);
            tested += 1;
        }
        assert!(tested > 0, "No suitable device; not a passing skip");
    }
    #[test]
    fn split_scratch_preserves_all_native_array_alignments() {
        assert_eq!(
            split_scratch(0).unwrap(),
            HostRequirements {
                size: 0,
                alignment: 1
            }
        );
        for count in [1, 2, 17] {
            let (layout, info, global) = split_layout(count).unwrap();
            assert!(info >= count as usize * size_of::<vk::VkEvent>());
            assert!(global >= info + count as usize * size_of::<vk::VkDependencyInfo>());
            assert!(layout.size() >= global + count as usize * size_of::<vk::VkMemoryBarrier2>());
            assert_eq!(info % align_of::<vk::VkDependencyInfo>(), 0);
            assert_eq!(global % align_of::<vk::VkMemoryBarrier2>(), 0);
        }
    }
}
