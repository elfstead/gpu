//! Native Rust foundation operations, below the C ABI.
//!
//! Handles are non-owning tokens, not RAII owners. Copying or dropping one does
//! not retain or destroy a resource. Explicit unsafe destruction never waits.
//! GPU-address validity, asynchronous lifetime and object-local synchronization
//! remain caller obligations, as specified in `include/ogpu_next.h`.
//! Records reuse `types`, including trusted nested metadata and address pointers.
//! Rust and C share backend validation/execution and recording-error state.
use super::types::*;
use super::{native, Adapter, Discovery};
use std::ptr::NonNull;

pub type Result<T> = std::result::Result<T, Status>;

macro_rules! handle {
    ($name:ident, $raw:ident) => {
        /// Non-owning handle. All dereferencing operations require a live object.
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct $name(NonNull<super::$raw>);
        // SAFETY: transferring a token does not access its pointee. All operations
        // that do so are unsafe and require the documented host synchronization.
        unsafe impl Send for $name {}
        unsafe impl Sync for $name {}
        impl $name {
            /// Borrow the same handle for C interoperability; transfers no ownership.
            pub fn as_raw(self) -> *mut super::$raw {
                self.0.as_ptr()
            }
            /// Adopt a token without retaining its resource. Null yields None.
            ///
            /// # Safety
            /// Non-null must identify an object of the matching foundation type.
            /// Its original ownership and synchronization obligations remain.
            pub unsafe fn from_raw(raw: *mut super::$raw) -> Option<Self> {
                NonNull::new(raw).map(Self)
            }
            #[allow(dead_code)] // Arena only needs exclusive access in this tranche.
            unsafe fn get<'a>(self) -> &'a super::$raw {
                unsafe { self.0.as_ref() }
            }
        }
    };
}
handle!(DeviceHandle, Device);
handle!(MemoryHandle, Memory);
handle!(TimelineHandle, Timeline);
handle!(QueueHandle, Queue);
handle!(ArenaHandle, Arena);
handle!(ListHandle, List);
handle!(ImageHandle, Image);
handle!(ViewHandle, View);
handle!(ExecutableHandle, Executable);
handle!(GeneratedLayoutHandle, GeneratedLayout);
handle!(GeneratedSetHandle, GeneratedSet);
handle!(ExecutableCacheHandle, ExecutableCache);
handle!(QueryPoolHandle, QueryPool);
handle!(SplitHandle, Split);
handle!(SurfaceHandle, Surface);
handle!(BinaryHandle, Binary);
handle!(FenceHandle, Fence);
handle!(SwapchainHandle, Swapchain);

mod commands;
mod generated;
mod presentation;
mod resources;
pub use commands::*;
pub use generated::*;
pub use presentation::*;
pub use resources::*;

fn count(len: usize) -> Result<u32> {
    u32::try_from(len).map_err(|_| CAPACITY)
}

/// Discover immutable adapters. This owner may be dropped normally; created
/// devices independently retain the loader/instance, not a borrowed adapter.
pub fn discover() -> Result<Box<Discovery>> {
    #[cfg(target_os = "windows")]
    crate::dx12_sdk::touch();
    native::discover()
}
pub fn adapters(discovery: &Discovery) -> &[Adapter] {
    &discovery.adapters
}
pub fn adapter_info(adapter: &Adapter) -> &AdapterInfo {
    &adapter.snapshot.info
}
pub fn adapter_queues(adapter: &Adapter) -> &[QueueInfo] {
    &adapter.snapshot.queues
}
pub fn adapter_features(adapter: &Adapter) -> &FeatureInfo {
    &adapter.snapshot.features
}
pub fn adapter_memory_types(adapter: &Adapter) -> &[MemoryTypeInfo] {
    &adapter.snapshot.memory_types
}

/// Versioned query escape hatch; simple immutable queries also have typed views.
/// # Safety
/// Query output must be writable, aligned and disjoint, with its declared capacity.
pub unsafe fn adapter_query(adapter: &Adapter, query: *mut Query) -> Result<()> {
    unsafe { adapter.snapshot.query(query) }
}

