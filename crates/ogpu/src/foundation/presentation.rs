//! Opt-in Vulkan WSI. Cold setup/query work, independent of headless device setup.
use super::*;

macro_rules! functions {
    ($($name:ident: $ty:ident),* $(,)?) => {
        #[allow(non_snake_case)]
        struct Functions { $($name: vk::$ty,)* }
        impl Functions {
            fn load(instance: &Instance) -> Result<Self, Status> {
                Ok(Self { $( $name: {
                    // SAFETY: each procedure name is paired with its generated PFN.
                    let f: vk::$ty = unsafe { std::mem::transmute(instance.proc(
                        std::ffi::CStr::from_bytes_with_nul(concat!(stringify!($name), "\0").as_bytes()).unwrap())) };
                    Some(f.ok_or(UNSUPPORTED)?)
                }, )* })
            }
        }
    };
}
functions! {
    vkCreateWaylandSurfaceKHR: PFN_vkCreateWaylandSurfaceKHR,
    vkDestroySurfaceKHR: PFN_vkDestroySurfaceKHR,
    vkGetPhysicalDeviceSurfaceSupportKHR: PFN_vkGetPhysicalDeviceSurfaceSupportKHR,
    vkGetPhysicalDeviceSurfaceCapabilities2KHR: PFN_vkGetPhysicalDeviceSurfaceCapabilities2KHR,
    vkGetPhysicalDeviceSurfaceFormats2KHR: PFN_vkGetPhysicalDeviceSurfaceFormats2KHR,
    vkGetPhysicalDeviceSurfacePresentModesKHR: PFN_vkGetPhysicalDeviceSurfacePresentModesKHR,
}

fn status(result: vk::VkResult) -> Result<(), Status> {
    match result {
        vk::VkResult_VK_SUCCESS => Ok(()),
        vk::VkResult_VK_ERROR_SURFACE_LOST_KHR => Err(SURFACE_LOST),
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY | vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY => {
            Err(OUT_OF_MEMORY)
        }
        vk::VkResult_VK_ERROR_DEVICE_LOST => Err(DEVICE_LOST),
        vk::VkResult_VK_ERROR_EXTENSION_NOT_PRESENT | vk::VkResult_VK_ERROR_FEATURE_NOT_PRESENT => {
            Err(UNSUPPORTED)
        }
        _ => Err(BACKEND_ERROR),
    }
}

pub(super) fn support(adapter: &Adapter) -> Result<PresentationSupport, Status> {
    support_native(&adapter.instance, adapter.physical)
}
pub(super) fn support_native(
    instance: &Instance,
    physical: vk::VkPhysicalDevice,
) -> Result<PresentationSupport, Status> {
    let swapchain = instance
        .supports_extension(physical, c"VK_KHR_swapchain")
        .map_err(loader_error)?;
    let maintenance = instance
        .supports_extension(physical, c"VK_KHR_swapchain_maintenance1")
        .map_err(loader_error)?;
    let mut features = vk::VkPhysicalDeviceSwapchainMaintenance1FeaturesKHR {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SWAPCHAIN_MAINTENANCE_1_FEATURES_KHR,
        ..Default::default()
    };
    if maintenance {
        // SAFETY: the generated PFN matches the named core query. Does not need WSI enabling.
        let get: vk::PFN_vkGetPhysicalDeviceFeatures2 =
            unsafe { std::mem::transmute(instance.proc(c"vkGetPhysicalDeviceFeatures2")) };
        let mut f = vk::VkPhysicalDeviceFeatures2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2,
            pNext: ptr::from_mut(&mut features).cast(),
            ..Default::default()
        };
        unsafe {
            get.ok_or(BACKEND_ERROR)?(physical, &mut f);
        }
    }
    Ok(PresentationSupport {
        swapchain: u32::from(swapchain),
        swapchain_maintenance: features.swapchainMaintenance1,
    })
}

