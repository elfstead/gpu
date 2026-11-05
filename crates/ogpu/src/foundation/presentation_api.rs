//! Thin optional C presentation table, backed by the native Rust operations.
use super::*;

/// Get the exact revision's table, without loading Vulkan or accessing a display.
/// # Safety
/// Out points to writable, aligned, disjoint storage of byte_size bytes. Failure
/// leaves storage unchanged. See include/ogpu_next.h for extension contracts.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_presentation_api(
    version: u32,
    byte_size: u32,
    out: *mut PresentationApi,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    if version != VERSION {
        return VERSION_MISMATCH;
    }
    if byte_size as usize != size_of::<PresentationApi>() {
        return INVALID;
    }
    unsafe {
        out.write(PresentationApi {
            discovery_create: Some(discovery_create),
            adapter_support: Some(adapter_support),
            surface_wayland_create: Some(surface_wayland_create),
            surface_destroy: Some(surface_destroy),
            surface_query: Some(surface_query),
            surface_mode_query: Some(surface_mode_query),
            binary_create: Some(binary_create),
            binary_destroy: Some(binary_destroy),
            fence_create: Some(fence_create),
            fence_destroy: Some(fence_destroy),
            fence_poll: Some(fence_poll),
            fence_wait: Some(fence_wait),
            fence_reset: Some(fence_reset),
            surface_win32_create: Some(surface_win32_create),
            presentation_contract_query: Some(presentation_contract_query),
            chain_create: Some(chain_create),
            chain_destroy: Some(chain_destroy),
            chain_info_query: Some(chain_info_query),
            chain_images: Some(chain_images),
            chain_resize: Some(chain_resize),
            chain_pacing_handle: Some(chain_pacing_handle),
            acquire: Some(acquire),
            abandon_scratch_requirements: Some(abandon_scratch_requirements),
            abandon: Some(abandon),
            present_scratch_requirements: Some(present_scratch_requirements),
            queue_present: Some(queue_present),
        });
    }
    OK
}
unsafe extern "C" fn adapter_support(
    adapter: *const Adapter,
    out: *mut PresentationSupport,
) -> Status {
    boundary(|| {
        let adapter = unsafe { adapter.as_ref() }.ok_or(INVALID)?;
        if out.is_null() {
            return Err(INVALID);
        }
        let value = rust::presentation_support(adapter)?;
        unsafe {
            out.write(value);
        }
        Ok(())
    })
}
unsafe extern "C" fn discovery_create(platforms: u32, out: *mut *mut Discovery) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let discovery = rust::discover_presentation(platforms)?;
        unsafe {
            out.write(Box::into_raw(discovery));
        }
        Ok(())
    })
}
unsafe extern "C" fn surface_wayland_create(
    adapter: *const Adapter,
    desc: *const WaylandSurfaceDesc,
    out: *mut *mut Surface,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let adapter = unsafe { adapter.as_ref() }.ok_or(INVALID)?;
        if desc.is_null() {
            return Err(INVALID);
        }
        unsafe { desc.cast::<Record>().read() }
            .validate::<WaylandSurfaceDesc>(WAYLAND_SURFACE_DESC)?;
        let surface = unsafe { rust::surface_wayland_create(adapter, &*desc)? };
        unsafe {
            out.write(surface.as_raw());
        }
        Ok(())
    })
}
unsafe extern "C" fn surface_destroy(surface: *mut Surface) {
    if let Some(surface) = unsafe { rust::SurfaceHandle::from_raw(surface) } {
        unsafe {
            rust::surface_destroy(surface);
        }
    }
}
unsafe extern "C" fn surface_query(
    adapter: *const Adapter,
    surface: *const Surface,
    query: *mut Query,
) -> Status {
    boundary(|| unsafe {
        rust::surface_query(
            adapter.as_ref().ok_or(INVALID)?,
            rust::SurfaceHandle::from_raw(surface.cast_mut()).ok_or(INVALID)?,
            query,
        )
    })
}

unsafe extern "C" fn surface_mode_query(
    adapter: *const Adapter,
    surface: *const Surface,
    mode: u32,
    query: *mut Query,
) -> Status {
    boundary(|| unsafe {
        rust::surface_mode_query(
            adapter.as_ref().ok_or(INVALID)?,
            rust::SurfaceHandle::from_raw(surface.cast_mut()).ok_or(INVALID)?,
            mode,
            query,
        )
    })
}