/// Create a device with exact queues/features. The returned token needs explicit
/// destruction; losing it leaks the resource rather than implicitly waiting.
pub fn device_create(
    adapter: &Adapter,
    requests: &[QueueRequest],
    features: u64,
) -> Result<DeviceHandle> {
    if requests.is_empty() || requests.len() > adapter.snapshot.queues.len() {
        return Err(INVALID);
    }
    validate_requests(&adapter.snapshot.queues, requests)?;
    validate_feature_dependencies(features)?;
    if adapter.snapshot.features.baseline_supported == 0
        || features & !adapter.snapshot.features.available != 0
    {
        return Err(UNSUPPORTED);
    }
    Ok(DeviceHandle(NonNull::from(Box::leak(
        super::Device::create(adapter, requests, features)?,
    ))))
}
/// # Safety
/// Device is live; query output has its declared writable capacity and is disjoint.
pub unsafe fn device_query(device: DeviceHandle, query: *mut Query) -> Result<()> {
    unsafe { device.get().snapshot.query(query) }
}
/// # Safety
/// Same storage and lifetime obligations as device_query. This samples live budgets.
pub unsafe fn memory_budget_query(device: DeviceHandle, query: *mut Query) -> Result<()> {
    unsafe { device.get().memory_budget_query(query) }
}
/// # Safety
/// Device is live through this call and all subsequent uses of the borrowed queue.
pub unsafe fn device_queue(device: DeviceHandle, domain: u32, index: u32) -> Option<QueueHandle> {
    unsafe { device.get() }
        .queue(domain, index)
        .map(|q| QueueHandle(NonNull::from(q)))
}
/// # Safety
/// All children, recorded/pending work and host access must be retired. Exactly
/// one destruction is allowed across every copy of this handle. Never waits.
pub unsafe fn device_destroy(device: DeviceHandle) {
    unsafe {
        drop(Box::from_raw(device.as_raw()));
    }
}
/// # Safety
/// Device is live and outlives the timeline and all its uses.
pub unsafe fn timeline_create(device: DeviceHandle, initial: u64) -> Result<TimelineHandle> {
    Ok(TimelineHandle(NonNull::from(Box::leak(
        super::Timeline::create(unsafe { device.get() }, initial)?,
    ))))
}
/// # Safety
/// Live timeline/device; all host access obeys the synchronization contract.
pub unsafe fn timeline_poll(timeline: TimelineHandle) -> Result<u64> {
    unsafe { timeline.get() }.poll()
}
/// # Safety
/// Timeline/device remain live throughout the wait. Timeout does not retire work.
pub unsafe fn timeline_wait(timeline: TimelineHandle, value: u64, timeout_ns: u64) -> Result<()> {
    unsafe { timeline.get() }.wait(value, timeout_ns)
}
/// # Safety
/// Live timeline/device; caller orders monotonic values against all host/GPU signals.
pub unsafe fn timeline_signal(timeline: TimelineHandle, value: u64) -> Result<()> {
    unsafe { timeline.get() }.signal(value)
}
/// # Safety
/// No recorded, pending or host uses remain; parent device is live. Destroy once.
pub unsafe fn timeline_destroy(timeline: TimelineHandle) {
    unsafe {
        drop(Box::from_raw(timeline.as_raw()));
    }
}

/// # Safety
/// Live device; all embedded arrays and output storage are valid and disjoint.
pub unsafe fn memory_requirements(
    device: DeviceHandle,
    desc: &MemoryDesc,
    out: &mut Requirements,
) -> Result<()> {
    unsafe { device.get().memory_requirements(desc, out) }
}
/// # Safety
/// Live device and description arrays; device outlives backing and every use.
pub unsafe fn memory_create(device: DeviceHandle, desc: &MemoryDesc) -> Result<MemoryHandle> {
    Ok(MemoryHandle(NonNull::from(Box::leak(unsafe {
        super::Memory::create(device.get(), desc)?
    }))))
}
impl MemoryHandle {
    /// Describe a range without accessing or retaining backing. Validity is checked
    /// by operations; lifetime and GPU access synchronization remain caller-owned.
    pub fn span(self, offset: u64, size: u64) -> Span {
        Span {
            memory: self.as_raw(),
            offset,
            size,
        }
    }
}
/// # Safety
/// Span backing/device is live. Returned address does not retain it.
pub unsafe fn memory_address(span: Span) -> Result<u64> {
    unsafe { span.memory.as_ref() }
        .ok_or(INVALID)?
        .address(span.offset, span.size)
}
/// # Safety
/// Span backing/device is live; externally serialize mapping changes. Caller
/// maintains CPU/GPU access ordering and retains backing for every returned view.
pub unsafe fn memory_map(span: Span) -> Result<Mapping> {
    unsafe { span.memory.as_ref() }
        .ok_or(INVALID)?
        .map(span.offset, span.size)
}
/// # Safety
/// Live backing/device; no view/cache user may race this operation.
pub unsafe fn memory_unmap(memory: MemoryHandle) {
    unsafe { memory.get() }.unmap();
}
/// # Safety
/// Live span; caller orders CPU/GPU access including neighboring cache-atom bytes.
pub unsafe fn memory_flush(span: Span) -> Result<()> {
    unsafe { span.memory.as_ref() }
        .ok_or(INVALID)?
        .cache(span.offset, span.size, true)
}
/// # Safety
/// Same access-ordering and lifetime obligations as memory_flush.
pub unsafe fn memory_invalidate(span: Span) -> Result<()> {
    unsafe { span.memory.as_ref() }
        .ok_or(INVALID)?
        .cache(span.offset, span.size, false)
}
/// # Safety
/// No views, recorded, pending or host uses remain; live parent device. Destroy once.
pub unsafe fn memory_destroy(memory: MemoryHandle) {
    unsafe {
        drop(Box::from_raw(memory.as_raw()));
    }
}

