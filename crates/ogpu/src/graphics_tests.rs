use super::*;

#[test]
#[ignore = "requires a graphics+compute Vulkan device"]
fn gpu_rgba16_transfers_and_raster() {
    let instance = Arc::new(Instance::new().unwrap());
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let d = match Device::new_graphics(instance.clone(), physical) {
            Ok(d) => d,
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        };
        let desc = ImageDesc {
            format: 2,
            ..ImageDesc::rgba8(3, 5)
        };
        assert_eq!(Image::check_support(&d, desc).unwrap(), 120);
        let image = Rc::new(Image::new(d.clone(), desc).unwrap());
        let buffer = Rc::new(Buffer::new(d.clone(), 136).unwrap());
        // Packed binary16: >1, fractional, negative, opaque. Prefix/suffix guards.
        let pixel: Vec<u8> = [0x4400u16, 0x3800, 0xc000, 0x3c00]
            .into_iter()
            .flat_map(u16::to_ne_bytes)
            .collect();
        let expected = pixel.repeat(15);
        buffer.write(0, &[0x5a; 136]).unwrap();
        buffer.write(8, &expected).unwrap();
        let mut batch = Batch::new(d.clone()).unwrap();
        for offset in [1, 4, 12] {
            assert_eq!(
                batch
                    .copy_buffer_to_image(buffer.clone(), offset, image.clone())
                    .unwrap_err()
                    .status,
                INVALID_ARGUMENT
            );
            assert_eq!(
                batch
                    .copy_image_to_buffer(image.clone(), buffer.clone(), offset)
                    .unwrap_err()
                    .status,
                INVALID_ARGUMENT
            );
        }
        assert_eq!(
            batch
                .copy_buffer_to_image(buffer.clone(), 24, image.clone())
                .unwrap_err()
                .status,
            OUT_OF_RANGE
        );
        batch.discard_image(image.clone()).unwrap();
        batch
            .copy_buffer_to_image(buffer.clone(), 8, image.clone())
            .unwrap();
        unsafe {
            batch.submit().unwrap().wait().unwrap();
        }
        buffer.write(8, &[0; 120]).unwrap();
        let mut batch = Batch::new(d.clone()).unwrap();
        batch
            .copy_image_to_buffer(image.clone(), buffer.clone(), 8)
            .unwrap();
        unsafe {
            batch.submit().unwrap().wait().unwrap();
        }
        let mut actual = [0u8; 136];
        unsafe {
            buffer.read(0, actual.as_mut_ptr(), actual.len()).unwrap();
        }
        assert_eq!(&actual[8..128], expected.as_slice());
        assert_eq!(&actual[..8], &[0x5a; 8]);
        assert_eq!(&actual[128..], &[0x5a; 8]);
        let vertex = words(include_bytes!(
            "../../../examples/shaders/fullscreen.vert.spv"
        ));
        let fragment = words(include_bytes!("../../../examples/shaders/hdr.frag.spv"));
        for format in [1, 3] {
            assert!(
                matches!(unsafe { Raster::new(d.clone(), &vertex, &fragment, 0, [&[], &[]], 0, format) },
                Err(e) if e.status == INVALID_ARGUMENT)
            );
        }
        let raster = Rc::new(unsafe {
            Raster::new(d.clone(), &vertex, &fragment, 0, [&[], &[]], 0, 2).unwrap()
        });
        let rgba8 = Rc::new(Image::new(d.clone(), ImageDesc::rgba8(3, 5)).unwrap());
        let indirect = Rc::new(Buffer::new(d.clone(), 16).unwrap());
        indirect
            .write(
                0,
                &[3u32, 1, 0, 0]
                    .into_iter()
                    .flat_map(u32::to_ne_bytes)
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        let mut batch = Batch::new(d.clone()).unwrap();
        assert_eq!(
            batch
                .draw(
                    raster.clone(),
                    rgba8,
                    indirect.clone(),
                    0,
                    &[],
                    batch::CLEAR
                )
                .unwrap_err()
                .status,
            INVALID_ARGUMENT
        );
        batch
            .draw(raster, image.clone(), indirect, 0, &[], batch::CLEAR)
            .unwrap();
        batch
            .copy_image_to_buffer(image, buffer.clone(), 8)
            .unwrap();
        unsafe {
            batch.submit().unwrap().wait().unwrap();
            buffer.read(0, actual.as_mut_ptr(), actual.len()).unwrap();
        }
        assert_eq!(&actual[8..128], expected.as_slice());
        assert_eq!(&actual[..8], &[0x5a; 8]);
        assert_eq!(&actual[128..], &[0x5a; 8]);
        tested += 1;
    }
    assert!(tested > 0, "no suitable device");
}

fn image_memory(flags: &[u32]) -> vk::VkPhysicalDeviceMemoryProperties {
    let mut memory = vk::VkPhysicalDeviceMemoryProperties {
        memoryTypeCount: flags.len() as u32,
        ..Default::default()
    };
    for (ty, &flags) in memory.memoryTypes.iter_mut().zip(flags) {
        ty.propertyFlags = flags;
    }
    memory
}

