//! Optional native presentation setup. No window ownership or scheduling.
use super::*;

/// Create an unsignaled native binary semaphore. No WSI enabling is required.
/// # Safety
/// Device is live and outlives the semaphore and every pending use.
pub unsafe fn binary_create(device: DeviceHandle) -> Result<BinaryHandle> {
    Ok(BinaryHandle(NonNull::from(Box::leak(
        super::super::Binary::create(unsafe { device.get() })?,
    ))))
}
/// # Safety
/// All signal/wait/host uses are retired and the parent device is live. Destroy
/// exactly once. Does not wait, consume or reset a signal.
pub unsafe fn binary_destroy(binary: BinaryHandle) {
    unsafe {
        drop(Box::from_raw(binary.as_raw()));
    }
}
impl BinaryHandle {
    /// Describe an explicit dependency, without accessing or retaining its object.
    pub fn at(self, stages: u64) -> BinaryPoint {
        BinaryPoint {
            binary: self.as_raw(),
            stages,
        }
    }
}
/// Create a reusable native fence, initially signaled or unsignaled as requested.
/// # Safety
/// Device is live and outlives the fence and all its uses.
pub unsafe fn fence_create(device: DeviceHandle, signaled: bool) -> Result<FenceHandle> {
    Ok(FenceHandle(NonNull::from(Box::leak(
        super::super::Fence::create(unsafe { device.get() }, signaled)?,
    ))))
}
/// # Safety
/// Fence/device are live; exclude reset/destruction. Concurrent polls/waits are allowed.
/// A true result retires only the operation associated with this fence.
pub unsafe fn fence_poll(fence: FenceHandle) -> Result<bool> {
    unsafe { fence.get() }.poll()
}
/// # Safety
/// Fence/device stay live during the wait. Timeout does not retire any work.
/// Exclude reset/destruction; concurrent polls/waits are allowed.
pub unsafe fn fence_wait(fence: FenceHandle, timeout_ns: u64) -> Result<()> {
    unsafe { fence.get() }.wait(timeout_ns)
}
/// # Safety
/// Fence has no pending signal or other host access. Reset does not wait and does
/// not retire the old operation. It may reset an already-unsignaled idle fence.
pub unsafe fn fence_reset(fence: FenceHandle) -> Result<()> {
    unsafe { fence.get() }.reset()
}
/// # Safety
/// No pending signal or host access remains; device is live. Destroy exactly once.
pub unsafe fn fence_destroy(fence: FenceHandle) {
    unsafe {
        drop(Box::from_raw(fence.as_raw()));
    }
}

/// Submit explicit binary and timeline dependencies together, in one native submit.
/// Scratch counts are the SUM of timeline and binary waits/signals, respectively.
/// Batch callers chain BinarySubmitDesc to each SubmitDesc in queue_submit_batch.
/// # Safety
/// Same queue/list/resource/scratch obligations as queue_submit. Binary signals
/// must target an unsignaled semaphore; each signal is consumed by one ordered
/// wait. Caller proves pending signal/wait validity and retirement before reuse or
/// destruction. OGPU does not track binary state, insert bridges or wait on the host.
pub unsafe fn queue_submit_with_binary(
    queue: QueueHandle,
    lists: &[ListHandle],
    waits: &[SyncPoint],
    signals: &[SyncPoint],
    binary_waits: &[BinaryPoint],
    binary_signals: &[BinaryPoint],
    scratch: HostSpan,
) -> Result<()> {
    let binary = BinarySubmitDesc {
        header: Record::new::<BinarySubmitDesc>(BINARY_SUBMIT_DESC),
        wait_count: count(binary_waits.len())?,
        signal_count: count(binary_signals.len())?,
        waits: binary_waits.as_ptr(),
        signals: binary_signals.as_ptr(),
    };
    let mut header = Record::new::<SubmitDesc>(SUBMIT_DESC);
    header.next = &binary.header;
    let desc = SubmitDesc {
        header,
        list_count: count(lists.len())?,
        wait_count: count(waits.len())?,
        signal_count: count(signals.len())?,
        lists: lists.as_ptr().cast(),
        waits: waits.as_ptr(),
        signals: signals.as_ptr(),
        scratch: scratch.data,
        scratch_size: scratch.size,
    };
    unsafe { queue_submit_record(queue, &desc) }
}

/// Enable exactly the selected window-system instance profile. This does not
/// enable swapchains on a device or choose a surface/queue. Plain discover stays
/// headless. Currently only PRESENTATION_WAYLAND is implemented.
pub fn discover_presentation(platforms: u32) -> Result<Box<Discovery>> {
    if platforms != PRESENTATION_WAYLAND {
        return Err(UNSUPPORTED);
    }
    discover_instance(Arc::new(
        Instance::with_wayland(true).map_err(super::super::loader_error)?,
    ))
}

/// Query physical-device presentation-profile availability without a window,
/// including on a headless discovery. This enables nothing. Cold native extension
/// enumeration may allocate; enabling will still require the opt-in instance.
pub fn presentation_support(adapter: &Adapter) -> Result<PresentationSupport> {
    super::super::presentation::support(adapter)
}

/// Borrow native Wayland objects and own only the OGPU/Vulkan surface. The surface
/// retains its instance independently of discovery, not the native window.
/// # Safety
/// Non-null native handles have the specified Wayland types, belong together and
/// remain live until surface destruction. Serialize surface/window lifecycle;
/// follow the window library's thread restrictions. The wl_surface must not
/// already be associated with another Vulkan surface.
pub unsafe fn surface_wayland_create(
    adapter: &Adapter,
    desc: &WaylandSurfaceDesc,
) -> Result<SurfaceHandle> {
    Ok(SurfaceHandle(NonNull::from(Box::leak(unsafe {
        super::super::Surface::create(adapter, desc)?
    }))))
}

/// Live surface query; cold-path native enumeration may allocate/retry. It does
/// not wait for GPU work. Surface capabilities may change immediately afterwards.
/// # Safety
/// Surface/native window remain live. Query storage is aligned, writable and
/// disjoint with the declared capacity; externally synchronize surface lifecycle.
pub unsafe fn surface_query(
    adapter: &Adapter,
    surface: SurfaceHandle,
    query: *mut Query,
) -> Result<()> {
    unsafe { surface.get().query(adapter, None, query) }
}

/// Query a supported native present mode's capabilities, scaling or compatible
/// modes. Mode-specific image counts require the matching explicit mode list at
/// swapchain creation; they are not a substitute for the unqualified query.
/// # Safety
/// Same live surface/native-window, query storage and host synchronization rules
/// as surface_query. This cold query may allocate; it never waits for GPU work.
pub unsafe fn surface_mode_query(
    adapter: &Adapter,
    surface: SurfaceHandle,
    mode: u32,
    query: *mut Query,
) -> Result<()> {
    unsafe { surface.get().query(adapter, Some(mode), query) }
}

/// # Safety
/// All swapchains, host accesses and uses are gone; native Wayland handles remain
/// live through destruction. Destroy exactly once. Never waits for GPU work.
pub unsafe fn surface_destroy(surface: SurfaceHandle) {
    unsafe {
        drop(Box::from_raw(surface.as_raw()));
    }
}
