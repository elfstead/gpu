//! D3D12 presentation boundary. Not implemented in D1: DXGI swapchains bind to
//! externally supplied windows through a surface record that lands in D3.
use super::types::*;
use super::native::formats;
use super::native::Image;
use super::native::{map_error, slice as native_slice};
use super::Adapter;
use super::Device;
use std::cell::Cell;
use std::slice;
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D12 as d3d;
use windows::Win32::Graphics::Dxgi as dxgi;
use windows::Win32::Graphics::Dxgi::Common as dxgic;

/// Copy query values into caller storage with the shared query semantics:
/// the count always reports the full value count (the capacity-zero probe),
/// and a short non-zero capacity rejects with CAPACITY after writing nothing.
unsafe fn copy_out<T: Copy>(query: &mut Query, values: &[T]) -> Result<(), Status> {
    let count = values.len();
    query.count = count as u32;
    if query.capacity == 0 {
        return Ok(());
    }
    if (query.capacity as usize) < count {
        return Err(CAPACITY);
    }
    // SAFETY: caller storage covers capacity elements of the queried type.
    unsafe {
        std::ptr::copy_nonoverlapping(values.as_ptr(), query.data.cast::<T>(), count);
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
    queue: d3d::ID3D12CommandQueue,
    images: Vec<Box<Image>>,
    extent: Extent,
    sync_interval: u32,
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
        if mode.is_some() && q.header.kind != SURFACE_COMPATIBLE_MODES && q.header.kind != SURFACE_SCALING && q.header.kind != SURFACE_CAPABILITIES {
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
            SURFACE_SCALING => {
                // The DXGI swapchain scaling models; gravity is the anchor
                // for the non-stretch modes and centers by default.
                let scaling = [
                    SurfaceScaling {
                        scaling: 0,
                        gravity_x: 1,
                        gravity_y: 1,
                        reserved: 0,
                        min_extent: [1, 1],
                        max_extent: [16384, 16384],
                    },
                    SurfaceScaling {
                        scaling: 1,
                        gravity_x: 1,
                        gravity_y: 1,
                        reserved: 0,
                        min_extent: [1, 1],
                        max_extent: [16384, 16384],
                    },
                ];
                unsafe { copy_out(q, &scaling) }
            }
            SURFACE_COMPATIBLE_MODES => {
                // DXGI presents at any sync interval regardless of the chain
                // effect, so every reported mode is compatible.
                let modes = [0u32, 2];
                unsafe { copy_out(q, &modes) }
            }
            _ => Err(INVALID),
        }
    }
}