#[test]
fn image_memory_prefers_device_only_local_in_either_order() {
    let local = vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT;
    let visible = vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT;
    let memory = image_memory(&[local, local | visible, visible]);
    assert_eq!(image_memory_type(&memory, 0b111), Some(0));
    let reversed = image_memory(&[visible, local | visible, local]);
    assert_eq!(image_memory_type(&reversed, 0b111), Some(2));
    // Locality is the first preference, not invisibility by itself.
    let memory = image_memory(&[local | visible, 0]);
    assert_eq!(image_memory_type(&memory, 0b11), Some(0));
}

#[test]
fn image_memory_accepts_visible_local_and_unified_memory() {
    let local = vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT;
    let visible = vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT;
    let coherent = vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_HOST_COHERENT_BIT;
    let memory = image_memory(&[local, local | visible, visible]);
    // Requirements can exclude the otherwise preferred device-only type.
    assert_eq!(image_memory_type(&memory, 0b110), Some(1));
    assert_eq!(image_memory_type(&memory, 0b100), Some(2));
    let unified = image_memory(&[local | visible | coherent]);
    assert_eq!(image_memory_type(&unified, 1), Some(0));
    assert_eq!(image_memory_type(&unified, 0), None);
}

#[test]
fn image_memory_preserves_eligibility_and_exclusions() {
    let local = vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT;
    let visible = vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT;
    let memory = image_memory(&[
        local | vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_DEVICE_COHERENT_BIT_AMD,
        local | vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_PROTECTED_BIT,
        local | vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_LAZILY_ALLOCATED_BIT,
        local | visible,
    ]);
    assert_eq!(image_memory_type(&memory, 0b1111), Some(3));
    assert_eq!(image_memory_type(&memory, 0b0111), None);
    assert_eq!(image_memory_type(&memory, 0), None);
    assert_eq!(image_memory_type(&memory, 1 << 31), None);
    assert_eq!(image_memory_type(&image_memory(&[]), u32::MAX), None);
}

#[test]
fn image_description_respects_usage_and_dimension_limits() {
    let limits = vk::VkPhysicalDeviceLimits {
        maxImageDimension1D: 256,
        maxImageDimension2D: 512,
        maxFramebufferWidth: 64,
        maxFramebufferHeight: 64,
        maxViewportDimensions: [64, 64],
        viewportBoundsRange: [-64.0, 64.0],
        ..Default::default()
    };
    let desc = ImageDesc {
        dimension: 1,
        width: 256,
        height: 1,
        format: 1,
        usage: SAMPLED | COPY_DST,
        reserved: 0,
    };
    assert_eq!(desc.validate(&limits).unwrap(), 1024);
    assert_eq!(
        ImageDesc { format: 3, ..desc }.validate(&limits).unwrap(),
        2048
    );
    for usage in [STORAGE, COLOR] {
        assert_eq!(
            ImageDesc {
                format: 3,
                usage,
                ..desc
            }
            .validate(&limits)
            .unwrap_err()
            .status,
            INVALID_ARGUMENT
        );
    }
    assert_eq!(
        ImageDesc { format: 2, ..desc }.validate(&limits).unwrap(),
        2048
    );
    assert_eq!(
        ImageDesc {
            format: 2,
            ..ImageDesc::rgba8(64, 64)
        }
        .validate(&limits)
        .unwrap(),
        32768
    );
    assert_eq!(
        ImageDesc {
            format: 2,
            ..ImageDesc::rgba8(65, 1)
        }
        .validate(&limits)
        .unwrap_err()
        .status,
        INVALID_ARGUMENT
    );
    assert_eq!(
        ImageDesc {
            dimension: 2,
            width: 512,
            height: 512,
            ..desc
        }
        .validate(&limits)
        .unwrap(),
        512 * 512 * 4
    );
    for bad in [
        ImageDesc {
            dimension: 0,
            ..desc
        },
        ImageDesc {
            dimension: 3,
            ..desc
        },
        ImageDesc { width: 0, ..desc },
        ImageDesc { width: 257, ..desc },
        ImageDesc { height: 0, ..desc },
        ImageDesc { height: 2, ..desc },
        ImageDesc { format: 4, ..desc },
        ImageDesc {
            reserved: 1,
            ..desc
        },
        ImageDesc { usage: 0, ..desc },
        ImageDesc { usage: 32, ..desc },
        ImageDesc {
            usage: COLOR,
            ..desc
        },
        ImageDesc {
            dimension: 2,
            width: 1,
            usage: COLOR,
            ..desc
        },
        ImageDesc::rgba8(65, 1),
        ImageDesc {
            dimension: 2,
            width: 513,
            ..desc
        },
    ] {
        assert_eq!(
            bad.validate(&limits).unwrap_err().status,
            INVALID_ARGUMENT,
            "{bad:?}"
        );
    }
}
use crate::compute::batch::{COMPUTE_WRITE, INDIRECT_READ, TRANSFER_WRITE, VERTEX_READ};

thread_local! {
    static QUERY_FAMILIES: Cell<vk::PFN_vkGetPhysicalDeviceQueueFamilyProperties> = const { Cell::new(None) };
    static DEDICATED_COMPUTE: Cell<bool> = const { Cell::new(false) };
    static QUERY_FEATURES: Cell<vk::PFN_vkGetPhysicalDeviceFeatures2> = const { Cell::new(None) };
    static CREATE_DEVICE: Cell<vk::PFN_vkCreateDevice> = const { Cell::new(None) };
    static FEATURE_MODE: Cell<u32> = const { Cell::new(0) };
    static EXPECT_UNIFIED: Cell<bool> = const { Cell::new(false) };
    static EXPECT_RASTER: Cell<bool> = const { Cell::new(true) };
    static CREATED: Cell<bool> = const { Cell::new(false) };
}