pub struct Surface {
    handle: vk::VkSurfaceKHR,
    instance: Arc<Instance>,
    f: Functions,
}
// SAFETY: surface handles are non-owning Rust tokens. Every operation requires
// live borrowed native objects and externally synchronized lifecycle accesses.
unsafe impl Send for Surface {}
unsafe impl Sync for Surface {}
impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            (self.f.vkDestroySurfaceKHR.unwrap())(self.instance.handle(), self.handle, ptr::null());
        }
    }
}
impl Surface {
    pub(super) unsafe fn create(
        adapter: &Adapter,
        desc: &WaylandSurfaceDesc,
    ) -> Result<Box<Self>, Status> {
        desc.header
            .validate::<WaylandSurfaceDesc>(WAYLAND_SURFACE_DESC)?;
        if desc.display.is_null() || desc.surface.is_null() {
            return Err(INVALID);
        }
        if !adapter.instance.wayland_enabled() {
            return Err(UNSUPPORTED);
        }
        let f = Functions::load(&adapter.instance)?;
        let info = vk::VkWaylandSurfaceCreateInfoKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_WAYLAND_SURFACE_CREATE_INFO_KHR,
            display: desc.display.cast(),
            surface: desc.surface.cast(),
            ..Default::default()
        };
        let mut handle = ptr::null_mut();
        status(unsafe {
            (f.vkCreateWaylandSurfaceKHR.unwrap())(
                adapter.instance.handle(),
                &info,
                ptr::null(),
                &mut handle,
            )
        })?;
        Ok(Box::new(Self {
            handle,
            instance: adapter.instance.clone(),
            f,
        }))
    }

    pub(super) unsafe fn query(
        &self,
        adapter: &Adapter,
        mode: Option<u32>,
        query: *mut Query,
    ) -> Result<(), Status> {
        if !Arc::ptr_eq(&self.instance, &adapter.instance) || query.is_null() {
            return Err(INVALID);
        }
        let header = unsafe { query.cast::<Record>().read() };
        header.validate::<Query>(header.kind)?;
        let q = unsafe { &mut *query };
        validate_query(q)?;
        if mode.is_some() {
            if !matches!(
                header.kind,
                SURFACE_CAPABILITIES | SURFACE_SCALING | SURFACE_COMPATIBLE_MODES
            ) {
                return Err(UNSUPPORTED);
            }
        } else if matches!(header.kind, SURFACE_SCALING | SURFACE_COMPATIBLE_MODES) {
            return Err(UNSUPPORTED);
        }
        if matches!(
            header.kind,
            SURFACE_CAPABILITIES
                | SURFACE_FORMATS
                | SURFACE_PRESENT_MODES
                | SURFACE_SCALING
                | SURFACE_COMPATIBLE_MODES
        ) {
            // Vulkan requires physical-device surface support before these queries.
            // Do not issue invalid native calls for a non-presenting adapter.
            let mut any = false;
            for queue in &adapter.snapshot.queues {
                let mut supported = 0;
                status(unsafe {
                    (self.f.vkGetPhysicalDeviceSurfaceSupportKHR.unwrap())(
                        adapter.physical,
                        queue.domain,
                        self.handle,
                        &mut supported,
                    )
                })?;
                any |= supported != 0;
                if any {
                    break;
                }
            }
            if !any {
                return Err(UNSUPPORTED);
            }
        }
        if let Some(mode) = mode {
            let modes = enumerate(0u32, |count, data| unsafe {
                (self.f.vkGetPhysicalDeviceSurfacePresentModesKHR.unwrap())(
                    adapter.physical,
                    self.handle,
                    count,
                    data,
                )
            })?;
            if !modes.contains(&mode) {
                return Err(UNSUPPORTED);
            }
        }
        let mode_info = vk::VkSurfacePresentModeKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SURFACE_PRESENT_MODE_KHR,
            presentMode: mode.unwrap_or(0),
            ..Default::default()
        };
        let info = vk::VkPhysicalDeviceSurfaceInfo2KHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SURFACE_INFO_2_KHR,
            pNext: if mode.is_some() {
                ptr::from_ref(&mode_info).cast()
            } else {
                ptr::null()
            },
            surface: self.handle,
        };
        // Stage native results before copy_out: failures (including enumeration
        // races) never partially overwrite the caller's storage or count.
        unsafe {
            match header.kind {
                SURFACE_SCALING => {
                    let mut scaling = vk::VkSurfacePresentScalingCapabilitiesKHR {
                        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SURFACE_PRESENT_SCALING_CAPABILITIES_KHR,
                        ..Default::default()
                    };
                    let mut caps = vk::VkSurfaceCapabilities2KHR {
                        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SURFACE_CAPABILITIES_2_KHR,
                        pNext: ptr::from_mut(&mut scaling).cast(),
                        ..Default::default()
                    };
                    status(
                        (self.f.vkGetPhysicalDeviceSurfaceCapabilities2KHR.unwrap())(
                            adapter.physical,
                            &info,
                            &mut caps,
                        ),
                    )?;
                    copy_out(
                        q,
                        &[SurfaceScaling {
                            scaling: scaling.supportedPresentScaling,
                            gravity_x: scaling.supportedPresentGravityX,
                            gravity_y: scaling.supportedPresentGravityY,
                            reserved: 0,
                            min_extent: [
                                scaling.minScaledImageExtent.width,
                                scaling.minScaledImageExtent.height,
                            ],
                            max_extent: [
                                scaling.maxScaledImageExtent.width,
                                scaling.maxScaledImageExtent.height,
                            ],
                        }],
                    )
                }
                SURFACE_COMPATIBLE_MODES => {
                    // This native output chain does not report VK_INCOMPLETE on
                    // truncation. Reserve a spare element and retry a full array.
                    let modes = enumerate_impl(0u32, true, |count, data| {
                        let mut compatible = vk::VkSurfacePresentModeCompatibilityKHR {
                            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SURFACE_PRESENT_MODE_COMPATIBILITY_KHR,
                            presentModeCount: *count,
                            pPresentModes: data,
                            ..Default::default()
                        };
                        let mut caps = vk::VkSurfaceCapabilities2KHR {
                            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SURFACE_CAPABILITIES_2_KHR,
                            pNext: ptr::from_mut(&mut compatible).cast(),
                            ..Default::default()
                        };
                        let result = (self.f.vkGetPhysicalDeviceSurfaceCapabilities2KHR.unwrap())(
                            adapter.physical,
                            &info,
                            &mut caps,
                        );
                        *count = compatible.presentModeCount;
                        result
                    })?;
                    copy_out(q, &modes)
                }
                SURFACE_CAPABILITIES => {
                    let mut caps = vk::VkSurfaceCapabilities2KHR {
                        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SURFACE_CAPABILITIES_2_KHR,
                        ..Default::default()
                    };
                    status(
                        (self.f.vkGetPhysicalDeviceSurfaceCapabilities2KHR.unwrap())(
                            adapter.physical,
                            &info,
                            &mut caps,
                        ),
                    )?;
                    let c = caps.surfaceCapabilities;
                    copy_out(
                        q,
                        &[SurfaceCapabilities {
                            min_image_count: c.minImageCount,
                            max_image_count: c.maxImageCount,
                            current_extent: [c.currentExtent.width, c.currentExtent.height],
                            min_extent: [c.minImageExtent.width, c.minImageExtent.height],
                            max_extent: [c.maxImageExtent.width, c.maxImageExtent.height],
                            max_layers: c.maxImageArrayLayers,
                            supported_transforms: c.supportedTransforms,
                            current_transform: c.currentTransform,
                            supported_alpha: c.supportedCompositeAlpha,
                            image_usage: u64::from(c.supportedUsageFlags & 255),
                            native_image_usage: c.supportedUsageFlags,
                            reserved: 0,
                        }],
                    )
                }
                SURFACE_FORMATS => {
                    let items = enumerate(
                        vk::VkSurfaceFormat2KHR {
                            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SURFACE_FORMAT_2_KHR,
                            ..Default::default()
                        },
                        |count, data| {
                            (self.f.vkGetPhysicalDeviceSurfaceFormats2KHR.unwrap())(
                                adapter.physical,
                                &info,
                                count,
                                data,
                            )
                        },
                    )?;
                    let mut values = Vec::new();
                    values
                        .try_reserve_exact(items.len())
                        .map_err(|_| OUT_OF_MEMORY)?;
                    for item in items {
                        let f = item.surfaceFormat;
                        let format = native::image_format_id(f.format);
                        values.push(SurfaceFormat {
                            format,
                            native_format: f.format,
                            color_space: f.colorSpace,
                            reserved: 0,
                        });
                    }
                    copy_out(q, &values)
                }
                SURFACE_PRESENT_MODES => {
                    let modes = enumerate(0u32, |count, data| {
                        (self.f.vkGetPhysicalDeviceSurfacePresentModesKHR.unwrap())(
                            adapter.physical,
                            self.handle,
                            count,
                            data,
                        )
                    })?;
                    copy_out(q, &modes)
                }
                SURFACE_QUEUES => {
                    let mut queues = Vec::new();
                    queues
                        .try_reserve_exact(adapter.snapshot.queues.len())
                        .map_err(|_| OUT_OF_MEMORY)?;
                    for queue in &adapter.snapshot.queues {
                        let mut supported = 0;
                        status((self.f.vkGetPhysicalDeviceSurfaceSupportKHR.unwrap())(
                            adapter.physical,
                            queue.domain,
                            self.handle,
                            &mut supported,
                        ))?;
                        queues.push(SurfaceQueue {
                            domain: queue.domain,
                            supported,
                        });
                    }
                    copy_out(q, &queues)
                }
                _ => Err(UNSUPPORTED),
            }
        }
    }
}
fn validate_query(q: &Query) -> Result<(), Status> {
    let size = match q.header.kind {
        SURFACE_CAPABILITIES => size_of::<SurfaceCapabilities>(),
        SURFACE_FORMATS => size_of::<SurfaceFormat>(),
        SURFACE_PRESENT_MODES => size_of::<u32>(),
        SURFACE_QUEUES => size_of::<SurfaceQueue>(),
        SURFACE_SCALING => size_of::<SurfaceScaling>(),
        SURFACE_COMPATIBLE_MODES => size_of::<u32>(),
        _ => return Err(UNSUPPORTED),
    };
    if q.reserved != 0 || q.element_size as usize != size || (q.capacity != 0 && q.data.is_null()) {
        return Err(INVALID);
    }
    Ok(())
}

