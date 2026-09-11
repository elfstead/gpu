use super::*;
use crate::compute::batch::{COMPUTE_WRITE, INDIRECT_READ, TRANSFER_WRITE, VERTEX_READ};

thread_local! {
    static MODULE_CALLS: Cell<u32> = const { Cell::new(0) };
    static REAL_MODULE: Cell<vk::PFN_vkCreateShaderModule> = const { Cell::new(None) };
    static REAL_PIPELINE: Cell<vk::PFN_vkCreateGraphicsPipelines> = const { Cell::new(None) };
}
macro_rules! fail {
    ($name:ident($($arg:ident: $ty:ty),*)) => {
        unsafe extern "C" fn $name($($arg: $ty),*) -> vk::VkResult {
            vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
        }
    };
}
fail!(fail_pass(_d: vk::VkDevice, _i: *const vk::VkRenderPassCreateInfo,
    _a: *const vk::VkAllocationCallbacks, _o: *mut vk::VkRenderPass));
fail!(fail_image(_d: vk::VkDevice, _i: *const vk::VkImageCreateInfo,
    _a: *const vk::VkAllocationCallbacks, _o: *mut vk::VkImage));
fail!(fail_memory(_d: vk::VkDevice, _i: *const vk::VkMemoryAllocateInfo,
    _a: *const vk::VkAllocationCallbacks, _o: *mut vk::VkDeviceMemory));
fail!(fail_bind(_d: vk::VkDevice, _i: vk::VkImage, _m: vk::VkDeviceMemory, _o: vk::VkDeviceSize));
fail!(fail_view(_d: vk::VkDevice, _i: *const vk::VkImageViewCreateInfo,
    _a: *const vk::VkAllocationCallbacks, _o: *mut vk::VkImageView));
fail!(fail_framebuffer(_d: vk::VkDevice, _i: *const vk::VkFramebufferCreateInfo,
    _a: *const vk::VkAllocationCallbacks, _o: *mut vk::VkFramebuffer));
fail!(fail_layout(_d: vk::VkDevice, _i: *const vk::VkPipelineLayoutCreateInfo,
    _a: *const vk::VkAllocationCallbacks, _o: *mut vk::VkPipelineLayout));

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
        for point in 0..9 {
            let mut device = Device::new_graphics(instance.clone(), physical).unwrap();
            let f = &mut Rc::get_mut(&mut device).unwrap().f;
            match point {
                0 => f.vkCreateRenderPass = Some(fail_pass),
                1 => f.vkCreateImage = Some(fail_image),
                2 => f.vkAllocateMemory = Some(fail_memory),
                3 => f.vkBindImageMemory = Some(fail_bind),
                4 => f.vkCreateImageView = Some(fail_view),
                5 => f.vkCreateFramebuffer = Some(fail_framebuffer),
                6 => {
                    MODULE_CALLS.set(0);
                    REAL_MODULE.set(f.vkCreateShaderModule);
                    f.vkCreateShaderModule = Some(fail_second_module);
                }
                7 => f.vkCreatePipelineLayout = Some(fail_layout),
                _ => {
                    REAL_PIPELINE.set(f.vkCreateGraphicsPipelines);
                    f.vkCreateGraphicsPipelines = Some(fail_after_pipeline);
                }
            }
            let result = if point < 6 {
                Target::new(device.clone(), 64, 64).map(drop)
            } else {
                unsafe { Raster::new(device.clone(), &vertex, &fragment, 16) }.map(drop)
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
        let mut device = Device::new_graphics(instance.clone(), physical).unwrap();
        Rc::get_mut(&mut device).unwrap().graphics = false;
        assert!(matches!(Target::new(device.clone(), 64, 64), Err(e) if e.status == UNSUPPORTED));
        assert!(
            matches!(unsafe { Raster::new(device.clone(), &vertex, &fragment, 16) }, Err(e) if e.status == UNSUPPORTED)
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
            matches!(Target::new(device.clone(), 0, 64), Err(e) if e.status == INVALID_ARGUMENT)
        );
        assert!(
            matches!(unsafe { Raster::new(device.clone(), &[0; 5], &fragment, 16) }, Err(e) if e.status == INVALID_ARGUMENT)
        );
        let kernel = Rc::new(unsafe { Kernel::new(device.clone(), &compute, 16).unwrap() });
        let raster =
            Rc::new(unsafe { Raster::new(device.clone(), &vertex, &fragment, 16).unwrap() });
        let target = Rc::new(Target::new(device.clone(), 64, 64).unwrap());
        let vertices = Buffer::new(device.clone(), 48).unwrap();
        let indirect = Rc::new(Buffer::new(device.clone(), 16).unwrap());
        assert_eq!(vertices.address().unwrap() % 16, 0);
        let mut root = [0u8; 16];
        root[..8].copy_from_slice(&vertices.address().unwrap().to_ne_bytes());
        root[8..].copy_from_slice(&indirect.address().unwrap().to_ne_bytes());
        let other = Device::new_graphics(instance.clone(), physical).unwrap();
        let mut wrong = Batch::new(other).unwrap();
        assert_eq!(
            wrong
                .draw(raster.clone(), target.clone(), indirect.clone(), 0, &root)
                .unwrap_err()
                .status,
            INVALID_ARGUMENT
        );
        let mut completions = Vec::new();
        let mut outputs = Vec::new();
        for _ in 0..2 {
            let output = Rc::new(Buffer::new(device.clone(), target.size * 2 + 8).unwrap());
            output.write(0, &vec![0xAA; target.size * 2 + 8]).unwrap();
            let mut batch = Batch::new(device.clone()).unwrap();
            assert_eq!(
                batch
                    .copy_target(target.clone(), output.clone(), 4)
                    .unwrap_err()
                    .status,
                INVALID_ARGUMENT
            );
            assert_eq!(
                batch
                    .draw(raster.clone(), target.clone(), indirect.clone(), 1, &root)
                    .unwrap_err()
                    .status,
                INVALID_ARGUMENT
            );
            assert_eq!(
                batch
                    .draw(raster.clone(), target.clone(), indirect.clone(), 4, &root)
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
                        &root[..8]
                    )
                    .unwrap_err()
                    .status,
                INVALID_ARGUMENT
            );
            // Protect vertices/indirect arguments from prior graphics reads on target reuse.
            batch
                .barrier(COMPUTE_WRITE | VERTEX_READ | INDIRECT_READ, COMPUTE_WRITE)
                .unwrap();
            batch.dispatch(kernel.clone(), 1, &root).unwrap();
            batch
                .barrier(COMPUTE_WRITE, VERTEX_READ | INDIRECT_READ)
                .unwrap();
            for copy in 0..2 {
                batch
                    .draw(raster.clone(), target.clone(), indirect.clone(), 0, &root)
                    .unwrap();
                assert_eq!(
                    batch
                        .copy_target(target.clone(), output.clone(), 1)
                        .unwrap_err()
                        .status,
                    INVALID_ARGUMENT
                );
                assert_eq!(
                    batch
                        .copy_target(target.clone(), output.clone(), output.size)
                        .unwrap_err()
                        .status,
                    OUT_OF_RANGE
                );
                // Explicitly order writes to this readback allocation between copies.
                batch.barrier(TRANSFER_WRITE, TRANSFER_WRITE).unwrap();
                batch
                    .copy_target(target.clone(), output.clone(), 4 + copy * target.size)
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