// Test-only queue restriction: use a real compute-only family if one exists,
// preserving its Vulkan index. Production queue selection remains unchanged.
unsafe extern "C" fn compute_only_families(
    physical: vk::VkPhysicalDevice,
    count: *mut u32,
    properties: *mut vk::VkQueueFamilyProperties,
) {
    unsafe {
        (QUERY_FAMILIES.get().unwrap())(physical, count, properties);
        if properties.is_null() {
            return;
        }
        let families = std::slice::from_raw_parts_mut(properties, *count as usize);
        let dedicated = |p: &vk::VkQueueFamilyProperties| {
            p.queueCount != 0
                && p.queueFlags & vk::VkQueueFlagBits_VK_QUEUE_COMPUTE_BIT != 0
                && p.queueFlags & vk::VkQueueFlagBits_VK_QUEUE_GRAPHICS_BIT == 0
        };
        DEDICATED_COMPUTE.set(families.iter().any(dedicated));
        if DEDICATED_COMPUTE.get() {
            for family in families {
                if !dedicated(family) {
                    family.queueCount = 0;
                }
            }
        }
    }
}

unsafe extern "C" fn optional_image_features(
    physical: vk::VkPhysicalDevice,
    query: *mut vk::VkPhysicalDeviceFeatures2,
) {
    unsafe {
        (QUERY_FEATURES.get().unwrap())(physical, query);
        // This test intercepts Device::create_configured's known feature chain.
        let v12 = (*query)
            .pNext
            .cast::<vk::VkPhysicalDeviceVulkan12Features>();
        let v13 = (*v12).pNext.cast::<vk::VkPhysicalDeviceVulkan13Features>();
        let v14 = (*v13).pNext.cast::<vk::VkPhysicalDeviceVulkan14Features>();
        let images = (*v14)
            .pNext
            .cast::<vk::VkPhysicalDeviceUnifiedImageLayoutsFeaturesKHR>();
        if FEATURE_MODE.get() == 2 {
            (*v13).dynamicRendering = vk::VK_FALSE;
        }
        EXPECT_UNIFIED.set(false);
        if !images.is_null() {
            if FEATURE_MODE.get() == 1 {
                (*images).unifiedImageLayouts = vk::VK_FALSE;
            }
            EXPECT_UNIFIED.set((*images).unifiedImageLayouts != 0);
        }
    }
}

unsafe extern "C" fn checked_image_device(
    physical: vk::VkPhysicalDevice,
    create: *const vk::VkDeviceCreateInfo,
    allocator: *const vk::VkAllocationCallbacks,
    output: *mut vk::VkDevice,
) -> vk::VkResult {
    unsafe {
        let names = std::slice::from_raw_parts(
            (*create).ppEnabledExtensionNames,
            (*create).enabledExtensionCount as usize,
        );
        let enabled = names
            .iter()
            .any(|&name| std::ffi::CStr::from_ptr(name) == c"VK_KHR_unified_image_layouts");
        assert_eq!(enabled, EXPECT_UNIFIED.get());
        let storage16 = (*create)
            .pNext
            .cast::<vk::VkPhysicalDevice16BitStorageFeatures>();
        assert_eq!(
            (*storage16).sType,
            vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_16BIT_STORAGE_FEATURES
        );
        assert_eq!((*storage16).storageBuffer16BitAccess, vk::VK_TRUE);
        assert_eq!(
            (*storage16).uniformAndStorageBuffer16BitAccess,
            vk::VK_FALSE
        );
        assert_eq!((*storage16).storagePushConstant16, vk::VK_FALSE);
        assert_eq!((*storage16).storageInputOutput16, vk::VK_FALSE);
        let v12 = (*storage16)
            .pNext
            .cast::<vk::VkPhysicalDeviceVulkan12Features>();
        let v13 = (*v12).pNext.cast::<vk::VkPhysicalDeviceVulkan13Features>();
        assert_eq!((*v13).dynamicRendering != 0, EXPECT_RASTER.get());
        assert_eq!((*v12).shaderFloat16, vk::VK_FALSE);
        assert_eq!((*v12).shaderInt8, vk::VK_FALSE);
        assert!((*create).pEnabledFeatures.is_null());
        let v14 = (*v13).pNext.cast::<vk::VkPhysicalDeviceVulkan14Features>();
        let heap = (*v14)
            .pNext
            .cast::<vk::VkPhysicalDeviceDescriptorHeapFeaturesEXT>();
        let addresses = (*heap)
            .pNext
            .cast::<vk::VkPhysicalDeviceDeviceAddressCommandsFeaturesKHR>();
        let untyped = (*addresses)
            .pNext
            .cast::<vk::VkPhysicalDeviceShaderUntypedPointersFeaturesKHR>();
        let images = (*untyped)
            .pNext
            .cast::<vk::VkPhysicalDeviceUnifiedImageLayoutsFeaturesKHR>();
        assert_eq!(!images.is_null(), enabled);
        if enabled {
            assert_eq!((*images).unifiedImageLayouts, vk::VK_TRUE);
            assert_eq!((*images).unifiedImageLayoutsVideo, vk::VK_FALSE);
        }
        CREATED.set(true);
        (CREATE_DEVICE.get().unwrap())(physical, create, allocator, output)
    }
}

