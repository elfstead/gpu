//! Optional native presentation setup. No window ownership or scheduling.
use super::*;

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
    unsafe { surface.get().query(adapter, query) }
}

/// # Safety
/// All swapchains, host accesses and uses are gone; native Wayland handles remain
/// live through destruction. Destroy exactly once. Never waits for GPU work.
pub unsafe fn surface_destroy(surface: SurfaceHandle) {
    unsafe {
        drop(Box::from_raw(surface.as_raw()));
    }
}
