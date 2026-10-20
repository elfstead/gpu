//! D2/D3 surface stubs. These operations have no resolved D3D12 mapping in this
//! batch: they return UNSUPPORTED instead of emulating weaker semantics. The
//! mapping matrix in docs/dx12-backend-d1.md tracks each one.
use super::*;
use std::ffi::c_void;

/// Record validation shared with the C boundary: exact kind/version/size, no
/// flags and no chained records. Backend-neutral boundary check.
pub(in crate::foundation) unsafe fn executable_record<'a, T>(
    p: *const Record,
    kind: u32,
) -> Result<&'a T, Status> {
    if p.is_null() {
        return Err(INVALID);
    }
    // SAFETY: caller guarantees a complete record of the declared kind.
    unsafe { p.read() }.validate::<T>(kind)?;
    Ok(unsafe { &*p.cast() })
}

// Scratch queries for the D2/D3 surfaces return exact zero-size requirements;
// those operations reject before any translation would need storage.
fn no_scratch() -> Result<HostRequirements, Status> {
    Ok(HostRequirements {
        size: 0,
        alignment: 1,
    })
}

pub(in crate::foundation) fn present_scratch(
    _waits: u32,
    _images: u32,
) -> Result<HostRequirements, Status> {
    no_scratch()
}
pub(in crate::foundation) fn execute_scratch(_count: u32) -> Result<HostRequirements, Status> {
    no_scratch()
}
pub(in crate::foundation) fn inheritance_scratch(
    _colors: u32,
) -> Result<HostRequirements, Status> {
    no_scratch()
}
pub(in crate::foundation) fn split_scratch(_count: u32) -> Result<HostRequirements, Status> {
    no_scratch()
}

pub struct Split;
pub struct GeneratedLayout;
pub struct GeneratedSet;
pub struct Binary;
pub struct Fence;

impl Split {
    pub(in crate::foundation) fn create(_d: &Device) -> Result<Box<Self>, Status> {
        Err(UNSUPPORTED)
    }
}

impl GeneratedLayout {
    /// # Safety
    /// Live device and readable token array; object borrows its device.
    pub(in crate::foundation) unsafe fn create(
        _d: &Device,
        _desc: &GeneratedLayoutDesc,
    ) -> Result<Box<Self>, Status> {
        Err(UNSUPPORTED)
    }
}

impl GeneratedSet {
    /// # Safety
    /// Live device and initial executable, externally synchronized.
    pub(in crate::foundation) unsafe fn create(
        _d: &Device,
        _desc: &GeneratedSetDesc,
    ) -> Result<Box<Self>, Status> {
        Err(UNSUPPORTED)
    }
    /// # Safety
    /// Exclusive set access; no pending reference to updated slots.
    pub(in crate::foundation) unsafe fn update(
        &self,
        _writes: &[GeneratedSetWrite],
    ) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
}

impl Binary {
    pub(in crate::foundation) fn create(_d: &Device) -> Result<Box<Self>, Status> {
        Err(UNSUPPORTED)
    }
}

impl Fence {
    pub(in crate::foundation) fn create(_d: &Device, _signaled: bool) -> Result<Box<Self>, Status> {
        Err(UNSUPPORTED)
    }
    pub(in crate::foundation) fn poll(&self) -> Result<bool, Status> {
        Err(UNSUPPORTED)
    }
    pub(in crate::foundation) fn wait(&self, _timeout: u64) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
    pub(in crate::foundation) fn reset(&self) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
}

impl Queue {
    /// # Safety
    /// Exclusive queue access; see the presentation contract in ogpu_next.h.
    pub(in crate::foundation) unsafe fn present(
        &self,
        desc: &PresentDesc,
    ) -> Result<bool, Status> {
        desc.header.validate::<PresentDesc>(PRESENT_DESC)?;
        if desc.image_count == 0 {
            return Err(INVALID);
        }
        if desc.wait_count != 0 || !desc.waits.is_null() {
            return Err(UNSUPPORTED);
        }
        // SAFETY: trusted image array of the declared count.
        let images = unsafe { super::slice(desc.images, desc.image_count) }?;
        for index in 0..desc.image_count as usize {
            let image = &images[index];
            if image.reserved != 0 {
                return Err(INVALID);
            }
            if !image.fence.is_null() {
                return Err(UNSUPPORTED);
            }
            // SAFETY: live swapchain behind the borrowed pointer.
            let chain = unsafe { image.swapchain.as_ref() }.ok_or(INVALID)?;
            if image.index as usize >= chain.image_count() {
                return Err(INVALID);
            }
            // SAFETY: exclusive queue access per the contract.
            let status = unsafe { chain.present_native() };
            if !desc.results.is_null() {
                // SAFETY: caller storage covers one status per image.
                unsafe { *desc.results.add(index) = status };
            }
            if status != OK {
                return Ok(false);
            }
        }
        Ok(false)
    }
}


impl Device {
    pub(in crate::foundation) unsafe fn generated_requirements(
        &self,
        _work: &GeneratedWork,
    ) -> Result<GeneratedRequirements, Status> {
        Err(UNSUPPORTED)
    }
}

impl List {
    pub(in crate::foundation) fn draw_mesh(&self, _groups: Extent) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
    pub(in crate::foundation) fn draw_mesh_indirect(
        &self,
        _desc: &Indirect,
    ) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
    /// # Safety
    /// Batch/state storage stays live through all recorded/pending uses.
    pub(in crate::foundation) unsafe fn generated_preprocess(
        &self,
        _batch: &GeneratedBatch,
        _state: &List,
    ) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
    /// # Safety
    /// Batch storage stays live through all recorded/pending uses.
    pub(in crate::foundation) unsafe fn generated_execute(
        &self,
        _batch: &GeneratedBatch,
        _preprocessed: u32,
    ) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
    pub(in crate::foundation) fn transfer_image(
        &self,
        _dst: &Image,
        _src: &Image,
        _transfer: &ImageTransfer,
        _mode: u32,
    ) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
    pub(in crate::foundation) fn blit_image(
        &self,
        _dst: &Image,
        _src: &Image,
        _blit: &ImageBlit,
    ) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
    /// # Safety
    /// Referenced secondary lists/arenas outlive all recorded/pending uses.
    pub(in crate::foundation) unsafe fn execute(
        &self,
        _count: u32,
        _lists: *const *mut List,
        _scratch: HostSpan,
    ) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
    pub(in crate::foundation) fn split_reset(
        &self,
        _split: &Split,
        _before: u64,
    ) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
    /// # Safety
    /// Caller guarantees matching acquire and event-cycle ordering.
    pub(in crate::foundation) fn split_release(
        &self,
        _split: &Split,
        _dependency: &Dependency,
    ) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
    /// # Safety
    /// Dependency arrays and scratch valid and disjoint for the call.
    pub(in crate::foundation) unsafe fn split_acquire(
        &self,
        _count: u32,
        _waits: *const SplitDependency,
        _scratch: HostSpan,
    ) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
}
