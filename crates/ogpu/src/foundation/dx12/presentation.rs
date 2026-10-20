//! D3D12 presentation boundary. Not implemented in D1: DXGI swapchains bind to
//! externally supplied windows through a surface record that lands in D3.
use super::types::*;
use super::native::formats;
use super::native::Image;
use super::native::{map_error, slice as native_slice};
use super::Adapter;
use super::Device;
use std::slice;
use windows::Win32::Graphics::Dxgi as dxgi;
use windows::Win32::Graphics::Dxgi::Common as dxgic;

/// Copy query values into caller storage with the contract's capacity
/// semantics: the capacity names element slots and the count reports how
/// many were written.
unsafe fn copy_out<T: Copy>(query: &mut Query, values: &[T]) -> Result<(), Status> {
    let count = values.len().min(query.capacity as usize);
    // SAFETY: caller storage covers capacity elements of the queried type.
    unsafe {
        std::ptr::copy_nonoverlapping(
            values.as_ptr(),
            query.data.cast::<T>(),
            count,
        );
    }
    query.count = count as u32;
    if count < values.len() {
        return Err(CAPACITY);
    }
    Ok(())
}

/// Opaque surface owner: the externally supplied native window this
/// backend's swapchains bind to. The window stays the caller's (the contract:
/// window/input handling is external and borrowed, never owned).
pub struct Surface {
    window: isize,
    adapter: *const Adapter,
}

/// A DXGI swapchain bound to one surface. Images are the chain's buffers
/// wrapped as borrowed foundation images; the chain owns nothing about
/// windowing and no acquire protocol is invented (DXGI's native model is the
/// current back buffer plus frame latency).
pub struct Swapchain {
    device: *const Device,
    chain: dxgi::IDXGISwapChain3,
    images: Vec<Box<Image>>,
    extent: Extent,
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
    /// Live surface and writable query storage.
    pub(in crate::foundation) unsafe fn query(
        &self,
        adapter: &Adapter,
        mode: Option<u32>,
        query: *mut Query,
    ) -> Result<(), Status> {
        let q = unsafe { query.as_mut() }.ok_or(INVALID)?;
        // The same element contract as the shared query validation: the
        // element size names the query's value type exactly.
        let size = match q.header.kind {
            SURFACE_CAPABILITIES => size_of::<SurfaceCapabilities>(),
            SURFACE_FORMATS => size_of::<SurfaceFormat>(),
            SURFACE_PRESENT_MODES => size_of::<u32>(),
            SURFACE_QUEUES => size_of::<SurfaceQueue>(),
            SURFACE_SCALING => size_of::<SurfaceScaling>(),
            SURFACE_COMPATIBLE_MODES => size_of::<u32>(),
            _ => return Err(UNSUPPORTED),
        };
        if q.reserved != 0
            || q.element_size as usize != size
            || (q.capacity != 0 && q.data.is_null())
        {
            return Err(INVALID);
        }
        if mode.is_some() {
            // Mode-scoped queries arrive with the swapchain work.
            return Err(UNSUPPORTED);
        }
        match q.header.kind {
            SURFACE_QUEUES => {
                // DXGI presents from any graphics-capable direct queue: the
                // swapchain binds to the device and its command queue.
                let mut queues = Vec::new();
                for queue in &adapter.snapshot.queues {
                    queues
                        .push(SurfaceQueue {
                            domain: queue.domain,
                            supported: u32::from(queue.flags & GRAPHICS != 0),
                        });
                }
                unsafe { copy_out(q, &queues) }
            }
            SURFACE_FORMATS => {
                // The DXGI flip-model swapchain formats; native_format carries
                // the DXGI_FORMAT value.
                let formats = [
                    (RGBA8_UNORM, 28u32),
                    (0, 87u32),
                    (0, 24u32),
                    (0, 29u32),
                    (0, 28u32),
                ];
                let values: Vec<SurfaceFormat> = formats
                    .iter()
                    .map(|&(format, native)| SurfaceFormat {
                        format,
                        native_format: native,
                        color_space: 0,
                        reserved: 0,
                    })
                    .collect();
                unsafe { copy_out(q, &values) }
            }
            SURFACE_PRESENT_MODES => {
                // DXGI sync intervals: immediate (0) and the vsync fifo (2 in
                // the shared present-mode numbering). Mailbox has no DXGI
                // analog and is not reported.
                let modes = [0u32, 2];
                unsafe { copy_out(q, &modes) }
            }
            SURFACE_CAPABILITIES => {
                // The window extent is measured, not guessed.
                let mut rect = windows::Win32::Foundation::RECT::default();
                // SAFETY: the borrowed window is live per the surface contract.
                unsafe {
                    windows::Win32::UI::WindowsAndMessaging::GetClientRect(
                        windows::Win32::Foundation::HWND(self.window as *mut _),
                        &mut rect,
                    )
                }
                .map_err(map_error)?;
                let extent = [
                    (rect.right - rect.left).max(0) as u32,
                    (rect.bottom - rect.top).max(0) as u32,
                ];
                let capabilities = SurfaceCapabilities {
                    // DXGI flip-model buffer counts; the swapchain work
                    // measures what the driver accepts beyond these.
                    min_image_count: 2,
                    max_image_count: 16,
                    current_extent: extent,
                    min_extent: [0, 0],
                    max_extent: [16384, 16384],
                    max_layers: 1,
                    supported_transforms: 1,
                    current_transform: 1,
                    supported_alpha: 1,
                    image_usage: 16 | 4 | 8,
                    native_image_usage: 16 | 4 | 8,
                    reserved: 0,
                };
                unsafe { copy_out(q, &[capabilities]) }
            }
            SURFACE_SCALING | SURFACE_COMPATIBLE_MODES => {
                // Measured with the swapchain work (what DXGI reports for the
                // live chain), never guessed.
                Err(UNSUPPORTED)
            }
            _ => Err(INVALID),
        }
    }
}
