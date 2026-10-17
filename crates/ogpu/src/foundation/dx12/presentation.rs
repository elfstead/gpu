//! D3D12 presentation boundary. Not implemented in D1: DXGI swapchains bind to
//! externally supplied windows through a surface record that lands in D3.
use super::types::*;
use super::Adapter;

/// Opaque surface owner; creation is unsupported in this batch.
pub struct Surface;

/// Report the presentation profiles of this adapter. Nothing is enabled.
pub(in crate::foundation) fn support(_adapter: &Adapter) -> Result<PresentationSupport, Status> {
    Ok(PresentationSupport {
        swapchain: 0,
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