// Unlike plain output arrays, extensible Vulkan records need sType/pNext
// initialized for every element before the driver sees them. Bound retries.
fn enumerate<T: Copy>(
    initial: T,
    call: impl FnMut(*mut u32, *mut T) -> vk::VkResult,
) -> Result<Vec<T>, Status> {
    enumerate_impl(initial, false, call)
}
fn enumerate_impl<T: Copy>(
    initial: T,
    unmarked_truncation: bool,
    mut call: impl FnMut(*mut u32, *mut T) -> vk::VkResult,
) -> Result<Vec<T>, Status> {
    for _ in 0..8 {
        let mut count = 0;
        let r = call(&mut count, ptr::null_mut());
        if r == vk::VkResult_VK_INCOMPLETE {
            continue;
        }
        status(r)?;
        if count == 0 {
            return Ok(Vec::new());
        }
        if unmarked_truncation {
            count = count.checked_add(1).ok_or(CAPACITY)?;
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(count as usize)
            .map_err(|_| OUT_OF_MEMORY)?;
        values.resize(count as usize, initial);
        let r = call(&mut count, values.as_mut_ptr());
        if r == vk::VkResult_VK_INCOMPLETE {
            continue;
        }
        status(r)?;
        if count as usize > values.len() {
            return Err(INTERNAL_ERROR);
        }
        if unmarked_truncation && count as usize == values.len() {
            continue;
        }
        values.truncate(count as usize);
        return Ok(values);
    }
    Err(BACKEND_ERROR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_enumeration_retries_unmarked_truncation() {
        let mut calls = 0;
        let values = enumerate_impl(0u32, true, |count, data| unsafe {
            calls += 1;
            if data.is_null() {
                *count = if calls == 1 { 1 } else { 2 };
                return vk::VkResult_VK_SUCCESS;
            }
            if calls == 2 {
                assert_eq!(*count, 2);
                *data = 1;
                *data.add(1) = 2;
                // Full output with SUCCESS is ambiguous: retry, not partial success.
            } else {
                assert_eq!(*count, 3);
                *data = 2;
                *data.add(1) = 1;
                *count = 2;
            }
            vk::VkResult_VK_SUCCESS
        })
        .unwrap();
        assert_eq!(calls, 4);
        assert_eq!(values, [2, 1]);
        let result = enumerate_impl(0u32, true, |count, data| unsafe {
            if data.is_null() {
                *count = 1;
            }
            vk::VkResult_VK_SUCCESS
        });
        assert_eq!(result, Err(BACKEND_ERROR));
    }

    #[test]
    fn surface_enumeration_initializes_records_and_retries_growth() {
        let mut calls = 0;
        let result = enumerate(42u32, |count, data| unsafe {
            calls += 1;
            if data.is_null() {
                *count = 2;
                return vk::VkResult_VK_SUCCESS;
            }
            assert_eq!(*count, 2);
            assert_eq!(*data, 42);
            assert_eq!(*data.add(1), 42);
            if calls == 2 {
                *data = 99;
                return vk::VkResult_VK_INCOMPLETE;
            }
            *data = 7;
            *count = 1;
            vk::VkResult_VK_SUCCESS
        })
        .unwrap();
        assert_eq!(result, [7]);
        assert_eq!(calls, 4);
    }
    #[test]
    fn surface_enumeration_bounds_retries_and_preserves_native_errors() {
        assert_eq!(
            enumerate(0u32, |_, _| vk::VkResult_VK_INCOMPLETE),
            Err(BACKEND_ERROR)
        );
        assert_eq!(
            enumerate(0u32, |_, _| vk::VkResult_VK_ERROR_SURFACE_LOST_KHR),
            Err(SURFACE_LOST)
        );
        assert_eq!(
            enumerate(0u32, |_, _| vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY),
            Err(OUT_OF_MEMORY)
        );
        assert_eq!(enumerate(0u32, |_, _| vk::VkResult_VK_SUCCESS), Ok(vec![]));
    }
    #[test]
    fn surface_queries_check_schema_before_driver_calls() {
        for (kind, size) in [
            (SURFACE_CAPABILITIES, size_of::<SurfaceCapabilities>()),
            (SURFACE_FORMATS, size_of::<SurfaceFormat>()),
            (SURFACE_PRESENT_MODES, 4),
            (SURFACE_QUEUES, size_of::<SurfaceQueue>()),
            (SURFACE_SCALING, size_of::<SurfaceScaling>()),
            (SURFACE_COMPATIBLE_MODES, size_of::<u32>()),
        ] {
            let mut q = Query {
                header: Record::new::<Query>(kind),
                data: ptr::null_mut(),
                capacity: 0,
                count: 55,
                element_size: size as u32,
                reserved: 0,
            };
            assert_eq!(validate_query(&q), Ok(()));
            q.capacity = 1;
            assert_eq!(validate_query(&q), Err(INVALID));
            q.capacity = 0;
            q.element_size -= 1;
            assert_eq!(validate_query(&q), Err(INVALID));
            assert_eq!(q.count, 55);
        }
    }
}