impl Swapchain {
    /// # Safety
    /// Live device/surface descriptions; external window objects stay live.
    pub(in crate::foundation) unsafe fn create(
        d: &Device,
        desc: &SwapchainDesc,
    ) -> Result<Box<Self>, Status> {
        desc.header.validate::<SwapchainDesc>(SWAPCHAIN_DESC)?;
        if desc.layer_count != 1 || desc.min_image_count == 0 {
            return Err(INVALID);
        }
        // DXGI binds one command queue per chain: an empty domain list is the
        // unconstrained default, one name binds it, more stay visible.
        if desc.domain_count > 1 {
            return Err(UNSUPPORTED);
        }
        if desc.domain_count == 1 && desc.domains.is_null() {
            return Err(INVALID);
        }
        // DXGI resizes the live chain rather than recreating swapchains;
        // old_swapchain maps to ResizeBuffers on its native chain (the design
        // record: no invented recreation semantics).
        if !desc.old_swapchain.is_null() {
            // SAFETY: live retired chain by caller contract.
            let old = unsafe { desc.old_swapchain.as_ref() }.ok_or(INVALID)?;
            let format = formats::dxgi_format(desc.format);
            let extent = Extent {
                x: desc.extent[0],
                y: desc.extent[1],
                z: 1,
            };
            // SAFETY: live chain with no pending work per the caller's
            // retirement obligations.
            unsafe {
                old.chain.ResizeBuffers(
                    desc.min_image_count,
                    extent.x,
                    extent.y,
                    format,
                    dxgi::DXGI_SWAP_CHAIN_FLAG(0),
                )
            }
            .map_err(map_error)?;
            let images = Self::wrap_buffers(d, &old.chain, desc.format, desc.usage, extent)?;
            return Ok(Box::new(Self {
                device: d,
                chain: old.chain.clone(),
                queue: old.queue.clone(),
                images,
                extent,
                sync_interval: u32::from(desc.present_mode == 2),
            }));
        }
        let surface = unsafe { desc.surface.as_ref() }.ok_or(INVALID)?;
        let domain = if desc.domain_count == 0 {
            // The unconstrained default binds the first graphics-capable
            // queue (the DXGI present path).
            d.snapshot
                .queues
                .iter()
                .find(|q| q.flags & GRAPHICS != 0)
                .ok_or(UNSUPPORTED)?
                .domain
        } else {
            let domains = unsafe { native_slice(desc.domains, 1) }?;
            domains[0]
        };
        let queue = d.command_queue(domain).ok_or(INVALID)?;
        let factory: dxgi::IDXGIFactory2 = unsafe {
            surface
                .adapter
                .as_ref()
                .ok_or(INVALID)?
                .state
                .adapter
                .GetParent::<dxgi::IDXGIFactory2>()
        }
        .map_err(map_error)?;
        let extent = Extent {
            x: desc.extent[0],
            y: desc.extent[1],
            z: 1,
        };
        let chain_desc = dxgi::DXGI_SWAP_CHAIN_DESC1 {
            Width: extent.x,
            Height: extent.y,
            Format: formats::dxgi_format(desc.format),
            Stereo: Default::default(),
            SampleDesc: dxgic::DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: if desc.usage & 16 != 0 {
                dxgi::DXGI_USAGE_RENDER_TARGET_OUTPUT
            } else {
                dxgi::DXGI_USAGE(0)
            },
            BufferCount: desc.min_image_count,
            Scaling: dxgi::DXGI_SCALING_STRETCH,
            SwapEffect: if desc.present_mode == 2 {
                dxgi::DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL
            } else {
                dxgi::DXGI_SWAP_EFFECT_FLIP_DISCARD
            },
            AlphaMode: Default::default(),
            // DXGI's immediate present requires the tearing flag and the
            // matching swapchain flag; the vsync fifo needs neither.
            Flags: if desc.present_mode == 2 {
                0
            } else {
                dxgi::DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING.0 as u32
            },
        };
        let chain: dxgi::IDXGISwapChain3 = unsafe {
            factory.CreateSwapChainForHwnd(
                queue,
                windows::Win32::Foundation::HWND(surface.window as *mut _),
                &chain_desc,
                None,
                None,
            )
        }
        .map_err(map_error)?
        .cast::<dxgi::IDXGISwapChain3>()
        .map_err(map_error)?;
        let images = Self::wrap_buffers(d, &chain, desc.format, desc.usage, extent)?;
        Ok(Box::new(Self {
            device: d,
            chain,
            queue: queue.clone(),
            images,
            extent,
            sync_interval: u32::from(desc.present_mode == 2),
        }))
    }

    /// # Safety
    /// Live chain; writable borrowed-image output storage.
    pub(in crate::foundation) unsafe fn images(
        &self,
        count: u32,
        out: *mut *mut Image,
        count_out: *mut u32,
    ) -> Result<(), Status> {
        if count_out.is_null() {
            return Err(INVALID);
        }
        unsafe { *count_out = self.images.len() as u32 };
        if count == 0 {
            return Ok(());
        }
        if out.is_null() {
            return Err(INVALID);
        }
        let out = unsafe { slice::from_raw_parts_mut(out, count as usize) };
        for (slot, image) in out.iter_mut().zip(self.images.iter()) {
            *slot = &**image as *const Image as *mut Image;
        }
        Ok(())
    }

    /// # Safety
    /// Exclusive chain host access; valid acquire objects.
    pub(in crate::foundation) unsafe fn acquire(
        &self,
        _timeout_ns: u64,
        binary: *mut Binary,
        fence: *mut Fence,
    ) -> Result<(u32, bool), Status> {
        let index = unsafe { self.chain.GetCurrentBackBufferIndex() };
        // OK/SUBOPTIMAL schedules the caller's native signals at the
        // acquisition point on the chain's queue.
        if !binary.is_null() {
            // SAFETY: live acquisition object by caller contract.
            let binary = unsafe { binary.as_ref() }.ok_or(INVALID)?;
            unsafe { binary.schedule(&self.queue) }?;
        }
        if !fence.is_null() {
            // SAFETY: live acquisition object by caller contract.
            let fence = unsafe { fence.as_ref() }.ok_or(INVALID)?;
            unsafe { fence.schedule(&self.queue) }?;
        }
        Ok((index, false))
    }

    /// # Safety
    /// Exclusive chain host access; unique acquired indices.
    pub(in crate::foundation) fn release(&self, indices: &[u32]) -> Result<(), Status> {
        if indices.is_empty() {
            return Err(INVALID);
        }
        let mut seen = [false; 16];
        for &index in indices {
            if index as usize >= self.images.len() || seen[index as usize] {
                return Err(INVALID);
            }
            seen[index as usize] = true;
        }
        Ok(())
    }

    /// The chain's command queue (present wait/signal ordering).
    pub(in crate::foundation) fn queue(&self) -> &d3d::ID3D12CommandQueue {
        &self.queue
    }

