//! Vulkan presentation lifecycle: native effects and borrowed frame values.
//! Frames borrow images and caller synchronization. There is no lease registry,
//! hidden submission, host wait, per-frame allocation or duplicate/resource walk.
use super::*;
fn enabled(d: &Device) -> Result<(), Status> {
    d.ready()?;
    if d.snapshot.features.enabled & PRESENTATION == 0 {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
pub(in crate::foundation) fn contract(
    adapter: &Adapter,
    surface: &Surface,
    profile: u32,
) -> Result<PresentationContract, Status> {
    surface.native_handle(&adapter.instance)?;
    if profile != PRESENT_VULKAN || adapter.snapshot.features.available & PRESENTATION == 0 {
        return Err(UNSUPPORTED);
    }
    let queues = unsafe { query::<SurfaceQueue>(surface, adapter, None, SURFACE_QUEUES)? };
    if !queues.iter().any(|q| q.supported != 0) {
        return Err(UNSUPPORTED);
    }
    Ok(PresentationContract {
        profile,
        availability: AVAILABILITY_SIGNALS,
        capabilities: PRESENT_REPLACE | PRESENT_RETIRE_FENCE | PRESENT_BINARY_WAIT,
        max_present_count: u32::MAX,
    })
}

// Recognized complete records expose the old chain's actual state even on local
// rejection. A short/unknown record cannot identify a previous chain safely.
pub(in crate::foundation) unsafe fn create(
    d: &Device,
    record: &Record,
    out: &mut ChainCreateOutcome,
) -> Result<(), Status> {
    *out = ChainCreateOutcome {
        chain: ptr::null_mut(),
        previous_state: CHAIN_NONE,
        reserved: 0,
    };
    if record.kind == DXGI_CHAIN_DESC {
        record.validate::<DxgiChainDesc>(DXGI_CHAIN_DESC)?;
        return Err(UNSUPPORTED);
    }
    if record.kind != SWAPCHAIN_DESC {
        return Err(UNSUPPORTED);
    }
    if record.byte_size as usize != size_of::<SwapchainDesc>() {
        return Err(INVALID);
    }
    let desc = unsafe { &*ptr::from_ref(record).cast::<SwapchainDesc>() };
    let previous = unsafe { desc.old_swapchain.as_ref() };
    out.previous_state = previous.map_or(CHAIN_NONE, |p| p.state.get());
    let result = unsafe { Swapchain::create(d, desc) };
    out.previous_state = previous.map_or(CHAIN_NONE, |p| p.state.get());
    out.chain = Box::into_raw(result?);
    Ok(())
}
impl Swapchain {
    pub(in crate::foundation) fn info(&self) -> ChainInfo {
        ChainInfo {
            generation: self.generation,
            state: self.state.get(),
            image_count: self.images.len() as u32,
            ordered_queue: ptr::null_mut(),
        }
    }
    pub(in crate::foundation) fn resize(
        &self,
        _desc: &ChainResizeDesc,
        out: &mut ChainResizeOutcome,
    ) -> Result<(), Status> {
        *out = ChainResizeOutcome {
            generation: self.generation,
            state: self.state.get(),
            image_effect: IMAGES_UNCHANGED,
        };
        enabled(unsafe { &*self.device })?;
        Err(UNSUPPORTED)
    }
    pub(in crate::foundation) fn pacing_handle(
        &self,
        _out: &mut PacingHandle,
    ) -> Result<(), Status> {
        enabled(unsafe { &*self.device })?;
        Err(UNSUPPORTED)
    }
    fn frame(&self, index: u32, desc: &AcquireDesc) -> Frame {
        Frame {
            chain: ptr::from_ref(self).cast_mut(),
            image: ptr::from_ref(&self.images[index as usize]).cast_mut(),
            generation: self.generation,
            index,
            availability: AVAILABILITY_SIGNALS,
            ordered_queue: ptr::null_mut(),
            binary: desc.binary,
            fence: desc.fence,
        }
    }
    pub(in crate::foundation) unsafe fn acquire_frame(
        &self,
        desc: &AcquireDesc,
        out: &mut Frame,
    ) -> Result<(), Status> {
        let d = unsafe { &*self.device };
        enabled(d)?;
        if self.state.get() != CHAIN_ACTIVE {
            return Err(INVALID);
        }
        if desc.binary.is_null() && desc.fence.is_null() {
            return Err(INVALID);
        }
        let binary = unsafe { desc.binary.as_ref() };
        let fence = unsafe { desc.fence.as_ref() };
        if binary.is_some_and(|b| b.device != self.device)
            || fence.is_some_and(|f| f.device != self.device)
        {
            return Err(INVALID);
        }
        let mut index = u32::MAX;
        let result = d.result(unsafe {
            (self.f.vkAcquireNextImageKHR.unwrap())(
                d.handle,
                self.handle,
                desc.timeout_ns,
                binary.map_or(ptr::null_mut(), |b| b.handle),
                fence.map_or(ptr::null_mut(), |f| f.handle),
                &mut index,
            )
        });
        if result.is_ok() || result == Err(SUBOPTIMAL) {
            if index as usize >= self.images.len() {
                // Signals/ownership are uncertain; never publish a bogus image.
                d.lost.store(true, Ordering::Relaxed);
                return Err(BACKEND_ERROR);
            }
            *out = self.frame(index, desc);
        }
        result
    }

    // Acquisition sync fields are historical identities: never dereference them.
    // Generation detects replacement of the image set, not copied/reused leases.
    fn validate_frame(&self, frame: &Frame) -> Result<u32, Status> {
        if !ptr::eq(frame.chain, self)
            || frame.generation != self.generation
            || frame.index as usize >= self.images.len()
            || !ptr::eq(frame.image, &self.images[frame.index as usize])
            || frame.availability != AVAILABILITY_SIGNALS
            || !frame.ordered_queue.is_null()
            || !matches!(self.state.get(), CHAIN_ACTIVE | CHAIN_RETIRED)
        {
            return Err(INVALID);
        }
        Ok(frame.index)
    }
    pub(in crate::foundation) unsafe fn abandon(
        &self,
        frames: &[Frame],
        storage: HostSpan,
        effect: &mut u32,
    ) -> Result<(), Status> {
        *effect = FRAME_RETAINED;
        let d = unsafe { &*self.device };
        enabled(d)?;
        let count = u32::try_from(frames.len()).map_err(|_| INVALID)?;
        if count == 0 {
            return Err(INVALID);
        }
        let indices =
            scratch(storage.data, storage.size, abandon_requirements(d, count)?)?.cast::<u32>();
        for (i, frame) in frames.iter().enumerate() {
            unsafe {
                indices.add(i).write(self.validate_frame(frame)?);
            }
        }
        let indices = unsafe { std::slice::from_raw_parts(indices, count as usize) };
        let result = d.result(unsafe { self.release_native(d, indices) });
        *effect = if result.is_ok() {
            FRAME_RELEASED
        } else {
            FRAME_UNKNOWN
        };
        result
    }
}
pub(in crate::foundation) fn abandon_requirements(
    d: &Device,
    count: u32,
) -> Result<HostRequirements, Status> {
    enabled(d)?;
    if count == 0 {
        return Ok(HostRequirements {
            size: 0,
            alignment: 1,
        });
    }
    let layout = Layout::array::<u32>(count as usize).map_err(|_| INVALID)?;
    Ok(HostRequirements {
        size: layout.size() as u64,
        alignment: layout.align() as u64,
    })
}
pub(in crate::foundation) fn present_requirements(
    d: &Device,
    waits: u32,
    items: u32,
) -> Result<HostRequirements, Status> {
    enabled(d)?;
    let mut req = present_scratch(waits, items)?;
    if req.size == 0 {
        req.alignment = 1;
    }
    Ok(req)
}

// This native-call guarantee is not inferred from generic OGPU error classes.
fn batch_effect(result: vk::VkResult) -> u32 {
    match result {
        vk::VkResult_VK_SUCCESS
        | vk::VkResult_VK_SUBOPTIMAL_KHR
        | vk::VkResult_VK_ERROR_OUT_OF_DATE_KHR
        | vk::VkResult_VK_ERROR_SURFACE_LOST_KHR => EFFECT_ENQUEUED,
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY | vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY => {
            EFFECT_NONE
        }
        _ => EFFECT_UNKNOWN,
    }
}
fn item_result(aggregate: vk::VkResult, item: vk::VkResult, has_fence: bool) -> PresentResult {
    let mapped = |r| status(r).err().unwrap_or(OK);
    // The aggregate OOM guarantee covers ALL resources/sync. Native per-item
    // outputs cannot contradict that call-wide unchanged-state guarantee.
    if batch_effect(aggregate) == EFFECT_NONE {
        return PresentResult {
            status: mapped(aggregate),
            frame_effect: FRAME_RETAINED,
            fence_effect: EFFECT_NONE,
            reserved: 0,
        };
    }
    // An item OOM alone is not the native call's no-effects guarantee.
    let enqueued = batch_effect(item) == EFFECT_ENQUEUED;
    PresentResult {
        status: mapped(item),
        frame_effect: if enqueued {
            FRAME_RELEASED
        } else {
            FRAME_UNKNOWN
        },
        fence_effect: if !has_fence {
            EFFECT_NONE
        } else if enqueued {
            EFFECT_ENQUEUED
        } else {
            EFFECT_UNKNOWN
        },
        reserved: 0,
    }
}
impl Queue {
    pub(in crate::foundation) unsafe fn present_frames(
        &self,
        desc: &PresentDesc,
        out: &mut PresentOutcome,
    ) -> Result<(), Status> {
        *out = PresentOutcome {
            work_effect: EFFECT_NONE,
            wait_effect: EFFECT_NONE,
        };
        let d = self.present_device()?;
        if desc.item_count == 0 || desc.results.is_null() {
            return Err(INVALID);
        }
        Layout::array::<PresentResult>(desc.item_count as usize).map_err(|_| INVALID)?;
        let waits = unsafe { slice(desc.waits, desc.wait_count)? };
        let items = unsafe { slice(desc.items, desc.item_count)? };
        let (aggregate, native_results) = unsafe {
            self.present_native(d, waits, desc.item_count, desc.scratch, |i| {
                let item = &items[i];
                if item.sync_interval != 0 || item.native_flags != 0 {
                    return Err(UNSUPPORTED);
                }
                let chain = item.frame.chain.as_ref().ok_or(INVALID)?;
                let index = chain.validate_frame(&item.frame)?;
                chain.present_entry(d, self.domain, index, item.retire_fence)
            })?
        };
        let effect = batch_effect(aggregate);
        *out = PresentOutcome {
            work_effect: effect,
            wait_effect: if desc.wait_count == 0 {
                EFFECT_NONE
            } else {
                effect
            },
        };
        let mut result = d.result(aggregate);
        for (i, item) in items.iter().enumerate() {
            let native = unsafe { native_results.add(i).read() };
            let value = item_result(aggregate, native, !item.retire_fence.is_null());
            unsafe {
                desc.results.add(i).write(value);
            }
            if effect != EFFECT_NONE && native == vk::VkResult_VK_ERROR_DEVICE_LOST {
                d.lost.store(true, Ordering::Relaxed);
            }
            // Never turn missing/failed item results into an aggregate success.
            // An existing aggregate failure takes precedence over item success.
            if (result.is_ok() || result == Err(SUBOPTIMAL)) && value.status != OK {
                result = Err(value.status);
            }
        }
        result
    }
}

#[cfg(test)]
#[path = "presentation_lifecycle_tests.rs"]
mod tests;
