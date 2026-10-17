//! Explicit WSI ownership and execution. No frame scheduler or pending-state mirror.
use super::*;
use crate::foundation::Surface;
use commands::{scratch, slice};
use std::alloc::Layout;

macro_rules! functions {
    ($($name:ident: $ty:ident),* $(,)?) => {
        #[allow(non_snake_case)]
        struct Functions { $($name: vk::$ty,)* }
        impl Functions {
            fn load(instance: &Instance) -> Result<Self, Status> {
                Ok(Self { $( $name: {
                    let f: vk::$ty = unsafe { std::mem::transmute(instance.proc(
                        std::ffi::CStr::from_bytes_with_nul(concat!(stringify!($name), "\0").as_bytes()).unwrap())) };
                    Some(f.ok_or(UNSUPPORTED)?)
                }, )* })
            }
        }
    };
}
functions! {
    vkCreateSwapchainKHR: PFN_vkCreateSwapchainKHR,
    vkDestroySwapchainKHR: PFN_vkDestroySwapchainKHR,
    vkGetSwapchainImagesKHR: PFN_vkGetSwapchainImagesKHR,
    vkAcquireNextImageKHR: PFN_vkAcquireNextImageKHR,
    vkReleaseSwapchainImagesKHR: PFN_vkReleaseSwapchainImagesKHR,
    vkQueuePresentKHR: PFN_vkQueuePresentKHR,
}

pub struct Swapchain {
    device: *const Device,
    surface: *const Surface,
    handle: vk::VkSwapchainKHR,
    f: Functions,
    // Frozen after construction, so borrowed Image addresses never move.
    images: Vec<Image>,
    present_domains: Vec<u32>,
}
impl Drop for Swapchain {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            let d = unsafe { &*self.device };
            unsafe {
                (self.f.vkDestroySwapchainKHR.unwrap())(d.handle, self.handle, ptr::null());
            }
        }
        // Images are borrows: dropping their metadata never destroys a VkImage.
    }
}