/// # Safety
/// Live device and valid embedded description storage; device outlives arena/uses.
pub unsafe fn arena_create(device: DeviceHandle, desc: &ArenaDesc) -> Result<ArenaHandle> {
    Ok(ArenaHandle(NonNull::from(Box::leak(super::Arena::create(
        unsafe { device.get() },
        desc,
    )?))))
}
/// # Safety
/// Live arena/device, exclusive pool access, no simultaneous encoder/list access.
pub unsafe fn arena_reserve(arena: ArenaHandle, count: u32) -> Result<()> {
    unsafe { &mut *arena.as_raw() }.reserve(count)
}
/// # Safety
/// Live arena/device, exclusive pool access and no pending use. Invalidates lists.
pub unsafe fn arena_reset(arena: ArenaHandle) -> Result<()> {
    unsafe { &mut *arena.as_raw() }.reset(false)
}
/// # Safety
/// Same exclusion and lifetime requirements as arena_reset.
pub unsafe fn arena_trim(arena: ArenaHandle, count: u32) -> Result<()> {
    unsafe { &mut *arena.as_raw() }.trim(count)
}
/// # Safety
/// No pending/host uses; parent device live. Invalidates all lists. Destroy once.
pub unsafe fn arena_destroy(arena: ArenaHandle) {
    unsafe {
        drop(Box::from_raw(arena.as_raw()));
    }
}
/// # Safety
/// Exclusive live pool; all nested description arrays/scratch valid and disjoint.
pub unsafe fn commands_begin(arena: ArenaHandle, desc: &RecordingDesc) -> Result<ListHandle> {
    let list = unsafe { &mut *arena.as_raw() }.begin(desc)?;
    Ok(ListHandle(NonNull::new(list).ok_or(INTERNAL_ERROR)?))
}
/// # Safety
/// Live recording slot; exclusive pool access. Reports a poisoned recording.
pub unsafe fn commands_end(list: ListHandle) -> Result<ListHandle> {
    unsafe { list.get() }.end()?;
    Ok(list)
}
/// # Safety
/// Live recording slot; exclusive pool access. Does not cancel submitted GPU work.
pub unsafe fn commands_cancel(list: ListHandle) {
    unsafe { list.get() }.cancel();
}