unsafe extern "C" fn binary_create(device: *mut Device, out: *mut *mut Binary) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let d = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let binary = unsafe { rust::binary_create(d)? };
        unsafe {
            out.write(binary.as_raw());
        }
        Ok(())
    })
}
unsafe extern "C" fn binary_destroy(binary: *mut Binary) {
    if let Some(binary) = unsafe { rust::BinaryHandle::from_raw(binary) } {
        unsafe {
            rust::binary_destroy(binary);
        }
    }
}
unsafe extern "C" fn fence_create(
    device: *mut Device,
    signaled: u32,
    out: *mut *mut Fence,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        if signaled > 1 {
            return Err(INVALID);
        }
        let d = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let fence = unsafe { rust::fence_create(d, signaled != 0)? };
        unsafe {
            out.write(fence.as_raw());
        }
        Ok(())
    })
}
unsafe extern "C" fn fence_destroy(fence: *mut Fence) {
    if let Some(fence) = unsafe { rust::FenceHandle::from_raw(fence) } {
        unsafe {
            rust::fence_destroy(fence);
        }
    }
}
unsafe extern "C" fn fence_poll(fence: *const Fence) -> Status {
    boundary(|| {
        let fence = unsafe { rust::FenceHandle::from_raw(fence.cast_mut()) }.ok_or(INVALID)?;
        if unsafe { rust::fence_poll(fence)? } {
            Ok(())
        } else {
            Err(NOT_READY)
        }
    })
}
unsafe extern "C" fn fence_wait(fence: *const Fence, timeout: u64) -> Status {
    boundary(|| {
        let fence = unsafe { rust::FenceHandle::from_raw(fence.cast_mut()) }.ok_or(INVALID)?;
        unsafe { rust::fence_wait(fence, timeout) }
    })
}
unsafe extern "C" fn fence_reset(fence: *mut Fence) -> Status {
    boundary(|| {
        let fence = unsafe { rust::FenceHandle::from_raw(fence) }.ok_or(INVALID)?;
        unsafe { rust::fence_reset(fence) }
    })
}