#[test]
#[ignore = "requires graphics Vulkan; unified layouts optional, dynamic rendering required"]
fn gpu_optional_unified_layouts() {
    let instance = Arc::new(Instance::new().unwrap());
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        match Device::new_graphics(instance.clone(), physical) {
            Ok(_) => {}
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        }
        for (graphics, mode) in [
            (true, 0),
            (true, 1),
            (true, 2),
            (false, 0),
            (false, 1),
            (false, 2),
        ] {
            FEATURE_MODE.set(mode);
            EXPECT_RASTER.set(graphics);
            CREATED.set(false);
            let result = Device::create_configured(instance.clone(), physical, graphics, |f| {
                QUERY_FEATURES.set(f.vkGetPhysicalDeviceFeatures2);
                f.vkGetPhysicalDeviceFeatures2 = Some(optional_image_features);
                CREATE_DEVICE.set(f.vkCreateDevice);
                f.vkCreateDevice = Some(checked_image_device);
                if !graphics && mode == 2 {
                    QUERY_FAMILIES.set(f.vkGetPhysicalDeviceQueueFamilyProperties);
                    f.vkGetPhysicalDeviceQueueFamilyProperties = Some(compute_only_families);
                }
            });
            if graphics && mode == 2 {
                assert!(matches!(result, Err(e) if e.status == UNSUPPORTED));
                assert!(!CREATED.get());
            } else {
                let device = result.unwrap();
                assert_eq!(device.graphics, graphics);
                assert!(CREATED.get());
                let desc = ImageDesc {
                    usage: if graphics {
                        COLOR
                    } else {
                        SAMPLED | STORAGE | COPY_SRC | COPY_DST
                    },
                    ..ImageDesc::rgba8(2, 3)
                };
                let target = Rc::new(Image::new(device.clone(), desc).unwrap());
                let images = Rc::new(ImageHeap::new(device.clone(), 1).unwrap());
                let samplers = Rc::new(SamplerHeap::new(device.clone(), 1).unwrap());
                if !graphics && mode == 2 {
                    eprintln!(
                        "compute image/heap commands: dedicated compute family={}, index={}",
                        DEDICATED_COMPUTE.get(),
                        device.family
                    );
                }
                let mut batch = Batch::new(device).unwrap();
                batch.discard_image(target).unwrap();
                batch.bind_images(images).unwrap();
                batch.bind_samplers(samplers).unwrap();
                unsafe { batch.submit().unwrap().wait().unwrap() };
            }
        }
        tested += 1;
    }
    assert!(tested > 0, "No graphics+compute device found");
}

thread_local! {
    static SUBMIT: Cell<vk::PFN_vkQueueSubmit2> = const { Cell::new(None) };
    static REJECT: Cell<bool> = const { Cell::new(false) };
}
unsafe extern "C" fn reject_submit(
    q: vk::VkQueue,
    n: u32,
    s: *const vk::VkSubmitInfo2,
    f: vk::VkFence,
) -> vk::VkResult {
    if REJECT.replace(false) {
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
    } else {
        unsafe { (SUBMIT.get().unwrap())(q, n, s, f) }
    }
}

