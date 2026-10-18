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
pub(in crate::foundation) fn vertex_scratch(_count: u32) -> Result<HostRequirements, Status> {
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

pub struct ExecutableCache;
pub struct QueryPool;
pub struct Split;
pub struct GeneratedLayout;
pub struct GeneratedSet;
pub struct Binary;
pub struct Fence;
pub struct Swapchain;

impl ExecutableCache {
    /// # Safety
    /// Live device and initial cache bytes; declared synchronization applies.
    pub(in crate::foundation) unsafe fn create(
        _d: &Device,
        _desc: &ExecutableCacheDesc,
    ) -> Result<Box<Self>, Status> {
        Err(UNSUPPORTED)
    }
    /// # Safety
    /// Live cache; exclusive mutation/export access; writable output storage.
    pub(in crate::foundation) unsafe fn data(
        &self,
        _size: *mut usize,
        _data: *mut c_void,
    ) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
    /// # Safety
    /// Live same-device caches; exclusive destination, no source mutation.
    pub(in crate::foundation) unsafe fn merge(
        &self,
        _count: u32,
        _sources: *const *mut ExecutableCache,
    ) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
}

impl QueryPool {
    /// # Safety
    /// Live device with declared query features; device outlives the pool.
    pub(in crate::foundation) fn create(
        _d: &Device,
        _desc: &QueryPoolDesc,
    ) -> Result<Box<Self>, Status> {
        Err(UNSUPPORTED)
    }
}

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

impl Swapchain {
    /// # Safety
    /// Live device/surface descriptions; external window objects stay live.
    pub(in crate::foundation) unsafe fn create(
        _d: &Device,
        _desc: &SwapchainDesc,
    ) -> Result<Box<Self>, Status> {
        Err(UNSUPPORTED)
    }
    /// # Safety
    /// Live chain; writable borrowed-image output storage.
    pub(in crate::foundation) unsafe fn images(
        &self,
        _count: u32,
        _out: *mut *mut Image,
        _count_out: *mut u32,
    ) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
    /// # Safety
    /// Exclusive chain host access; valid acquire objects.
    pub(in crate::foundation) unsafe fn acquire(
        &self,
        _timeout_ns: u64,
        _binary: *mut Binary,
        _fence: *mut Fence,
    ) -> Result<(u32, bool), Status> {
        Err(UNSUPPORTED)
    }
    /// # Safety
    /// Exclusive chain host access; unique acquired indices.
    pub(in crate::foundation) fn release(&self, _indices: &[u32]) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
}

impl Queue {
    /// # Safety
    /// Exclusive queue access; see the presentation contract in ogpu_next.h.
    pub(in crate::foundation) unsafe fn present(
        &self,
        _desc: &PresentDesc,
    ) -> Result<bool, Status> {
        Err(UNSUPPORTED)
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
    pub(in crate::foundation) fn draw_indirect(
        &self,
        _desc: &Indirect,
        _indexed: bool,
    ) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
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
    /// # Safety
    /// Image and clear value follow the clear-value member contract.
    pub(in crate::foundation) fn clear_image(
        &self,
        _image: &Image,
        _state: u32,
        _range: Subresources,
        _value: ClearValue,
    ) -> Result<(), Status> {
        // D3D12 clears need view descriptors; the descriptor profile (D2) owns
        // the native alternative. Never synthesized via staging here.
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
    pub(in crate::foundation) fn queries_reset(
        &self,
        _pool: &QueryPool,
        _first: u32,
        _count: u32,
    ) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
    pub(in crate::foundation) fn query(
        &self,
        _pool: &QueryPool,
        _index: u32,
        _begin: bool,
        _flags: u32,
    ) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
    pub(in crate::foundation) fn timestamp(
        &self,
        _pool: &QueryPool,
        _index: u32,
        _scope: u64,
    ) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
    /// # Safety
    /// Destination span is live and writable through all recorded uses.
    pub(in crate::foundation) unsafe fn queries_resolve(
        &self,
        _pool: &QueryPool,
        _first: u32,
        _count: u32,
        _dst: Span,
        _stride: u64,
        _flags: u32,
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