    /// Order one binary signal against this chain's queue (present waits).
    /// # Safety
    /// Live fence and exclusive queue access per the contract.
    pub(in crate::foundation) unsafe fn queue_wait(
        &self,
        fence: &d3d::ID3D12Fence,
        value: u64,
    ) -> Result<(), Status> {
        // SAFETY: live queue and fence; the wait orders against prior signals.
        unsafe { self.queue.Wait(fence, value) }.map_err(map_error)
    }

    /// Wrap every chain buffer as a borrowed foundation image.
    fn wrap_buffers(
        d: &Device,
        chain: &dxgi::IDXGISwapChain3,
        format: u32,
        usage: u64,
        extent: Extent,
    ) -> Result<Vec<Box<Image>>, Status> {
        let count = unsafe { chain.GetDesc1() }.map_err(map_error)?.BufferCount;
        let mut images = Vec::new();
        images
            .try_reserve_exact(count as usize)
            .map_err(|_| OUT_OF_MEMORY)?;
        for index in 0..count {
            // SAFETY: live chain; the buffer reference is adopted and borrowed.
            let resource: d3d::ID3D12Resource =
                unsafe { chain.GetBuffer(index) }.map_err(map_error)?;
            let native = unsafe { resource.GetDesc() };
            // SAFETY: the chain owns the buffer for the chain's lifetime.
            let image = unsafe {
                Image::wrap(d, format, usage, extent, native, &resource)
            };
            images.push(image);
        }
        Ok(images)
    }

    /// Borrowed image count of the chain (release/present validation).
    pub(in crate::foundation) fn image_count(&self) -> usize {
        self.images.len()
    }

    /// One native present of the chain; the status maps the contract's
    /// unspecified-native clause to BACKEND_ERROR.
    /// # Safety
    /// Exclusive queue access through the native present call.
    pub(in crate::foundation) unsafe fn present_native(&self) -> Status {
        let flags = if self.sync_interval == 0 {
            dxgi::DXGI_PRESENT_ALLOW_TEARING
        } else {
            dxgi::DXGI_PRESENT(0)
        };
        eprintln!("[trace] Present sync {} flags {}", self.sync_interval, flags.0);
        let outcome = unsafe { self.chain.Present(self.sync_interval, flags) };
        eprintln!("[trace] Present returned {:?}", outcome);
        if outcome.is_ok() {
            OK
        } else {
            BACKEND_ERROR
        }
    }
}

/// A binary semaphore: one pending GPU signal over a native fence value.
/// Signal/wait are queue operations (the acquisition and present paths);
/// the host state tracks idle/signaled exactly as the contract describes.
pub struct Binary {
    fence: d3d::ID3D12Fence,
    value: Cell<u64>,
    pending: Cell<bool>,
}

impl Binary {
    /// Create an idle, unsignaled binary (the contract's acquisition object).
    /// The device outlives the binary and all its uses.
    pub(in crate::foundation) fn create(d: &Device) -> Result<Box<Self>, Status> {
        // SAFETY: well-formed fence description; the output adopts a reference.
        let fence: d3d::ID3D12Fence = unsafe {
            d.device
                .CreateFence(0, d3d::D3D12_FENCE_FLAG_NONE)
                .map_err(map_error)?
        };
        Ok(Box::new(Self {
            fence,
            value: Cell::new(0),
            pending: Cell::new(false),
        }))
    }

    /// The native fence and its pending signal value (queue signal/wait).
    pub(in crate::foundation) fn native(&self) -> (&d3d::ID3D12Fence, u64, bool) {
        (&self.fence, self.value.get(), self.pending.get())
    }

    /// Schedule one native signal at a queue point (acquire/present paths).
    /// # Safety
    /// Exclusive binary host access; the queue is live and externally
    /// synchronized per the contract.
    pub(in crate::foundation) unsafe fn schedule(
        &self,
        queue: &d3d::ID3D12CommandQueue,
    ) -> Result<(), Status> {
        if self.pending.get() {
            // One pending signal at most: the binary is not a counter.
            return Err(INVALID);
        }
        let value = self.value.get() + 1;
        eprintln!("[trace] binary schedule value {value} fence {:p}", &*self.fence);
        self.value.set(value);
        self.pending.set(true);
        // SAFETY: live queue and fence; the signal orders against recorded work.
        let outcome = unsafe { queue.Signal(&self.fence, value) };
        eprintln!("[trace] binary signal returned {:?}", outcome);
        outcome.map_err(map_error)?;
        Ok(())
    }