#[test]
#[ignore = "requires graphics Vulkan; preservation, LOAD/CLEAR and abandoned/rejected discard"]
fn gpu_image_preservation() {
    let instance = Arc::new(Instance::new().unwrap());
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let d = match Device::create_configured(instance.clone(), physical, true, |f| {
            SUBMIT.set(f.vkQueueSubmit2);
            f.vkQueueSubmit2 = Some(reject_submit);
        }) {
            Ok(d) => d,
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        };
        REJECT.set(false);
        let target = Rc::new(Image::new(d.clone(), ImageDesc::rgba8(64, 64)).unwrap());
        let pattern = Rc::new(unsafe {
            Raster::new(
                d.clone(),
                &words(include_bytes!(
                    "../../../examples/shaders/fullscreen.vert.spv"
                )),
                &words(include_bytes!(
                    "../../../examples/shaders/image-pattern.frag.spv"
                )),
                0,
                [&[], &[]],
                0,
                0,
            )
            .unwrap()
        });
        let triangle = Rc::new(unsafe {
            Raster::new(
                d.clone(),
                &words(include_bytes!(
                    "../../../examples/shaders/triangle.vert.spv"
                )),
                &words(include_bytes!(
                    "../../../examples/shaders/triangle.frag.spv"
                )),
                16,
                [&[], &[]],
                0,
                0,
            )
            .unwrap()
        });
        let vertices = Buffer::new(d.clone(), 48).unwrap();
        let data: [f32; 12] = [
            -0.75, -0.75, 0.0, 1.0, 0.75, -0.75, 0.0, 1.0, 0.0, 0.75, 0.0, 1.0,
        ];
        vertices
            .write(
                0,
                &data
                    .into_iter()
                    .flat_map(f32::to_ne_bytes)
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        let indirect = Rc::new(Buffer::new(d.clone(), 16).unwrap());
        indirect
            .write(
                0,
                &[3u32, 1, 0, 0]
                    .into_iter()
                    .flat_map(u32::to_ne_bytes)
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        let mut root = [0u8; 16];
        root[..8].copy_from_slice(&vertices.address.to_ne_bytes());
        let output = Rc::new(Buffer::new(d.clone(), target.size).unwrap());
        // A rejected first producer does not authorize using its image. Retry the
        // complete initialization explicitly before any dependent work is submitted.
        let mut rejected = Batch::new(d.clone()).unwrap();
        rejected.discard_image(target.clone()).unwrap();
        rejected
            .copy_buffer_to_image(output.clone(), 0, target.clone())
            .unwrap();
        REJECT.set(true);
        assert!(unsafe { rejected.submit() }.is_err());
        let mut first = Batch::new(d.clone()).unwrap();
        first
            .draw(
                pattern,
                target.clone(),
                indirect.clone(),
                0,
                &[],
                batch::CLEAR,
            )
            .unwrap();
        let initial = unsafe { first.submit().unwrap() };
        // Neither abandoned recording nor rejected discard may erase the pattern.
        let mut abandoned = Batch::new(d.clone()).unwrap();
        abandoned.discard_image(target.clone()).unwrap();
        drop(abandoned);
        let mut rejected = Batch::new(d.clone()).unwrap();
        rejected.discard_image(target.clone()).unwrap();
        REJECT.set(true);
        assert!(unsafe { rejected.submit() }.is_err());
        for load in [batch::LOAD, batch::CLEAR] {
            let mut draw = Batch::new(d.clone()).unwrap();
            assert!(draw
                .draw(
                    triangle.clone(),
                    target.clone(),
                    indirect.clone(),
                    0,
                    &root,
                    99
                )
                .is_err());
            draw.barrier(
                batch::COLOR_WRITE | batch::TRANSFER_READ,
                batch::COLOR_READ | batch::COLOR_WRITE,
            )
            .unwrap();
            draw.draw(
                triangle.clone(),
                target.clone(),
                indirect.clone(),
                0,
                &root,
                load,
            )
            .unwrap();
            let rendered = unsafe { draw.submit().unwrap() };
            let mut copy = Batch::new(d.clone()).unwrap();
            copy.copy_image_to_buffer(target.clone(), output.clone(), 0)
                .unwrap();
            let mut copied = unsafe { copy.submit().unwrap() };
            copied.wait().unwrap();
            let mut pixels = vec![0u8; target.size];
            unsafe {
                output.read(0, pixels.as_mut_ptr(), pixels.len()).unwrap();
            }
            for (x, y) in [(0, 0), (63, 63), (4, 32), (60, 32)] {
                let expected = if load == batch::LOAD {
                    [
                        (x * 17 + y * 3) as u8,
                        (x * 5 + y * 29) as u8,
                        (x * 11 + y * 7) as u8,
                        255,
                    ]
                } else {
                    [0, 0, 0, 255]
                };
                assert_eq!(&pixels[(y * 64 + x) * 4..][..4], &expected);
            }
            assert_eq!(&pixels[(24 * 64 + 32) * 4..][..4], &[255, 0, 0, 255]);
            drop(copied);
            drop(rendered);
        }
        drop(initial);
        tested += 1;
    }
    assert!(tested > 0);
}

thread_local! {
    static MODULE_CALLS: Cell<u32> = const { Cell::new(0) };
    static IMAGE_SUPPORT_MODE: Cell<u32> = const { Cell::new(0) };
    static REAL_MODULE: Cell<vk::PFN_vkCreateShaderModule> = const { Cell::new(None) };
    static REAL_PIPELINE: Cell<vk::PFN_vkCreateGraphicsPipelines> = const { Cell::new(None) };
}

unsafe extern "C" fn limited_image_support(
    _physical: vk::VkPhysicalDevice,
    format: vk::VkFormat,
    kind: vk::VkImageType,
    tiling: vk::VkImageTiling,
    usage: vk::VkImageUsageFlags,
    flags: vk::VkImageCreateFlags,
    out: *mut vk::VkImageFormatProperties,
) -> vk::VkResult {
    assert_eq!(format, vk::VkFormat_VK_FORMAT_R32_SFLOAT);
    assert_eq!(kind, vk::VkImageType_VK_IMAGE_TYPE_1D);
    assert_eq!(tiling, vk::VkImageTiling_VK_IMAGE_TILING_OPTIMAL);
    assert_eq!(
        usage,
        vk::VkImageUsageFlagBits_VK_IMAGE_USAGE_TRANSFER_SRC_BIT
    );
    assert_eq!(flags, 0);
    match IMAGE_SUPPORT_MODE.get() {
        0 => vk::VkResult_VK_ERROR_FORMAT_NOT_SUPPORTED,
        1 => vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY,
        mode => {
            unsafe {
                *out = vk::VkImageFormatProperties {
                    maxExtent: vk::VkExtent3D {
                        width: if mode == 2 { 4 } else { 8 },
                        height: 1,
                        depth: 1,
                    },
                    sampleCounts: if mode == 3 {
                        vk::VkSampleCountFlagBits_VK_SAMPLE_COUNT_2_BIT
                    } else {
                        vk::VkSampleCountFlagBits_VK_SAMPLE_COUNT_1_BIT
                    },
                    ..Default::default()
                };
            }
            vk::VkResult_VK_SUCCESS
        }
    }
}
macro_rules! fail {
    ($name:ident($($arg:ident: $ty:ty),*)) => {
        unsafe extern "C" fn $name($($arg: $ty),*) -> vk::VkResult {
            vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
        }
    };
}
fail!(fail_image(_d: vk::VkDevice, _i: *const vk::VkImageCreateInfo,
    _a: *const vk::VkAllocationCallbacks, _o: *mut vk::VkImage));
fail!(fail_memory(_d: vk::VkDevice, _i: *const vk::VkMemoryAllocateInfo,
    _a: *const vk::VkAllocationCallbacks, _o: *mut vk::VkDeviceMemory));
fail!(fail_bind(_d: vk::VkDevice, _i: vk::VkImage, _m: vk::VkDeviceMemory, _o: vk::VkDeviceSize));
fail!(fail_view(_d: vk::VkDevice, _i: *const vk::VkImageViewCreateInfo,
    _a: *const vk::VkAllocationCallbacks, _o: *mut vk::VkImageView));

unsafe extern "C" fn fail_second_module(
    device: vk::VkDevice,
    info: *const vk::VkShaderModuleCreateInfo,
    allocator: *const vk::VkAllocationCallbacks,
    output: *mut vk::VkShaderModule,
) -> vk::VkResult {
    let n = MODULE_CALLS.get();
    MODULE_CALLS.set(n + 1);
    if n == 0 {
        unsafe { (REAL_MODULE.get().unwrap())(device, info, allocator, output) }
    } else {
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
    }
}

unsafe extern "C" fn fail_after_pipeline(
    device: vk::VkDevice,
    cache: vk::VkPipelineCache,
    count: u32,
    infos: *const vk::VkGraphicsPipelineCreateInfo,
    allocator: *const vk::VkAllocationCallbacks,
    output: *mut vk::VkPipeline,
) -> vk::VkResult {
    let status =
        unsafe { (REAL_PIPELINE.get().unwrap())(device, cache, count, infos, allocator, output) };
    // Simulate partial pipeline creation: even an error can return owned pipeline handles.
    if status == vk::VkResult_VK_SUCCESS {
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
    } else {
        status
    }
}

#[test]
#[ignore = "requires Vulkan; injects graphics creation failures with real resource cleanup"]
fn gpu_graphics_failures() {
    let instance = Arc::new(Instance::new().unwrap());
    let vertex = words(include_bytes!(
        "../../../examples/shaders/triangle.vert.spv"
    ));
    let fragment = words(include_bytes!(
        "../../../examples/shaders/triangle.frag.spv"
    ));
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        match Device::new_graphics(instance.clone(), physical) {
            Ok(_) => {}
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        }
        for mode in 0..4 {
            IMAGE_SUPPORT_MODE.set(mode);
            let mut device = Device::new(instance.clone(), physical).unwrap();
            let f = &mut Rc::get_mut(&mut device).unwrap().f;
            f.vkGetPhysicalDeviceImageFormatProperties = Some(limited_image_support);
            // If preflight accidentally proceeds to creation this returns a distinct error.
            f.vkCreateImage = Some(fail_image);
            let desc = ImageDesc {
                dimension: 1,
                width: 8,
                height: 1,
                format: 1,
                usage: COPY_SRC,
                reserved: 0,
            };
            let query = Image::check_support(&device, desc).unwrap_err();
            let create = Image::new(device.clone(), desc).err().unwrap();
            assert_eq!((query.status, query.vk), (create.status, create.vk));
            if mode == 1 {
                assert_eq!(query.status, crate::VULKAN_ERROR);
                assert_eq!(query.vk, vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY);
            } else {
                assert_eq!(query.status, UNSUPPORTED);
            }
        }
        for point in 0..6 {
            let mut device = Device::new_graphics(instance.clone(), physical).unwrap();
            let f = &mut Rc::get_mut(&mut device).unwrap().f;
            match point {
                0 => f.vkCreateImage = Some(fail_image),
                1 => f.vkAllocateMemory = Some(fail_memory),
                2 => f.vkBindImageMemory = Some(fail_bind),
                3 => f.vkCreateImageView = Some(fail_view),
                4 => {
                    MODULE_CALLS.set(0);
                    REAL_MODULE.set(f.vkCreateShaderModule);
                    f.vkCreateShaderModule = Some(fail_second_module);
                }
                _ => {
                    REAL_PIPELINE.set(f.vkCreateGraphicsPipelines);
                    f.vkCreateGraphicsPipelines = Some(fail_after_pipeline);
                }
            }
            let result = if point < 4 {
                // Creation/allocation failures must not affect allocation-free preflight.
                assert_eq!(
                    Image::check_support(&device, ImageDesc::rgba8(64, 64)).unwrap(),
                    64 * 64 * 4
                );
                Image::new(device.clone(), ImageDesc::rgba8(64, 64)).map(drop)
            } else {
                unsafe { Raster::new(device.clone(), &vertex, &fragment, 16, [&[], &[]], 0, 0) }
                    .map(drop)
            };
            assert_eq!(
                result.unwrap_err().vk,
                vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
            );
            assert_eq!(
                Rc::strong_count(&device),
                1,
                "Partial objects must not retain device"
            );
        }
        // Exercise rejection without issuing unsupported commands to a real queue.
        let device = Device::new(instance.clone(), physical).unwrap();
        assert!(
            matches!(Image::new(device.clone(), ImageDesc::rgba8(64, 64)), Err(e) if e.status == UNSUPPORTED)
        );
        assert!(
            matches!(unsafe { Raster::new(device.clone(), &vertex, &fragment, 16, [&[], &[]], 0, 0) }, Err(e) if e.status == UNSUPPORTED)
        );
        let mut batch = Batch::new(device).unwrap();
        assert_eq!(
            batch
                .barrier(COMPUTE_WRITE, VERTEX_READ)
                .unwrap_err()
                .status,
            UNSUPPORTED
        );
        assert_eq!(
            batch.barrier(u32::MAX, VERTEX_READ).unwrap_err().status,
            INVALID_ARGUMENT
        );
        batch.barrier(COMPUTE_WRITE, TRANSFER_WRITE).unwrap();
        tested += 1;
    }
    assert!(tested > 0, "No graphics+compute device found");
}

