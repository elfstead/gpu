//! Explicit native binary semaphores and fences. No CPU/GPU state mirror,
//! implicit waits, retention or dependency conversion. Valid reuse is caller-owned.
use super::*;

pub struct Binary {
    pub(super) device: *const Device,
    pub(super) handle: vk::VkSemaphore,
}
impl Binary {
    pub(in crate::foundation) fn create(d: &Device) -> Result<Box<Self>, Status> {
        d.ready()?;
        let info = vk::VkSemaphoreCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO,
            ..Default::default()
        };
        let mut handle = ptr::null_mut();
        unsafe {
            d.result((d.f.vkCreateSemaphore.unwrap())(
                d.handle,
                &info,
                ptr::null(),
                &mut handle,
            ))?;
        }
        Ok(Box::new(Self { device: d, handle }))
    }
}
impl Drop for Binary {
    fn drop(&mut self) {
        let d = unsafe { &*self.device };
        unsafe {
            (d.f.vkDestroySemaphore.unwrap())(d.handle, self.handle, ptr::null());
        }
    }
}

pub struct Fence {
    device: *const Device,
    handle: vk::VkFence,
}
impl Fence {
    pub(in crate::foundation) fn create(d: &Device, signaled: bool) -> Result<Box<Self>, Status> {
        d.ready()?;
        let info = vk::VkFenceCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_FENCE_CREATE_INFO,
            flags: if signaled {
                vk::VkFenceCreateFlagBits_VK_FENCE_CREATE_SIGNALED_BIT
            } else {
                0
            },
            ..Default::default()
        };
        let mut handle = ptr::null_mut();
        unsafe {
            d.result((d.f.vkCreateFence.unwrap())(
                d.handle,
                &info,
                ptr::null(),
                &mut handle,
            ))?;
        }
        Ok(Box::new(Self { device: d, handle }))
    }
    pub(in crate::foundation) fn poll(&self) -> Result<bool, Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        match unsafe { d.result((d.f.vkGetFenceStatus.unwrap())(d.handle, self.handle)) } {
            Ok(()) => Ok(true),
            Err(NOT_READY) => Ok(false),
            Err(e) => Err(e),
        }
    }
    pub(in crate::foundation) fn wait(&self, timeout: u64) -> Result<(), Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        unsafe {
            d.result((d.f.vkWaitForFences.unwrap())(
                d.handle,
                1,
                &self.handle,
                vk::VK_TRUE,
                timeout,
            ))
        }
    }
    pub(in crate::foundation) fn reset(&self) -> Result<(), Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        unsafe { d.result((d.f.vkResetFences.unwrap())(d.handle, 1, &self.handle)) }
    }
}
impl Drop for Fence {
    fn drop(&mut self) {
        let d = unsafe { &*self.device };
        unsafe {
            (d.f.vkDestroyFence.unwrap())(d.handle, self.handle, ptr::null());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::{
        ogpu_next_presentation_api, ogpu_next_queue_submit, ogpu_next_queue_submit_batch, rust,
    };
    use std::cell::Cell;

    thread_local! {
        static NATIVE_SUBMIT: Cell<vk::PFN_vkQueueSubmit2> = const { Cell::new(None) };
        static CALLS: Cell<u32> = const { Cell::new(0) };
        static FAIL: Cell<bool> = const { Cell::new(false) };
        static SHAPE: Cell<bool> = const { Cell::new(false) };
        static BINARY: Cell<vk::VkSemaphore> = const { Cell::new(ptr::null_mut()) };
        static TIMELINE: Cell<vk::VkSemaphore> = const { Cell::new(ptr::null_mut()) };
    }
    unsafe extern "C" fn inspect_submit(
        queue: vk::VkQueue,
        count: u32,
        infos: *const vk::VkSubmitInfo2,
        fence: vk::VkFence,
    ) -> vk::VkResult {
        CALLS.set(CALLS.get() + 1);
        let mut shape = count == 2 && !infos.is_null() && fence.is_null();
        if shape {
            let a = unsafe { &*infos };
            let b = unsafe { &*infos.add(1) };
            shape = a.commandBufferInfoCount == 0
                && a.waitSemaphoreInfoCount == 0
                && a.signalSemaphoreInfoCount == 1
                && b.commandBufferInfoCount == 0
                && b.waitSemaphoreInfoCount == 2
                && b.signalSemaphoreInfoCount == 1;
            if shape {
                let signal = unsafe { &*a.pSignalSemaphoreInfos };
                let tw = unsafe { &*b.pWaitSemaphoreInfos };
                let bw = unsafe { &*b.pWaitSemaphoreInfos.add(1) };
                let complete = unsafe { &*b.pSignalSemaphoreInfos };
                shape = signal.semaphore == BINARY.get()
                    && signal.value == 0
                    && signal.stageMask == vk::VK_PIPELINE_STAGE_2_ALL_TRANSFER_BIT
                    && bw.semaphore == BINARY.get()
                    && bw.value == 0
                    && bw.stageMask == vk::VK_PIPELINE_STAGE_2_ALL_TRANSFER_BIT
                    && tw.semaphore == TIMELINE.get()
                    && tw.value == 0
                    && tw.stageMask == vk::VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT
                    && complete.semaphore == TIMELINE.get()
                    && complete.value == 1
                    && complete.stageMask == vk::VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT;
            }
        }
        SHAPE.set(shape);
        if FAIL.get() {
            return vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY;
        }
        match NATIVE_SUBMIT.get() {
            Some(f) => unsafe { f(queue, count, infos, fence) },
            None => vk::VkResult_VK_ERROR_UNKNOWN,
        }
    }
    unsafe extern "C" fn fail_fence(
        _: vk::VkDevice,
        _: *const vk::VkFenceCreateInfo,
        _: *const vk::VkAllocationCallbacks,
        out: *mut vk::VkFence,
    ) -> vk::VkResult {
        unsafe {
            out.write(ptr::dangling_mut());
        }
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
    }
    unsafe extern "C" fn fail_binary(
        _: vk::VkDevice,
        _: *const vk::VkSemaphoreCreateInfo,
        _: *const vk::VkAllocationCallbacks,
        out: *mut vk::VkSemaphore,
    ) -> vk::VkResult {
        unsafe {
            out.write(ptr::dangling_mut());
        }
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
    }

    #[test]
    #[ignore = "requires modern Vulkan; headless binary lowering, C extension paths, retry and fence lifecycle"]
    fn gpu_native_binary_and_fences() {
        let discovery = rust::discover().unwrap();
        let adapter = rust::adapters(&discovery)
            .iter()
            .find(|a| rust::adapter_features(a).baseline_supported != 0)
            .unwrap();
        let domain = rust::adapter_queues(adapter)
            .iter()
            .find(|q| q.flags & COMPUTE != 0)
            .unwrap()
            .domain;
        let requests = [QueueRequest {
            domain,
            count: 1,
            priority: 1.0,
        }];
        let mut d = Device::create(adapter, &requests, 0).unwrap();
        let other = Device::create(adapter, &requests, 0).unwrap();
        let foreign = Binary::create(&other).unwrap();
        let saved = d.f.vkCreateFence;
        d.f.vkCreateFence = Some(fail_fence);
        assert!(matches!(Fence::create(&d, false), Err(OUT_OF_MEMORY)));
        d.f.vkCreateFence = saved;
        let saved = d.f.vkCreateSemaphore;
        d.f.vkCreateSemaphore = Some(fail_binary);
        assert!(matches!(Binary::create(&d), Err(OUT_OF_MEMORY)));
        d.f.vkCreateSemaphore = saved;
        NATIVE_SUBMIT.set(d.f.vkQueueSubmit2);
        d.f.vkQueueSubmit2 = Some(inspect_submit);
        let mut api = std::mem::MaybeUninit::<PresentationApi>::uninit();
        unsafe {
            assert_eq!(
                ogpu_next_presentation_api(
                    VERSION,
                    size_of::<PresentationApi>() as u32,
                    api.as_mut_ptr()
                ),
                OK
            );
            let api = api.assume_init();
            let dp = ptr::from_mut(&mut *d);
            let mut fence = ptr::dangling_mut();
            assert_eq!(api.fence_create.unwrap()(dp, 2, &mut fence), INVALID);
            assert!(fence.is_null());
            for signaled in [0, 1] {
                assert_eq!(api.fence_create.unwrap()(dp, signaled, &mut fence), OK);
                assert_eq!(
                    api.fence_poll.unwrap()(fence),
                    if signaled != 0 { OK } else { NOT_READY }
                );
                assert_eq!(
                    api.fence_wait.unwrap()(fence, 0),
                    if signaled != 0 { OK } else { TIMEOUT }
                );
                assert_eq!(api.fence_reset.unwrap()(fence), OK);
                assert_eq!(api.fence_poll.unwrap()(fence), NOT_READY);
                assert_eq!(api.fence_wait.unwrap()(fence, 0), TIMEOUT);
                api.fence_destroy.unwrap()(fence);
            }
            let mut binary = ptr::null_mut();
            assert_eq!(api.binary_create.unwrap()(dp, &mut binary), OK);
            let timeline = Timeline::create(&d, 0).unwrap();
            BINARY.set((*binary).handle);
            TIMELINE.set(timeline.handle);
            let binary_point = BinaryPoint {
                binary,
                stages: STAGE_TRANSFER,
            };
            let foreign_point = BinaryPoint {
                binary: ptr::from_ref(&*foreign).cast_mut(),
                stages: STAGE_TRANSFER,
            };
            let extension = |waits, signals, wc, sc| BinarySubmitDesc {
                header: Record::new::<BinarySubmitDesc>(BINARY_SUBMIT_DESC),
                wait_count: wc,
                signal_count: sc,
                waits,
                signals,
            };
            let produce = extension(ptr::null(), &binary_point, 0, 1);
            let consume = extension(&binary_point, ptr::null(), 1, 0);
            let bad_consume = extension(&foreign_point, ptr::null(), 1, 0);
            let point = |value, stages| SyncPoint {
                point: Point {
                    timeline: ptr::from_ref(&*timeline).cast_mut(),
                    value,
                },
                stages,
            };
            let wait = point(0, STAGE_COMPUTE);
            let signal = point(1, STAGE_ALL);
            let mut first =
                vec![0u64; commands::submit_scratch(0, 0, 1).unwrap().size.div_ceil(8) as usize];
            let mut second =
                vec![0u64; commands::submit_scratch(0, 2, 1).unwrap().size.div_ceil(8) as usize];
            let mut batch =
                vec![0u64; commands::submit_batch_scratch(2).unwrap().size.div_ceil(8) as usize];
            let storage = HostSpan {
                data: batch.as_mut_ptr().cast(),
                size: (batch.len() * 8) as u64,
            };
            let mut a = Record::new::<SubmitDesc>(SUBMIT_DESC);
            a.next = &produce.header;
            let mut b = Record::new::<SubmitDesc>(SUBMIT_DESC);
            b.next = &bad_consume.header;
            let mut descs = [
                SubmitDesc {
                    header: a,
                    list_count: 0,
                    wait_count: 0,
                    signal_count: 0,
                    lists: ptr::null(),
                    waits: ptr::null(),
                    signals: ptr::null(),
                    scratch: first.as_mut_ptr().cast(),
                    scratch_size: (first.len() * 8) as u64,
                },
                SubmitDesc {
                    header: b,
                    list_count: 0,
                    wait_count: 1,
                    signal_count: 1,
                    lists: ptr::null(),
                    waits: &wait,
                    signals: &signal,
                    scratch: second.as_mut_ptr().cast(),
                    scratch_size: (second.len() * 8) as u64,
                },
            ];
            let queue = ptr::from_ref(d.queue(domain, 0).unwrap()).cast_mut();
            CALLS.set(0);
            FAIL.set(false);
            assert_eq!(
                ogpu_next_queue_submit_batch(queue, 2, descs.as_ptr(), storage),
                INVALID
            );
            assert_eq!(CALLS.get(), 0);
            descs[1].header.next = &consume.header;
            let short = Record::new::<Record>(BINARY_SUBMIT_DESC);
            descs[0].header.next = &short;
            assert_eq!(ogpu_next_queue_submit(queue, &descs[0]), INVALID);
            assert_eq!(CALLS.get(), 0);
            descs[0].header.next = &produce.header;
            let size = descs[1].scratch_size;
            descs[1].scratch_size = 0;
            assert_eq!(
                ogpu_next_queue_submit_batch(queue, 2, descs.as_ptr(), storage),
                CAPACITY
            );
            assert_eq!(CALLS.get(), 0);
            descs[1].scratch_size = size;
            FAIL.set(true);
            assert_eq!(
                ogpu_next_queue_submit_batch(queue, 2, descs.as_ptr(), storage),
                OUT_OF_MEMORY
            );
            assert_eq!(CALLS.get(), 1);
            assert!(SHAPE.get());
            FAIL.set(false);
            CALLS.set(0);
            assert_eq!(
                ogpu_next_queue_submit_batch(queue, 2, descs.as_ptr(), storage),
                OK
            );
            assert_eq!(CALLS.get(), 1);
            assert!(SHAPE.get());
            timeline.wait(1, 10_000_000_000).unwrap();
            api.binary_destroy.unwrap()(binary);
        }
    }
}
