//! Optional native presentation setup. No window ownership or scheduling.
use super::*;

/// Create the exact native chain profile. Output always reports the previous
/// chain state; native replacement retires it even if creation fails.
/// # Safety
/// Device, surface and old chain are live and externally synchronized. A complete
/// record of its declared type is readable, including all nested arrays. Parents
/// outlive the new chain. Acquired old images retain their native use permissions.
pub unsafe fn chain_create(
    device: DeviceHandle,
    desc: &Record,
    out: &mut ChainCreateOutcome,
) -> Result<()> {
    unsafe { native::chain_create(device.get(), desc, out) }
}
/// # Safety
/// All render, acquisition and presentation uses are retired and views destroyed.
/// Parent device/surface remain live; destroy once. No implicit wait. All borrowed
/// image tokens become invalid, including previously recorded references.
pub unsafe fn chain_destroy(chain: SwapchainHandle) {
    unsafe {
        drop(Box::from_raw(chain.as_raw()));
    }
}
/// Host snapshot, not a health poll. Does not allocate or call the native driver.
/// # Safety
/// Chain is live; serialize chain lifecycle and all borrowed-image access.
pub unsafe fn chain_info_query(chain: SwapchainHandle) -> ChainInfo {
    unsafe { chain.get() }.info()
}
/// Query count with an empty slice; otherwise fill the whole borrowed-image array.
/// On CAPACITY only count changes. No allocation or native call.
/// # Safety
/// Chain/device are live and output is disjoint. Borrowed images are valid until
/// invalidation/destruction, must not be rebound, and are usable only while acquired
/// with acquisition ordered. Retire all uses/views before invalidation/destruction.
pub unsafe fn chain_images(
    chain: SwapchainHandle,
    out: &mut [*mut super::super::Image],
    count_out: &mut u32,
) -> Result<()> {
    unsafe {
        chain
            .get()
            .images(count(out.len())?, out.as_mut_ptr(), count_out)
    }
}
/// Native in-place resize when advertised; Vulkan returns unchanged/UNSUPPORTED.
/// # Safety
/// Exclusive chain access. Retire acquisitions/GPU uses and remove dependent views,
/// recordings and native references first. Invalidation revokes all old images even
/// on failure, increments generation and never adds an implicit drain or rollback.
pub unsafe fn chain_resize(
    chain: SwapchainHandle,
    desc: &ChainResizeDesc,
    out: &mut ChainResizeOutcome,
) -> Result<()> {
    unsafe { chain.get() }.resize(desc, out)
}
/// Borrow the native pacing handle when advertised; failure preserves output.
/// # Safety
/// Chain stays live and synchronized. Caller must not close the borrowed handle;
/// its validity ends with the chain. Waiting/event-loop policy remains caller-owned.
pub unsafe fn chain_pacing_handle(chain: SwapchainHandle, out: &mut PacingHandle) -> Result<()> {
    unsafe { chain.get() }.pacing_handle(out)
}
/// Publish a borrowed frame only on OK/SUBOPTIMAL; errors preserve output.
/// # Safety
/// Exclusive chain/acquire-object access; chain is active. Vulkan requires >=1 live
/// same-device idle unsignaled binary/fence. Obey native acquired-count/forward-
/// progress limits. Order image access after acquisition. Copies confer no new lease.
pub unsafe fn acquire(chain: SwapchainHandle, desc: &AcquireDesc, out: &mut Frame) -> Result<()> {
    unsafe { chain.get().acquire_frame(desc, out) }
}
/// # Safety
/// Device is live with presentation enabled; no GPU or chain access is performed.
pub unsafe fn abandon_scratch_requirements(
    device: DeviceHandle,
    count: u32,
) -> Result<HostRequirements> {
    native::abandon_requirements(unsafe { device.get() }, count)
}
/// End acquisitions without presenting. Effects remain meaningful on error.
/// # Safety
/// Exclusive chain access. Nonempty unique acquired frames all name this chain;
/// all acquisition and device uses have completed. Scratch is writable and disjoint.
/// Release does not consume acquisition binary signals: retire them separately.
pub unsafe fn abandon(
    chain: SwapchainHandle,
    frames: &[Frame],
    scratch: HostSpan,
    effect: &mut u32,
) -> Result<()> {
    unsafe { chain.get().abandon(frames, scratch, effect) }
}
/// # Safety
/// Device is live with presentation enabled; no GPU or chain access is performed.
pub unsafe fn present_scratch_requirements(
    device: DeviceHandle,
    waits: u32,
    items: u32,
) -> Result<HostRequirements> {
    native::present_requirements(unsafe { device.get() }, waits, items)
}
/// One native Vulkan batch. Read both aggregate and per-item effects, even on error.
/// # Safety
/// Live same-device queue/objects and readable records; exclusive queue access.
/// Each frame is acquired, STATE_PRESENT, owned/shared for this queue; writes are
/// ordered by the supplied binary waits. No duplicate chain/wait/non-null fence;
/// fences are idle/unsignaled. Scratch and required per-item results are writable
/// and disjoint from each other, inputs and output. NONE/UNKNOWN/ENQUEUED do not
/// mean completion. Retire rendering and presentation separately. Historical
/// acquisition pointers in frames are never dereferenced by this operation.
pub unsafe fn queue_present(
    queue: QueueHandle,
    desc: &PresentDesc,
    out: &mut PresentOutcome,
) -> Result<()> {
    unsafe { queue.get().present_frames(desc, out) }
}
/// Query native availability/capabilities, without enabling anything.
/// # Safety
/// Surface/native window remain live and externally synchronized with lifecycle.
pub unsafe fn presentation_contract_query(
    adapter: &Adapter,
    surface: SurfaceHandle,
    profile: u32,
) -> Result<PresentationContract> {
    native::presentation_contract(adapter, unsafe { surface.get() }, profile)
}
/// Typed platform namespace. Unsupported on the current Vulkan/Wayland mapping.
/// # Safety
/// A supported mapping requires the native window to remain live through surface
/// destruction, with externally synchronized native-window/surface lifecycle.
pub unsafe fn surface_win32_create(
    _adapter: &Adapter,
    desc: &Win32SurfaceDesc,
) -> Result<SurfaceHandle> {
    desc.header
        .validate::<Win32SurfaceDesc>(WIN32_SURFACE_DESC)?;
    if desc.hwnd.is_null() {
        return Err(INVALID);
    }
    Err(UNSUPPORTED)
}

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