// Cold setup helper: existing surface queries retain their validation/retry rules.
unsafe fn query<T: Copy + Default>(
    surface: &Surface,
    adapter: &Adapter,
    mode: Option<u32>,
    kind: u32,
) -> Result<Vec<T>, Status> {
    let mut q = Query {
        header: Record::new::<Query>(kind),
        data: ptr::null_mut(),
        capacity: 0,
        count: 0,
        element_size: size_of::<T>() as u32,
        reserved: 0,
    };
    unsafe {
        surface.query(adapter, mode, &mut q)?;
    }
    for _ in 0..8 {
        let mut values = Vec::new();
        values
            .try_reserve_exact(q.count as usize)
            .map_err(|_| OUT_OF_MEMORY)?;
        values.resize(q.count as usize, T::default());
        q.capacity = q.count;
        q.data = values.as_mut_ptr().cast();
        match unsafe { surface.query(adapter, mode, &mut q) } {
            Ok(()) => {
                values.truncate(q.count as usize);
                return Ok(values);
            }
            Err(CAPACITY) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(CAPACITY)
}
fn check_caps(desc: &SwapchainDesc, caps: &SurfaceCapabilities) -> Result<(), Status> {
    if desc.min_image_count < caps.min_image_count
        || (caps.max_image_count != 0 && desc.min_image_count > caps.max_image_count)
        || desc.layer_count == 0
        || desc.layer_count > caps.max_layers
        || desc.usage == 0
        || desc.usage & !caps.image_usage != 0
        || !desc.transform.is_power_of_two()
        || desc.transform & caps.supported_transforms == 0
        || !desc.alpha.is_power_of_two()
        || desc.alpha & caps.supported_alpha == 0
        || desc.clipped > 1
        || (0..2).any(|i| {
            desc.extent[i] == 0
                || desc.extent[i] < caps.min_extent[i]
                || desc.extent[i] > caps.max_extent[i]
                || (caps.current_extent[i] != u32::MAX && desc.extent[i] != caps.current_extent[i])
        })
    {
        return Err(INVALID);
    }
    Ok(())
}
impl Swapchain {
    pub(in crate::foundation) unsafe fn create(
        d: &Device,
        desc: &SwapchainDesc,
    ) -> Result<Box<Self>, Status> {
        d.ready()?;
        desc.header.validate::<SwapchainDesc>(SWAPCHAIN_DESC)?;
        if d.snapshot.features.enabled & PRESENTATION == 0 {
            return Err(UNSUPPORTED);
        }
        let surface = unsafe { desc.surface.as_ref() }.ok_or(INVALID)?;
        let native_surface = surface.native_handle(&d._instance)?;
        // Shared-present, protected, mutable-format, scaling and latest-ready are
        // separate profiles, not silently enabled by this baseline.
        if desc.present_mode > 3 {
            return Err(UNSUPPORTED);
        }
        let old = unsafe { desc.old_swapchain.as_ref() };
        if old.is_some_and(|old| {
            old.device != ptr::from_ref(d) || old.surface != ptr::from_ref(surface)
        }) {
            return Err(INVALID);
        }
        let adapter = Adapter {
            state: AdapterState {
                instance: d._instance.clone(),
                physical: d.physical,
            },
            snapshot: d.snapshot.clone(),
        };
        let formats = unsafe { query::<SurfaceFormat>(surface, &adapter, None, SURFACE_FORMATS)? };
        if desc.format == 0
            || !formats
                .iter()
                .any(|f| f.format == desc.format && f.color_space == desc.color_space)
        {
            return Err(UNSUPPORTED);
        }
        let modes = unsafe { slice(desc.modes, desc.mode_count)? };
        if !modes.is_empty() {
            if !modes.contains(&desc.present_mode) {
                return Err(INVALID);
            }
            let compatible = unsafe {
                query::<u32>(
                    surface,
                    &adapter,
                    Some(desc.present_mode),
                    SURFACE_COMPATIBLE_MODES,
                )?
            };
            for (i, mode) in modes.iter().enumerate() {
                if *mode > 3 || !compatible.contains(mode) {
                    return Err(UNSUPPORTED);
                }
                if modes[..i].contains(mode) {
                    return Err(INVALID);
                }
            }
        }
        if modes.is_empty() {
            let available =
                unsafe { query::<u32>(surface, &adapter, None, SURFACE_PRESENT_MODES)? };
            if !available.contains(&desc.present_mode) {
                return Err(UNSUPPORTED);
            }
            check_caps(
                desc,
                &unsafe {
                    query::<SurfaceCapabilities>(surface, &adapter, None, SURFACE_CAPABILITIES)?
                }[0],
            )?;
        } else {
            for mode in modes {
                check_caps(
                    desc,
                    &unsafe {
                        query::<SurfaceCapabilities>(
                            surface,
                            &adapter,
                            Some(*mode),
                            SURFACE_CAPABILITIES,
                        )?
                    }[0],
                )?;
            }
        }
        let queues = unsafe { query::<SurfaceQueue>(surface, &adapter, None, SURFACE_QUEUES)? };
        let present_domains: Vec<_> = queues
            .iter()
            .filter(|q| q.supported != 0)
            .map(|q| q.domain)
            .collect();
        let image_desc = ImageDesc {
            header: Record::new::<ImageDesc>(IMAGE_DESC),
            dimension: 2,
            format: desc.format,
            extent: Extent {
                x: desc.extent[0],
                y: desc.extent[1],
                z: 1,
            },
            mip_count: 1,
            layer_count: desc.layer_count,
            sample_count: 1,
            usage: desc.usage,
            flags: 0,
            view_format_count: 0,
            view_formats: ptr::null(),
            concurrent_domain_count: desc.domain_count,
            concurrent_domains: desc.domains,
        };
        let prepared = unsafe { d.prepare_image(&image_desc)? };
        let mode_info = vk::VkSwapchainPresentModesCreateInfoKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SWAPCHAIN_PRESENT_MODES_CREATE_INFO_KHR,
            presentModeCount: desc.mode_count,
            pPresentModes: desc.modes,
            ..Default::default()
        };
        let info = vk::VkSwapchainCreateInfoKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SWAPCHAIN_CREATE_INFO_KHR,
            pNext: if modes.is_empty() {
                ptr::null()
            } else {
                ptr::from_ref(&mode_info).cast()
            },
            surface: native_surface,
            minImageCount: desc.min_image_count,
            imageFormat: images::format(desc.format)?.native,
            imageColorSpace: desc.color_space,
            imageExtent: vk::VkExtent2D {
                width: desc.extent[0],
                height: desc.extent[1],
            },
            imageArrayLayers: desc.layer_count,
            imageUsage: desc.usage as u32,
            imageSharingMode: if desc.domain_count == 0 {
                vk::VkSharingMode_VK_SHARING_MODE_EXCLUSIVE
            } else {
                vk::VkSharingMode_VK_SHARING_MODE_CONCURRENT
            },
            queueFamilyIndexCount: desc.domain_count,
            pQueueFamilyIndices: desc.domains,
            preTransform: desc.transform,
            compositeAlpha: desc.alpha,
            presentMode: desc.present_mode,
            clipped: desc.clipped,
            oldSwapchain: old.map_or(ptr::null_mut(), |s| s.handle),
            ..Default::default()
        };
        let mut chain = Box::new(Self {
            device: d,
            surface,
            handle: ptr::null_mut(),
            f: Functions::load(&d._instance)?,
            images: Vec::new(),
            present_domains,
        });
        let mut handle = ptr::null_mut();
        // After this native call, oldSwapchain is retired even on failure. No rollback.
        d.result(unsafe {
            (chain.f.vkCreateSwapchainKHR.unwrap())(d.handle, &info, ptr::null(), &mut handle)
        })?;
        chain.handle = handle;
        let handles =
            super::super::presentation::enumerate(ptr::null_mut(), |count, data| unsafe {
                (chain.f.vkGetSwapchainImagesKHR.unwrap())(d.handle, handle, count, data)
            })?;
        if handles.is_empty() {
            return Err(BACKEND_ERROR);
        }
        chain
            .images
            .try_reserve_exact(handles.len())
            .map_err(|_| OUT_OF_MEMORY)?;
        for handle in handles {
            chain
                .images
                .push(Image::from_swapchain(d, handle, prepared.clone()));
        }
        Ok(chain)
    }
    pub(in crate::foundation) unsafe fn images(
        &self,
        capacity: u32,
        out: *mut *mut Image,
        count: *mut u32,
    ) -> Result<(), Status> {
        if count.is_null() || (capacity != 0 && out.is_null()) {
            return Err(INVALID);
        }
        unsafe {
            count.write(self.images.len() as u32);
        }
        if capacity == 0 {
            return Ok(());
        }
        if (capacity as usize) < self.images.len() {
            return Err(CAPACITY);
        }
        for (i, image) in self.images.iter().enumerate() {
            unsafe {
                out.add(i).write(ptr::from_ref(image).cast_mut());
            }
        }
        Ok(())
    }
    pub(in crate::foundation) unsafe fn acquire(
        &self,
        timeout: u64,
        binary: *mut Binary,
        fence: *mut Fence,
    ) -> Result<(u32, bool), Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        if binary.is_null() && fence.is_null() {
            return Err(INVALID);
        }
        let binary = unsafe { binary.as_ref() };
        let fence = unsafe { fence.as_ref() };
        if binary.is_some_and(|b| b.device != self.device)
            || fence.is_some_and(|f| f.device != self.device)
        {
            return Err(INVALID);
        }
        let mut index = u32::MAX;
        let result = d.result(unsafe {
            (self.f.vkAcquireNextImageKHR.unwrap())(
                d.handle,
                self.handle,
                timeout,
                binary.map_or(ptr::null_mut(), |b| b.handle),
                fence.map_or(ptr::null_mut(), |f| f.handle),
                &mut index,
            )
        });
        match result {
            Ok(()) => Ok((index, false)),
            Err(SUBOPTIMAL) => Ok((index, true)),
            Err(e) => Err(e),
        }
    }
    pub(in crate::foundation) fn release(&self, indices: &[u32]) -> Result<(), Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        if indices.is_empty()
            || indices.len() > u32::MAX as usize
            || indices.iter().any(|i| *i as usize >= self.images.len())
        {
            return Err(INVALID);
        }
        let info = vk::VkReleaseSwapchainImagesInfoKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_RELEASE_SWAPCHAIN_IMAGES_INFO_KHR,
            swapchain: self.handle,
            imageIndexCount: indices.len() as u32,
            pImageIndices: indices.as_ptr(),
            ..Default::default()
        };
        d.result(unsafe { (self.f.vkReleaseSwapchainImagesKHR.unwrap())(d.handle, &info) })
    }
}