    /// Whether the scheduled signal has retired on the device.
    pub(in crate::foundation) fn retired(&self) -> bool {
        // SAFETY: live fence; the returned value is an opaque counter.
        let completed = unsafe { self.fence.GetCompletedValue() };
        completed >= self.value.get()
    }

    /// Order this binary's last scheduled value on a queue (submission waits).
    /// # Safety
    /// Live queue, exclusive queue access per the contract.
    pub(in crate::foundation) unsafe fn wait_on(
        &self,
        queue: &d3d::ID3D12CommandQueue,
    ) -> Result<(), Status> {
        // SAFETY: live queue and fence; the wait orders against prior signals.
        unsafe { queue.Wait(&self.fence, self.value.get()) }.map_err(map_error)
    }

    /// Release the binary back to idle after its signal retired.
    pub(in crate::foundation) fn reset(&self) {
        self.pending.set(false);
    }
}

/// A CPU-visible fence: signaled at native completion points, pollable and
/// waitable with a bounded timeout, resettable to unsignaled.
pub struct Fence {
    device: *const Device,
    fence: d3d::ID3D12Fence,
    event: windows::Win32::Foundation::HANDLE,
    value: Cell<u64>,
    signaled: Cell<bool>,
}

impl Fence {
    /// The device outlives the fence and all its uses.
    pub(in crate::foundation) fn create(
        d: &Device,
        signaled: bool,
    ) -> Result<Box<Self>, Status> {
        let value = u64::from(signaled);
        // SAFETY: well-formed fence description; the output adopts a reference.
        let fence: d3d::ID3D12Fence = unsafe {
            d.device
                .CreateFence(value, d3d::D3D12_FENCE_FLAG_NONE)
                .map_err(map_error)?
        };
        // SAFETY: manual-reset event for bounded fence waits.
        let event = unsafe {
            windows::Win32::System::Threading::CreateEventW(
                None,
                false,
                false,
                windows::core::w!("ogpu fence"),
            )
        }
        .map_err(map_error)?;
        Ok(Box::new(Self {
            device: d,
            fence,
            event,
            value: Cell::new(value),
            signaled: Cell::new(signaled),
        }))
    }

    /// Schedule one native signal at a queue point (present retirement).
    /// # Safety
    /// Exclusive fence host access; the queue is live and externally
    /// synchronized per the contract.
    pub(in crate::foundation) unsafe fn schedule(
        &self,
        queue: &d3d::ID3D12CommandQueue,
    ) -> Result<(), Status> {
        let value = self.value.get() + 1;
        self.value.set(value);
        self.signaled.set(false);
        // SAFETY: live queue and fence; the signal orders against recorded work.
        let outcome = unsafe { queue.Signal(&self.fence, value) };
        eprintln!("[trace] fence signal returned {:?}", outcome);
        outcome.map_err(map_error)?;
        Ok(())
    }

    /// Poll the native completion counter (no wait).
    pub(in crate::foundation) fn poll(&self) -> Result<bool, Status> {
        // SAFETY: live fence; the returned value is an opaque counter.
        let completed = unsafe { self.fence.GetCompletedValue() };
        Ok(completed >= self.value.get())
    }

    /// Bounded native wait; `UINT64_MAX` waits without a bound. Exclusive
    /// fence host access through the wait is the caller's.
    pub(in crate::foundation) fn wait(&self, timeout_ns: u64) -> Result<(), Status> {
        if self.poll()? {
            self.signaled.set(true);
            return Ok(());
        }
        // SAFETY: live fence and event handle.
        unsafe {
            self.fence
                .SetEventOnCompletion(self.value.get(), self.event)
        }
        .map_err(map_error)?;
        let timeout = if timeout_ns == u64::MAX {
            u32::MAX
        } else {
            (timeout_ns / 1_000_000) as u32
        };
        // SAFETY: live event handle from creation.
        let outcome =
            unsafe { windows::Win32::System::Threading::WaitForSingleObject(self.event, timeout) };
        match outcome {
            windows::Win32::Foundation::WAIT_OBJECT_0 => {
                self.signaled.set(true);
                Ok(())
            }
            windows::Win32::Foundation::WAIT_TIMEOUT => Err(TIMEOUT),
            _ => Err(BACKEND_ERROR),
        }
    }

    /// Return the fence to the unsignaled state for its next cycle.
    pub(in crate::foundation) fn reset(&self) -> Result<(), Status> {
        self.signaled.set(false);
        self.value.set(self.value.get() + 1);
        Ok(())
    }
}

impl Drop for Binary {
    fn drop(&mut self) {
        // COM references drop with the field; the caller retired all uses.
    }
}

impl Drop for Fence {
    fn drop(&mut self) {
        // SAFETY: the event handle is owned by this fence.
        unsafe {
            windows::Win32::Foundation::CloseHandle(self.event);
        }
    }
}
