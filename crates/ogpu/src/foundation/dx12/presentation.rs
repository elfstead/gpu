//! D3D12 presentation boundary. Not implemented in D1: DXGI swapchains bind to
//! externally supplied windows through a surface record that lands in D3.
use super::types::*;
use super::Adapter;
use windows::Win32::Graphics::Dxgi as dxgi;

/// Opaque surface owner; creation is unsupported in this batch.
pub struct Surface;

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
    /// Non-null native window handles would have to remain live; unsupported here.
    pub(in crate::foundation) unsafe fn create(
        _adapter: &Adapter,
        _desc: &WaylandSurfaceDesc,
    ) -> Result<Box<Self>, Status> {
        Err(UNSUPPORTED)
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