/// Record an operation, preserving error-at-end semantics without a C panic boundary.
pub(super) unsafe fn encode(list: ListHandle, f: impl FnOnce(&super::List) -> Result<()>) {
    let list = unsafe { list.get() };
    // Mark unwinding as poisoned too, without catching the panic on the Rust path.
    struct Poison<'a>(&'a super::List, bool);
    impl Drop for Poison<'_> {
        fn drop(&mut self) {
            if self.1 {
                self.0.poison(INTERNAL_ERROR);
            }
        }
    }
    let mut guard = Poison(list, true);
    list.poison(f(list).err().unwrap_or(OK));
    guard.1 = false;
}
/// # Safety
/// Exclusive live recording pool; destination backing/device remains live through
/// recording and execution, with correctly ordered GPU accesses.
pub unsafe fn fill_memory(list: ListHandle, dst: Span, pattern: u32) {
    unsafe {
        encode(list, |e| e.fill(dst, pattern));
    }
}
/// # Safety
/// Same obligations as fill_memory, for both source and destination ranges.
pub unsafe fn copy_memory(list: ListHandle, dst: Span, src: Span) {
    unsafe {
        encode(list, |e| e.copy(dst, src));
    }
}
/// # Safety
/// Exclusive live pool; dependency arrays and scratch valid/disjoint, referenced
/// resources live; caller supplies correct execution/access/ownership ordering.
pub unsafe fn barrier(list: ListHandle, dependency: &Dependency) {
    unsafe {
        encode(list, |e| e.barrier(dependency));
    }
}
pub fn submit_scratch_requirements(
    lists: u32,
    waits: u32,
    signals: u32,
) -> Result<HostRequirements> {
    native::submit_scratch(lists, waits, signals)
}
pub fn barrier_scratch_requirements(memory: u32, images: u32) -> Result<HostRequirements> {
    native::barrier_scratch(memory, images)
}
/// # Safety
/// Live queue with exclusive host access; valid disjoint descriptor arrays/scratch.
/// Caller retains resources/lists/timelines until completion and obeys replay,
/// semaphore, dependency and uncertain-submission lifetime rules from ogpu_next.h.
pub unsafe fn queue_submit(
    queue: QueueHandle,
    lists: &[ListHandle],
    waits: &[SyncPoint],
    signals: &[SyncPoint],
    scratch: HostSpan,
) -> Result<()> {
    let desc = SubmitDesc {
        header: Record::new::<SubmitDesc>(SUBMIT_DESC),
        list_count: u32::try_from(lists.len()).map_err(|_| CAPACITY)?,
        wait_count: u32::try_from(waits.len()).map_err(|_| CAPACITY)?,
        signal_count: u32::try_from(signals.len()).map_err(|_| CAPACITY)?,
        // ListHandle is transparent over NonNull<List>, with pointer layout.
        lists: lists.as_ptr().cast(),
        waits: waits.as_ptr(),
        signals: signals.as_ptr(),
        scratch: scratch.data,
        scratch_size: scratch.size,
    };
    unsafe { queue_submit_record(queue, &desc) }
}
pub(super) unsafe fn queue_submit_record(queue: QueueHandle, desc: &SubmitDesc) -> Result<()> {
    unsafe { queue.get().submit(desc) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handles_are_pointer_sized_without_drop_glue() {
        assert_eq!(
            size_of::<DeviceHandle>(),
            size_of::<*mut super::super::Device>()
        );
        assert_eq!(size_of::<Option<DeviceHandle>>(), size_of::<DeviceHandle>());
        assert!(!std::mem::needs_drop::<DeviceHandle>());
        assert!(!std::mem::needs_drop::<MemoryHandle>());
        assert!(!std::mem::needs_drop::<ArenaHandle>());
        fn transferable<T: Send + Sync>() {}
        transferable::<DeviceHandle>();
        transferable::<ListHandle>();
    }
    #[test]
    fn scratch_queries_match_c_boundary() {
        let expected = submit_scratch_requirements(1, 0, 1).unwrap();
        let mut actual = HostRequirements {
            size: 0,
            alignment: 0,
        };
        assert_eq!(
            unsafe { super::super::ogpu_next_submit_scratch_requirements(1, 0, 1, &mut actual) },
            OK
        );
        assert_eq!(actual.size, expected.size);
        assert_eq!(actual.alignment, expected.alignment);
    }

    #[test]
    #[ignore = "requires modern Vulkan; Rust unwind and C containment poison recordings"]
    fn gpu_native_recording_unwind() {
        let discovery = discover().unwrap();
        let adapter = adapters(&discovery)
            .iter()
            .find(|a| adapter_features(a).baseline_supported != 0)
            .expect("suitable device");
        let domain = adapter_queues(adapter)
            .iter()
            .find(|q| q.count > 0 && q.flags & (GRAPHICS | COMPUTE | TRANSFER) != 0)
            .unwrap()
            .domain;
        let device = device_create(
            adapter,
            &[QueueRequest {
                domain,
                count: 1,
                priority: 1.0,
            }],
            0,
        )
        .unwrap();
        // SAFETY: no submission; one thread owns the live device/pool and every slot.
        unsafe {
            let arena = arena_create(
                device,
                &ArenaDesc {
                    header: Record::new::<ArenaDesc>(ARENA_DESC),
                    domain,
                    list_capacity: 1,
                    level: 0,
                    reserved: 0,
                },
            )
            .unwrap();
            let recording = RecordingDesc {
                header: Record::new::<RecordingDesc>(RECORDING_DESC),
                replay_mode: 1,
                level: 0,
                inheritance: std::ptr::null(),
            };
            let list = commands_begin(arena, &recording).unwrap();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                encode(list, |_| panic!("native Rust recording panic"));
            }));
            assert!(result.is_err()); // Rust propagates rather than wrapping C containment.
            assert_eq!(commands_end(list), Err(INTERNAL_ERROR));
            arena_reset(arena).unwrap();
            let list = commands_begin(arena, &recording).unwrap();
            super::super::commands_api::encode(list.as_raw(), |_| panic!("C adapter containment"));
            assert_eq!(commands_end(list), Err(INTERNAL_ERROR));
            arena_reset(arena).unwrap();
            let list = commands_begin(arena, &recording).unwrap();
            encode(list, |_| Err(INVALID));
            super::super::commands_api::encode(list.as_raw(), |_| panic!("preserve first error"));
            assert_eq!(commands_end(list), Err(INVALID));
            arena_destroy(arena);
            device_destroy(device);
        }
    }
}