fn words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect()
}

#[test]
fn queue_selection_keeps_graphics_optional() {
    let compute = vk::VkQueueFlagBits_VK_QUEUE_COMPUTE_BIT;
    let graphics = vk::VkQueueFlagBits_VK_QUEUE_GRAPHICS_BIT;
    let families = [
        vk::VkQueueFamilyProperties {
            queueCount: 1,
            queueFlags: compute,
            ..Default::default()
        },
        vk::VkQueueFamilyProperties {
            queueCount: 1,
            queueFlags: graphics,
            ..Default::default()
        },
        vk::VkQueueFamilyProperties {
            queueCount: 0,
            queueFlags: compute | graphics,
            ..Default::default()
        },
        vk::VkQueueFamilyProperties {
            queueCount: 1,
            queueFlags: compute | graphics,
            ..Default::default()
        },
    ];
    assert_eq!(queue_family(&families, false), Some(0));
    assert_eq!(queue_family(&families, true), Some(3));
    assert_eq!(queue_family(&families[..3], true), None);
    assert_eq!(queue_family(&[], false), None);
}

#[test]
fn target_extents_and_memory_types_are_checked() {
    let limits = vk::VkPhysicalDeviceLimits {
        maxImageDimension2D: 4096,
        maxFramebufferWidth: 2048,
        maxFramebufferHeight: 2048,
        maxViewportDimensions: [1024, 1024],
        viewportBoundsRange: [-1024.0, 1024.0],
        ..Default::default()
    };
    assert_eq!(target_size(64, 64, &limits).unwrap(), 16384);
    for (w, h) in [(0, 64), (64, 0), (1025, 1), (1, 2049), (u32::MAX, u32::MAX)] {
        assert_eq!(
            target_size(w, h, &limits).unwrap_err().status,
            INVALID_ARGUMENT
        );
    }
    let mut memory = vk::VkPhysicalDeviceMemoryProperties {
        memoryTypeCount: 2,
        ..Default::default()
    };
    memory.memoryTypes[1].propertyFlags =
        vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT;
    assert_eq!(image_memory_type(&memory, 3), Some(1));
    assert_eq!(image_memory_type(&memory, 1), Some(0));
    for excluded in [
        vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_DEVICE_COHERENT_BIT_AMD,
        vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_PROTECTED_BIT,
        vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_LAZILY_ALLOCATED_BIT,
    ] {
        memory.memoryTypes[1].propertyFlags = excluded;
        assert_eq!(image_memory_type(&memory, 3), Some(0));
        assert_eq!(image_memory_type(&memory, 2), None);
    }
}