fn present_layout(waits: u32, images: u32) -> Result<(Layout, [usize; 4]), Status> {
    let layout = Layout::array::<vk::VkSemaphore>(waits as usize).map_err(|_| INVALID)?;
    let (layout, chains) = layout
        .extend(Layout::array::<vk::VkSwapchainKHR>(images as usize).map_err(|_| INVALID)?)
        .map_err(|_| INVALID)?;
    let (layout, indices) = layout
        .extend(Layout::array::<u32>(images as usize).map_err(|_| INVALID)?)
        .map_err(|_| INVALID)?;
    let (layout, fences) = layout
        .extend(Layout::array::<vk::VkFence>(images as usize).map_err(|_| INVALID)?)
        .map_err(|_| INVALID)?;
    let (layout, results) = layout
        .extend(Layout::array::<vk::VkResult>(images as usize).map_err(|_| INVALID)?)
        .map_err(|_| INVALID)?;
    Ok((layout.pad_to_align(), [chains, indices, fences, results]))
}
pub(in crate::foundation) fn present_scratch(
    waits: u32,
    images: u32,
) -> Result<HostRequirements, Status> {
    let (layout, _) = present_layout(waits, images)?;
    Ok(HostRequirements {
        size: layout.size() as u64,
        alignment: layout.align() as u64,
    })
}
impl Queue {
    pub(in crate::foundation) unsafe fn present(&self, desc: &PresentDesc) -> Result<bool, Status> {
        desc.header.validate::<PresentDesc>(PRESENT_DESC)?;
        let d = unsafe { self.device.load(Ordering::Relaxed).as_ref() }.ok_or(INVALID)?;
        d.ready()?;
        if d.snapshot.features.enabled & PRESENTATION == 0 {
            return Err(UNSUPPORTED);
        }
        if desc.image_count == 0 {
            return Err(INVALID);
        }
        let waits = unsafe { slice(desc.waits, desc.wait_count)? };
        let entries = unsafe { slice(desc.images, desc.image_count)? };
        let storage = scratch(
            desc.scratch.data,
            desc.scratch.size,
            present_scratch(desc.wait_count, desc.image_count)?,
        )?;
        let (_, offsets) = present_layout(desc.wait_count, desc.image_count)?;
        let native_waits = storage.cast::<vk::VkSemaphore>();
        let chains = storage
            .wrapping_add(offsets[0])
            .cast::<vk::VkSwapchainKHR>();
        let indices = storage.wrapping_add(offsets[1]).cast::<u32>();
        let fences = storage.wrapping_add(offsets[2]).cast::<vk::VkFence>();
        let results = storage.wrapping_add(offsets[3]).cast::<vk::VkResult>();
        for (i, wait) in waits.iter().enumerate() {
            let b = unsafe { wait.as_ref() }.ok_or(INVALID)?;
            if b.device != ptr::from_ref(d) {
                return Err(INVALID);
            }
            unsafe {
                native_waits.add(i).write(b.handle);
            }
        }
        for (i, entry) in entries.iter().enumerate() {
            let s = unsafe { entry.swapchain.as_ref() }.ok_or(INVALID)?;
            if s.device != ptr::from_ref(d)
                || entry.index as usize >= s.images.len()
                || entry.reserved != 0
            {
                return Err(INVALID);
            }
            if !s.present_domains.contains(&self.domain) {
                return Err(UNSUPPORTED);
            }
            s.images[entry.index as usize].command_handle(ptr::from_ref(d), self.domain, 0)?;
            let fence = unsafe { entry.fence.as_ref() };
            if fence.is_some_and(|f| f.device != ptr::from_ref(d)) {
                return Err(INVALID);
            }
            unsafe {
                chains.add(i).write(s.handle);
                indices.add(i).write(entry.index);
                fences
                    .add(i)
                    .write(fence.map_or(ptr::null_mut(), |f| f.handle));
                results.add(i).write(vk::VkResult_VK_ERROR_UNKNOWN);
            }
        }
        let fence_info = vk::VkSwapchainPresentFenceInfoKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SWAPCHAIN_PRESENT_FENCE_INFO_KHR,
            swapchainCount: desc.image_count,
            pFences: fences,
            ..Default::default()
        };
        let info = vk::VkPresentInfoKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PRESENT_INFO_KHR,
            pNext: ptr::from_ref(&fence_info).cast(),
            waitSemaphoreCount: desc.wait_count,
            pWaitSemaphores: native_waits,
            swapchainCount: desc.image_count,
            pSwapchains: chains,
            pImageIndices: indices,
            pResults: results,
        };
        let first = unsafe { &*entries[0].swapchain };
        let result = d.result(unsafe { (first.f.vkQueuePresentKHR.unwrap())(self.handle, &info) });
        if !desc.results.is_null() {
            for i in 0..entries.len() {
                unsafe {
                    desc.results
                        .add(i)
                        .write(d.result(results.add(i).read()).err().unwrap_or(OK));
                }
            }
        }
        match result {
            Ok(()) => Ok(false),
            Err(SUBOPTIMAL) => Ok(true),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::{ogpu_next_presentation_api, rust};
    use std::cell::Cell;

    #[test]
    fn scratch_and_surface_bounds_are_explicit() {
        let (layout, offsets) = present_layout(3, 2).unwrap();
        assert_eq!(offsets[0], 3 * size_of::<vk::VkSemaphore>());
        assert!(offsets.windows(2).all(|v| v[0] < v[1]));
        assert_eq!(offsets[2] % align_of::<vk::VkFence>(), 0);
        assert!(layout.size() >= offsets[3] + 2 * size_of::<vk::VkResult>());
        let caps = SurfaceCapabilities {
            min_image_count: 3,
            max_image_count: 5,
            current_extent: [u32::MAX; 2],
            min_extent: [1; 2],
            max_extent: [100; 2],
            max_layers: 1,
            supported_transforms: 1,
            supported_alpha: 1,
            image_usage: 18,
            ..Default::default()
        };
        let mut desc = SwapchainDesc {
            header: Record::new::<SwapchainDesc>(SWAPCHAIN_DESC),
            surface: ptr::null_mut(),
            old_swapchain: ptr::null_mut(),
            format: 3,
            color_space: 0,
            extent: [80; 2],
            layer_count: 1,
            min_image_count: 3,
            usage: 18,
            transform: 1,
            alpha: 1,
            present_mode: 2,
            clipped: 0,
            domain_count: 0,
            mode_count: 0,
            domains: ptr::null(),
            modes: ptr::null(),
        };
        assert_eq!(check_caps(&desc, &caps), Ok(()));
        desc.min_image_count = 2;
        assert_eq!(check_caps(&desc, &caps), Err(INVALID));
        desc.min_image_count = 3;
        desc.extent[0] = 0;
        assert_eq!(check_caps(&desc, &caps), Err(INVALID));
        desc.extent[0] = 101;
        assert_eq!(check_caps(&desc, &caps), Err(INVALID));
        desc.extent[0] = 80;
        desc.usage = 32;
        assert_eq!(check_caps(&desc, &caps), Err(INVALID));
        desc.usage = 18;
        desc.alpha = 3;
        assert_eq!(check_caps(&desc, &caps), Err(INVALID));
        assert_eq!(status(vk::VkResult_VK_SUBOPTIMAL_KHR), Err(SUBOPTIMAL));
        assert_eq!(
            status(vk::VkResult_VK_ERROR_OUT_OF_DATE_KHR),
            Err(OUT_OF_DATE)
        );
        assert_eq!(
            status(vk::VkResult_VK_ERROR_SURFACE_LOST_KHR),
            Err(SURFACE_LOST)
        );
    }
    thread_local! {
        static RESULT: Cell<vk::VkResult> = const { Cell::new(vk::VkResult_VK_SUCCESS) };
        static CALLS: Cell<u32> = const { Cell::new(0) };
        static SHAPE: Cell<bool> = const { Cell::new(false) };
    }
    unsafe extern "C" fn destroy(
        _: vk::VkDevice,
        _: vk::VkSwapchainKHR,
        _: *const vk::VkAllocationCallbacks,
    ) {
    }
    unsafe extern "C" fn acquire(
        _: vk::VkDevice,
        _: vk::VkSwapchainKHR,
        _: u64,
        _: vk::VkSemaphore,
        _: vk::VkFence,
        index: *mut u32,
    ) -> vk::VkResult {
        CALLS.set(CALLS.get() + 1);
        // Deliberately scribble an unspecified output even on failure.
        unsafe {
            index.write(0);
        }
        RESULT.get()
    }
    unsafe extern "C" fn release(
        _: vk::VkDevice,
        info: *const vk::VkReleaseSwapchainImagesInfoKHR,
    ) -> vk::VkResult {
        CALLS.set(CALLS.get() + 1);
        SHAPE.set(unsafe { (*info).imageIndexCount == 1 && *(*info).pImageIndices == 0 });
        RESULT.get()
    }
    unsafe extern "C" fn present(
        _: vk::VkQueue,
        info: *const vk::VkPresentInfoKHR,
    ) -> vk::VkResult {
        CALLS.set(CALLS.get() + 1);
        let info = unsafe { &*info };
        let fences = unsafe { &*info.pNext.cast::<vk::VkSwapchainPresentFenceInfoKHR>() };
        SHAPE.set(
            info.swapchainCount == 2
                && info.waitSemaphoreCount == 1
                && fences.swapchainCount == 2
                && unsafe {
                    *info.pImageIndices == 0
                        && *info.pImageIndices.add(1) == 0
                        && !(*info.pWaitSemaphores).is_null()
                        && !(*fences.pFences).is_null()
                        && (*fences.pFences.add(1)).is_null()
                },
        );
        unsafe {
            info.pResults.write(vk::VkResult_VK_SUCCESS);
            info.pResults.add(1).write(RESULT.get());
        }
        RESULT.get()
    }
    #[test]
    #[ignore = "requires modern Vulkan; headless injected WSI output, rejection and one-call batching checks"]
    fn gpu_injected_swapchain_contract() {
        let discovery = rust::discover().unwrap();
        let adapter = rust::adapters(&discovery)
            .iter()
            .find(|a| rust::adapter_features(a).baseline_supported != 0)
            .unwrap();
        let domain = rust::adapter_queues(adapter)
            .iter()
            .find(|q| q.flags & GRAPHICS != 0)
            .unwrap()
            .domain;
        let mut d = Device::create(
            adapter,
            &[QueueRequest {
                domain,
                count: 1,
                priority: 1.0,
            }],
            0,
        )
        .unwrap();
        // Only the wrapper gate is enabled: every WSI function is injected, no
        // headless device is passed to native WSI or advertised as presentation-capable.
        d.snapshot.features.enabled |= PRESENTATION;
        let other = Device::create(
            adapter,
            &[QueueRequest {
                domain,
                count: 1,
                priority: 1.0,
            }],
            0,
        )
        .unwrap();
        let binary = Binary::create(&d).unwrap();
        let foreign = Binary::create(&other).unwrap();
        let fence = Fence::create(&d, false).unwrap();
        let image_desc = ImageDesc {
            header: Record::new::<ImageDesc>(IMAGE_DESC),
            format: 3,
            dimension: 2,
            mip_count: 1,
            layer_count: 1,
            sample_count: 1,
            extent: Extent { x: 8, y: 8, z: 1 },
            usage: 18,
            flags: 0,
            view_formats: ptr::null(),
            view_format_count: 0,
            concurrent_domain_count: 0,
            concurrent_domains: ptr::null(),
        };
        let chain = || Swapchain {
            device: &*d,
            surface: ptr::null(),
            handle: ptr::dangling_mut(),
            f: Functions {
                vkCreateSwapchainKHR: None,
                vkDestroySwapchainKHR: Some(destroy),
                vkGetSwapchainImagesKHR: None,
                vkAcquireNextImageKHR: Some(acquire),
                vkReleaseSwapchainImagesKHR: Some(release),
                vkQueuePresentKHR: Some(present),
            },
            images: vec![Image::from_swapchain(&d, ptr::dangling_mut(), unsafe {
                d.prepare_image(&image_desc).unwrap()
            })],
            present_domains: vec![domain],
        };
        let mut a = chain();
        let mut b = chain();
        let mut api = std::mem::MaybeUninit::<PresentationApi>::uninit();
        unsafe {
            assert_eq!(
                ogpu_next_presentation_api(
                    VERSION,
                    size_of::<PresentationApi>() as u32,
                    api.as_mut_ptr()
                ),
                OK
            );
            let api = api.assume_init();
            let binary = ptr::from_ref(&*binary).cast_mut();
            let foreign = ptr::from_ref(&*foreign).cast_mut();
            let fence = ptr::from_ref(&*fence).cast_mut();
            let mut index = 99;
            CALLS.set(0);
            assert_eq!(
                api.acquire.unwrap()(&mut a, 0, foreign, fence, &mut index),
                INVALID
            );
            assert_eq!(
                api.acquire.unwrap()(&mut a, 0, ptr::null_mut(), ptr::null_mut(), &mut index),
                INVALID
            );
            assert_eq!(index, 99);
            assert_eq!(CALLS.get(), 0);
            for (native, mapped) in [
                (vk::VkResult_VK_TIMEOUT, TIMEOUT),
                (vk::VkResult_VK_NOT_READY, NOT_READY),
                (vk::VkResult_VK_ERROR_OUT_OF_DATE_KHR, OUT_OF_DATE),
                (vk::VkResult_VK_ERROR_SURFACE_LOST_KHR, SURFACE_LOST),
                (vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY, OUT_OF_MEMORY),
            ] {
                RESULT.set(native);
                assert_eq!(
                    api.acquire.unwrap()(&mut a, 0, binary, fence, &mut index),
                    mapped
                );
                assert_eq!(index, 99);
            }
            RESULT.set(vk::VkResult_VK_SUBOPTIMAL_KHR);
            assert_eq!(
                api.acquire.unwrap()(&mut a, 0, binary, fence, &mut index),
                SUBOPTIMAL
            );
            assert_eq!(index, 0);
            RESULT.set(vk::VkResult_VK_SUCCESS);
            assert_eq!(
                api.acquire.unwrap()(&mut a, 0, binary, fence, &mut index),
                OK
            );
            assert!(a.images[0].is_borrowed());
            assert_eq!(
                a.images[0].layout(STATE_PRESENT),
                Ok(vk::VkImageLayout_VK_IMAGE_LAYOUT_PRESENT_SRC_KHR)
            );
            let ordinary = Image::create(&d, &image_desc).unwrap();
            assert_eq!(ordinary.layout(STATE_PRESENT), Err(INVALID));
            CALLS.set(0);
            assert_eq!(api.release.unwrap()(&mut a, 1, &2), INVALID);
            assert_eq!(CALLS.get(), 0);
            assert_eq!(api.release.unwrap()(&mut a, 1, &0), OK);
            assert!(SHAPE.get());
            let waits = [binary];
            let mut entries = [
                PresentImage {
                    swapchain: &mut a,
                    index: 0,
                    reserved: 0,
                    fence,
                },
                PresentImage {
                    swapchain: &mut b,
                    index: 0,
                    reserved: 1,
                    fence: ptr::null_mut(),
                },
            ];
            let req = present_scratch(1, 2).unwrap();
            let mut storage = vec![0u64; req.size.div_ceil(8) as usize];
            let mut results = [99; 2];
            let mut desc = PresentDesc {
                header: Record::new::<PresentDesc>(PRESENT_DESC),
                wait_count: 1,
                image_count: 2,
                waits: waits.as_ptr(),
                images: entries.as_ptr(),
                results: results.as_mut_ptr(),
                scratch: HostSpan {
                    data: storage.as_mut_ptr().cast(),
                    size: req.size,
                },
            };
            let queue = ptr::from_ref(d.queue(domain, 0).unwrap()).cast_mut();
            CALLS.set(0);
            assert_eq!(api.queue_present.unwrap()(queue, &desc), INVALID);
            assert_eq!(results, [99; 2]);
            assert_eq!(CALLS.get(), 0);
            entries[1].reserved = 0;
            desc.images = entries.as_ptr();
            desc.scratch.size = 0;
            assert_eq!(api.queue_present.unwrap()(queue, &desc), CAPACITY);
            assert_eq!(results, [99; 2]);
            assert_eq!(CALLS.get(), 0);
            desc.scratch.size = req.size;
            for (native, mapped) in [
                (vk::VkResult_VK_SUCCESS, OK),
                (vk::VkResult_VK_SUBOPTIMAL_KHR, SUBOPTIMAL),
                (vk::VkResult_VK_ERROR_OUT_OF_DATE_KHR, OUT_OF_DATE),
                (vk::VkResult_VK_ERROR_SURFACE_LOST_KHR, SURFACE_LOST),
                (vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY, OUT_OF_MEMORY),
            ] {
                CALLS.set(0);
                RESULT.set(native);
                assert_eq!(api.queue_present.unwrap()(queue, &desc), mapped);
                assert_eq!(CALLS.get(), 1);
                assert!(SHAPE.get());
                assert_eq!(results, [OK, mapped]);
            }
            // Terminal native failure still marks the underlying device lost.
            RESULT.set(vk::VkResult_VK_ERROR_DEVICE_LOST);
            assert_eq!(api.queue_present.unwrap()(queue, &desc), DEVICE_LOST);
            CALLS.set(0);
            assert_eq!(api.queue_present.unwrap()(queue, &desc), DEVICE_LOST);
            assert_eq!(CALLS.get(), 0);
        }
    }
}
