//! D3D12 presentation boundary. Not implemented in D1: DXGI swapchains bind to
//! externally supplied windows through a surface record that lands in D3.
use super::types::*;
use super::Adapter;
use windows::Win32::Graphics::Dxgi as dxgi;

/// Opaque surface owner: the externally supplied native window this
/// backend's swapchains bind to. The window stays the caller's (the contract:
/// window/input handling is external and borrowed, never owned).
pub struct Surface {
    window: isize,
    adapter: *const Adapter,
}

/// Report the physical presentation profiles of this adapter, without a
/// window and without enabling anything (the contract's headless
/// `adapter_support` query). DXGI swapchain creation is a factory capability:
/// its presence is the physical swapchain profile. Swapchain maintenance
/// (parameter changes without recreation) maps to no DXGI profile this
/// backend reports yet and stays zero until the D3 swapchain work measures
/// what `ResizeBuffers` preserves.
pub(in crate::foundation) fn support(adapter: &Adapter) -> Result<PresentationSupport, Status> {
    // SAFETY: live adapter; the parent factory reference is adopted.
    let factory = unsafe {
        adapter
            .state
            .adapter
            .GetParent::<dxgi::IDXGIFactory2>()
            .ok()
    };
    Ok(PresentationSupport {
        swapchain: u32::from(factory.is_some()),
        swapchain_maintenance: 0,
    })
}

impl Surface {
    /// # Safety
    /// The record's `surface` pointer is the native window handle on this
    /// backend (provisional: `docs/dx12-presentation-profile.md`); the window
    /// must remain live and valid for every later surface/swapchain use.
    pub(in crate::foundation) unsafe fn create(
        adapter: &Adapter,
        desc: &WaylandSurfaceDesc,
    ) -> Result<Box<Self>, Status> {
        desc.header.validate::<WaylandSurfaceDesc>(WAYLAND_SURFACE_DESC)?;
        // The provisional surface decision: `display` is unused on Windows
        // and `surface` carries the externally supplied window handle.
        if !desc.display.is_null() {
            return Err(UNSUPPORTED);
        }
        if desc.surface.is_null() {
            return Err(INVALID);
        }
        Ok(Box::new(Self {
            window: desc.surface as isize,
            adapter,
        }))
    }
    /// # Safety
    /// Live surface and writable query storage; unsupported here.
    pub(in crate::foundation) unsafe fn query(
        &self,
        _adapter: &Adapter,
        _mode: Option<u32>,
        _query: *mut Query,
    ) -> Result<(), Status> {
        Err(UNSUPPORTED)
    }
}