#[test]
#[ignore = "requires a graphics+compute Vulkan device"]
fn gpu_graphics() {
    let instance = Arc::new(Instance::new().unwrap());
    let compute = words(include_bytes!(
        "../../../examples/shaders/triangle.comp.spv"
    ));
    let vertex = words(include_bytes!(
        "../../../examples/shaders/triangle.vert.spv"
    ));
    let fragment = words(include_bytes!(
        "../../../examples/shaders/triangle.frag.spv"
    ));
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let device = match Device::new_graphics(instance.clone(), physical) {
            Ok(d) => d,
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        };
        assert!(
            matches!(Image::new(device.clone(), ImageDesc::rgba8(0, 64)), Err(e) if e.status == INVALID_ARGUMENT)
        );
        assert!(
            matches!(unsafe { Raster::new(device.clone(), &[0; 5], &fragment, 16, [&[], &[]], 0, 0) }, Err(e) if e.status == INVALID_ARGUMENT)
        );
        let kernel = Rc::new(unsafe { Kernel::new(device.clone(), &compute, 16, &[]).unwrap() });
        let raster = Rc::new(unsafe {
            Raster::new(device.clone(), &vertex, &fragment, 16, [&[], &[]], 0, 0).unwrap()
        });
        let target = Rc::new(Image::new(device.clone(), ImageDesc::rgba8(64, 64)).unwrap());
        let vertices = Buffer::new(device.clone(), 48).unwrap();
        let indirect = Rc::new(Buffer::new(device.clone(), 16).unwrap());
        let non_attachment = Rc::new(
            Image::new(
                device.clone(),
                ImageDesc {
                    usage: SAMPLED | COPY_SRC,
                    ..ImageDesc::rgba8(64, 64)
                },
            )
            .unwrap(),
        );
        assert_eq!(
            Batch::new(device.clone())
                .unwrap()
                .draw(
                    raster.clone(),
                    non_attachment,
                    indirect.clone(),
                    0,
                    &[0; 16],
                    batch::CLEAR
                )
                .unwrap_err()
                .status,
            INVALID_ARGUMENT
        );
        assert_eq!(vertices.address().unwrap() % 16, 0);
        let mut root = [0u8; 16];
        root[..8].copy_from_slice(&vertices.address().unwrap().to_ne_bytes());
        root[8..].copy_from_slice(&indirect.address().unwrap().to_ne_bytes());
        let other = Device::new_graphics(instance.clone(), physical).unwrap();
        let mut wrong = Batch::new(other).unwrap();
        assert_eq!(
            wrong
                .draw(
                    raster.clone(),
                    target.clone(),
                    indirect.clone(),
                    0,
                    &root,
                    batch::CLEAR
                )
                .unwrap_err()
                .status,
            INVALID_ARGUMENT
        );
        let mut completions = Vec::new();
        let mut outputs = Vec::new();
        let timed = device.timing_info().is_ok();
        for _ in 0..2 {
            let output = Rc::new(Buffer::new(device.clone(), target.size * 2 + 8).unwrap());
            output.write(0, &vec![0xAA; target.size * 2 + 8]).unwrap();
            let mut batch = Batch::new(device.clone()).unwrap();
            if timed {
                batch.enable_timing().unwrap();
            }
            // Copy initialization is now a trusted cross-submission obligation.
            assert_eq!(
                batch
                    .draw(
                        raster.clone(),
                        target.clone(),
                        indirect.clone(),
                        1,
                        &root,
                        batch::CLEAR
                    )
                    .unwrap_err()
                    .status,
                INVALID_ARGUMENT
            );
            assert_eq!(
                batch
                    .draw(
                        raster.clone(),
                        target.clone(),
                        indirect.clone(),
                        4,
                        &root,
                        batch::CLEAR
                    )
                    .unwrap_err()
                    .status,
                OUT_OF_RANGE
            );
            assert_eq!(
                batch
                    .draw(
                        raster.clone(),
                        target.clone(),
                        indirect.clone(),
                        0,
                        &root[..8],
                        batch::CLEAR
                    )
                    .unwrap_err()
                    .status,
                INVALID_ARGUMENT
            );
            // Protect vertices/indirect arguments from prior graphics reads on target reuse.
            batch
                .barrier(COMPUTE_WRITE | VERTEX_READ | INDIRECT_READ, COMPUTE_WRITE)
                .unwrap();
            batch.dispatch(kernel.clone(), [1, 1, 1], &root).unwrap();
            batch
                .barrier(COMPUTE_WRITE, VERTEX_READ | INDIRECT_READ)
                .unwrap();
            for copy in 0..2 {
                batch
                    .draw(
                        raster.clone(),
                        target.clone(),
                        indirect.clone(),
                        0,
                        &root,
                        batch::CLEAR,
                    )
                    .unwrap();
                assert_eq!(
                    batch
                        .copy_image_to_buffer(target.clone(), output.clone(), 1)
                        .unwrap_err()
                        .status,
                    INVALID_ARGUMENT
                );
                assert_eq!(
                    batch
                        .copy_image_to_buffer(target.clone(), output.clone(), output.size)
                        .unwrap_err()
                        .status,
                    OUT_OF_RANGE
                );
                // Explicitly order writes to this readback allocation between copies.
                batch.barrier(TRANSFER_WRITE, TRANSFER_WRITE).unwrap();
                batch
                    .copy_image_to_buffer(target.clone(), output.clone(), 4 + copy * target.size)
                    .unwrap();
            }
            completions.push(unsafe { batch.submit().unwrap() });
            outputs.push(output);
        }
        root.fill(0);
        drop(target);
        drop(raster);
        drop(kernel);
        drop(indirect);
        drop(device);
        // Submitted target/pipeline/indirect ownership must survive all public owners.
        for mut completion in completions {
            completion.wait().unwrap();
            if timed {
                let elapsed = completion.elapsed_ns().unwrap();
                assert!(elapsed.is_finite() && elapsed >= 0.0);
            }
        }
        for output in outputs {
            let mut pixels = vec![0; output.size];
            unsafe {
                output.read(0, pixels.as_mut_ptr(), pixels.len()).unwrap();
            }
            assert_eq!(&pixels[..4], &[0xAA; 4]);
            assert_eq!(&pixels[pixels.len() - 4..], &[0xAA; 4]);
            for copy in 0..2 {
                for (x, y, expected) in [
                    (32, 24, [255, 0, 0, 255]),
                    (24, 24, [255, 0, 0, 255]),
                    (40, 24, [255, 0, 0, 255]),
                    (32, 40, [255, 0, 0, 255]),
                    (0, 0, [0, 0, 0, 255]),
                    (63, 63, [0, 0, 0, 255]),
                    (4, 32, [0, 0, 0, 255]),
                    (60, 32, [0, 0, 0, 255]),
                ] {
                    let offset = 4 + copy * 64 * 64 * 4 + (y * 64 + x) * 4;
                    assert_eq!(&pixels[offset..offset + 4], &expected, "pixel ({x}, {y})");
                }
            }
        }
        drop(vertices); // Only pointer-referenced allocation: caller retains through completion.
        tested += 1;
    }
    assert!(tested > 0, "No graphics+compute device found");
}