unsafe extern "C" fn surface_win32_create(
    adapter: *const Adapter,
    desc: *const Win32SurfaceDesc,
    out: *mut *mut Surface,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let adapter = unsafe { adapter.as_ref() }.ok_or(INVALID)?;
        let desc = unsafe { desc.cast::<Record>().as_ref() }.ok_or(INVALID)?;
        desc.validate::<Win32SurfaceDesc>(WIN32_SURFACE_DESC)?;
        let desc = unsafe { &*ptr::from_ref(desc).cast::<Win32SurfaceDesc>() };
        let surface = unsafe { rust::surface_win32_create(adapter, desc)? };
        unsafe {
            out.write(surface.as_raw());
        }
        Ok(())
    })
}
unsafe extern "C" fn presentation_contract_query(
    adapter: *const Adapter,
    surface: *const Surface,
    profile: u32,
    out: *mut PresentationContract,
) -> Status {
    boundary(|| {
        if out.is_null() {
            return Err(INVALID);
        }
        let adapter = unsafe { adapter.as_ref() }.ok_or(INVALID)?;
        let surface =
            unsafe { rust::SurfaceHandle::from_raw(surface.cast_mut()) }.ok_or(INVALID)?;
        let value = unsafe { rust::presentation_contract_query(adapter, surface, profile)? };
        unsafe {
            out.write(value);
        }
        Ok(())
    })
}
unsafe extern "C" fn chain_create(
    device: *mut Device,
    desc: *const Record,
    out: *mut ChainCreateOutcome,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ChainCreateOutcome::default());
    }
    boundary(|| {
        let device = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let desc = unsafe { desc.as_ref() }.ok_or(INVALID)?;
        unsafe { rust::chain_create(device, desc, &mut *out) }
    })
}
unsafe extern "C" fn chain_destroy(chain: *mut Swapchain) {
    if let Some(chain) = unsafe { rust::SwapchainHandle::from_raw(chain) } {
        unsafe {
            rust::chain_destroy(chain);
        }
    }
}
unsafe extern "C" fn chain_info_query(chain: *const Swapchain, out: *mut ChainInfo) {
    if let Some(chain) = unsafe { chain.as_ref() } {
        if !out.is_null() {
            unsafe {
                out.write(chain.info());
            }
        }
    }
}
unsafe extern "C" fn chain_images(
    chain: *const Swapchain,
    capacity: u32,
    out: *mut *mut Image,
    count: *mut u32,
) -> Status {
    boundary(|| {
        unsafe { chain.as_ref() }
            .ok_or(INVALID)
            .and_then(|chain| unsafe { chain.images(capacity, out, count) })
    })
}
unsafe extern "C" fn chain_resize(
    chain: *mut Swapchain,
    desc: *const ChainResizeDesc,
    out: *mut ChainResizeOutcome,
) -> Status {
    boundary(|| {
        let chain = unsafe { chain.as_ref() }.ok_or(INVALID)?;
        if out.is_null() {
            return Err(INVALID);
        }
        let info = chain.info();
        unsafe {
            out.write(ChainResizeOutcome {
                generation: info.generation,
                state: info.state,
                image_effect: IMAGES_UNCHANGED,
            });
        }
        let desc = unsafe { desc.as_ref() }.ok_or(INVALID)?;
        // The complete value was initialized above, including on local rejection.
        chain.resize(desc, unsafe { &mut *out })
    })
}
unsafe extern "C" fn chain_pacing_handle(
    chain: *const Swapchain,
    out: *mut PacingHandle,
) -> Status {
    boundary(|| {
        if out.is_null() {
            return Err(INVALID);
        }
        let mut value = PacingHandle::default();
        unsafe { chain.as_ref() }
            .ok_or(INVALID)?
            .pacing_handle(&mut value)?;
        unsafe {
            out.write(value);
        }
        Ok(())
    })
}
unsafe extern "C" fn acquire(
    chain: *mut Swapchain,
    desc: *const AcquireDesc,
    out: *mut Frame,
) -> Status {
    boundary(|| unsafe {
        if out.is_null() {
            return Err(INVALID);
        }
        chain
            .as_ref()
            .ok_or(INVALID)?
            .acquire_frame(desc.as_ref().ok_or(INVALID)?, out)
    })
}
unsafe extern "C" fn abandon_scratch_requirements(
    device: *const Device,
    count: u32,
    out: *mut HostRequirements,
) -> Status {
    boundary(|| {
        if out.is_null() {
            return Err(INVALID);
        }
        let device = unsafe { rust::DeviceHandle::from_raw(device.cast_mut()) }.ok_or(INVALID)?;
        let value = unsafe { rust::abandon_scratch_requirements(device, count)? };
        unsafe {
            out.write(value);
        }
        Ok(())
    })
}
unsafe extern "C" fn abandon(
    chain: *mut Swapchain,
    count: u32,
    frames: *const Frame,
    scratch: HostSpan,
    effect: *mut u32,
) -> Status {
    if effect.is_null() {
        return INVALID;
    }
    unsafe {
        effect.write(FRAME_RETAINED);
    }
    boundary(|| unsafe {
        let chain = chain.as_ref().ok_or(INVALID)?;
        let frames = native::command_slice(frames, count)?;
        chain.abandon(frames, scratch, &mut *effect)
    })
}
unsafe extern "C" fn present_scratch_requirements(
    device: *const Device,
    waits: u32,
    items: u32,
    out: *mut HostRequirements,
) -> Status {
    boundary(|| {
        if out.is_null() {
            return Err(INVALID);
        }
        let device = unsafe { rust::DeviceHandle::from_raw(device.cast_mut()) }.ok_or(INVALID)?;
        let value = unsafe { rust::present_scratch_requirements(device, waits, items)? };
        unsafe {
            out.write(value);
        }
        Ok(())
    })
}
unsafe extern "C" fn queue_present(
    queue: *mut Queue,
    desc: *const PresentDesc,
    out: *mut PresentOutcome,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(PresentOutcome::default());
    }
    boundary(|| unsafe {
        let queue = rust::QueueHandle::from_raw(queue).ok_or(INVALID)?;
        rust::queue_present(queue, desc.as_ref().ok_or(INVALID)?, &mut *out)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_negotiation_is_atomic_and_needs_no_loader() {
        let empty = PresentationApi {
            discovery_create: None,
            adapter_support: None,
            surface_wayland_create: None,
            surface_destroy: None,
            surface_query: None,
            surface_mode_query: None,
            binary_create: None,
            binary_destroy: None,
            fence_create: None,
            fence_destroy: None,
            fence_poll: None,
            fence_wait: None,
            fence_reset: None,
            surface_win32_create: None,
            presentation_contract_query: None,
            chain_create: None,
            chain_destroy: None,
            chain_info_query: None,
            chain_images: None,
            chain_resize: None,
            chain_pacing_handle: None,
            acquire: None,
            abandon_scratch_requirements: None,
            abandon: None,
            present_scratch_requirements: None,
            queue_present: None,
        };
        let mut table = empty;
        for (version, size, expected) in [
            (
                VERSION + 1,
                size_of::<PresentationApi>() as u32,
                VERSION_MISMATCH,
            ),
            (VERSION, 0, INVALID),
            (VERSION, size_of::<PresentationApi>() as u32 - 1, INVALID),
        ] {
            assert_eq!(
                unsafe { ogpu_next_presentation_api(version, size, &mut table) },
                expected
            );
            assert!(table.discovery_create.is_none());
            assert!(table.adapter_support.is_none());
            assert!(table.surface_wayland_create.is_none());
            assert!(table.surface_destroy.is_none());
            assert!(table.surface_query.is_none());
            assert!(table.surface_mode_query.is_none());
            assert!(table.binary_create.is_none() && table.binary_destroy.is_none());
            assert!(table.fence_create.is_none() && table.fence_destroy.is_none());
            assert!(table.surface_win32_create.is_none());
            assert!(table.presentation_contract_query.is_none());
            assert!(table.chain_create.is_none());
            assert!(table.chain_destroy.is_none());
            assert!(table.chain_info_query.is_none());
            assert!(table.chain_images.is_none());
            assert!(table.chain_resize.is_none());
            assert!(table.chain_pacing_handle.is_none());
            assert!(table.acquire.is_none());
            assert!(table.abandon_scratch_requirements.is_none());
            assert!(table.abandon.is_none());
            assert!(table.present_scratch_requirements.is_none());
            assert!(table.queue_present.is_none());
            assert!(
                table.fence_poll.is_none()
                    && table.fence_wait.is_none()
                    && table.fence_reset.is_none()
            );
        }
        assert_eq!(
            unsafe {
                ogpu_next_presentation_api(VERSION, size_of::<PresentationApi>() as u32, &mut table)
            },
            OK
        );
        assert!(table.discovery_create.is_some());
        assert!(table.adapter_support.is_some());
        assert!(table.surface_wayland_create.is_some());
        assert!(table.surface_destroy.is_some());
        assert!(table.surface_query.is_some());
        assert!(table.surface_mode_query.is_some());
        assert!(table.binary_create.is_some() && table.binary_destroy.is_some());
        assert!(table.fence_create.is_some() && table.fence_destroy.is_some());
        assert!(table.surface_win32_create.is_some());
        assert!(table.presentation_contract_query.is_some());
        assert!(table.chain_create.is_some());
        assert!(table.chain_destroy.is_some());
        assert!(table.chain_info_query.is_some());
        assert!(table.chain_images.is_some());
        assert!(table.chain_resize.is_some());
        assert!(table.chain_pacing_handle.is_some());
        assert!(table.acquire.is_some());
        assert!(table.abandon_scratch_requirements.is_some());
        assert!(table.abandon.is_some());
        assert!(table.present_scratch_requirements.is_some());
        assert!(table.queue_present.is_some());
        assert!(
            table.fence_poll.is_some() && table.fence_wait.is_some() && table.fence_reset.is_some()
        );
        let mut support = PresentationSupport {
            swapchain: 99,
            swapchain_maintenance: 99,
        };
        assert_eq!(
            unsafe { table.adapter_support.unwrap()(ptr::null(), &mut support) },
            INVALID
        );
        assert_eq!(
            support,
            PresentationSupport {
                swapchain: 99,
                swapchain_maintenance: 99
            }
        );
        assert_eq!(
            unsafe { ogpu_next_presentation_api(VERSION, 0, ptr::null_mut()) },
            INVALID
        );
        for platforms in [0, 2, 3, u32::MAX] {
            let mut out = ptr::dangling_mut();
            assert_eq!(
                unsafe { table.discovery_create.unwrap()(platforms, &mut out) },
                UNSUPPORTED
            );
            assert!(out.is_null());
        }
        let mut out = ptr::dangling_mut();
        assert_eq!(
            unsafe { table.surface_wayland_create.unwrap()(ptr::null(), ptr::null(), &mut out) },
            INVALID
        );
        assert!(out.is_null());
        unsafe {
            table.surface_destroy.unwrap()(ptr::null_mut());
        }
    }
}
