//! Public native interoperability fixture. Native Vulkan produces; OGPU consumes.
//! No private OGPU entry points, hidden submission or CPU payload handoff.
use ogpu::foundation::{rust as gpu, types::*};
use ogpu_vulkan_sys as vk;
use std::{ffi::CStr, ptr};
#[path = "../submission.rs"]
mod submission;

macro_rules! native {
    ($get:expr, $device:expr, $name:ident, $ty:ident) => {{
        let f: vk::$ty = {
            std::mem::transmute($get(
                $device,
                concat!(stringify!($name), "\0").as_ptr().cast(),
            ))
        };
        f.ok_or(UNSUPPORTED)?
    }};
}
fn check(result: vk::VkResult) -> gpu::Result<()> {
    if result == vk::VkResult_VK_SUCCESS {
        Ok(())
    } else {
        eprintln!("native Vulkan result={result}");
        Err(BACKEND_ERROR)
    }
}

pub fn run() -> gpu::Result<()> {
    let discovery = gpu::discover()?;
    let adapter = gpu::adapters(&discovery)
        .iter()
        .find(|a| gpu::adapter_features(a).baseline_supported != 0)
        .ok_or(UNSUPPORTED)?;
    let domain = gpu::adapter_queues(adapter)
        .iter()
        .find(|q| q.count > 0 && q.flags & (GRAPHICS | COMPUTE) != 0)
        .ok_or(UNSUPPORTED)?
        .domain;
    let device = gpu::device_create(
        adapter,
        &[QueueRequest {
            domain,
            count: 1,
            priority: 1.0,
        }],
        &[],
    )?;
    // SAFETY: all metadata below describes objects created on this exported
    // native device. Single-threaded queue access; explicit timeline/binary handoff.
    // On unknown submission/completion failure, leak pending objects and exit.
    unsafe {
        let queue = gpu::device_queue(device, domain, 0).ok_or(INVALID)?;
        let nd = gpu::device_native_vulkan(device)?;
        let nq = gpu::queue_native_vulkan(queue)?;
        assert_eq!((nq.domain, nq.index), (domain, 0));
        let native_owner = device;
        let imported_discovery = gpu::discovery_import_vulkan(&VulkanInstance {
            instance: nd.instance,
            api_version: nd.instance_api_version,
            flags: nd.instance_flags,
        })?;
        let requests = [QueueRequest {
            domain,
            count: 1,
            priority: 1.0,
        }];
        let device = gpu::adapters(&imported_discovery)
            .iter()
            .find_map(|a| gpu::device_import_vulkan(a, &requests, &[], &nd, NATIVE_BORROW).ok())
            .ok_or(UNSUPPORTED)?;
        let queue = gpu::device_queue(device, domain, 0).ok_or(INVALID)?;
        assert_eq!(gpu::queue_native_vulkan(queue)?.queue, nq.queue);
        // The imported device retains the borrowed instance wrapper, not native ownership.
        drop(imported_discovery);

        let path =
            std::env::var_os("OGPU_VULKAN_LIBRARY").unwrap_or_else(|| "libvulkan.so.1".into());
        let library = libloading::Library::new(path).map_err(|_| BACKEND_ERROR)?;
        let get = library
            .get::<vk::PFN_vkGetInstanceProcAddr>(b"vkGetInstanceProcAddr\0")
            .map_err(|_| BACKEND_ERROR)?
            .ok_or(UNSUPPORTED)?;
        let get_device: vk::PFN_vkGetDeviceProcAddr =
            std::mem::transmute(get(nd.instance.cast(), c"vkGetDeviceProcAddr".as_ptr()));
        let get_device = get_device.ok_or(UNSUPPORTED)?;
        let d = nd.device.cast();
        macro_rules! load {
            ($name:ident, $ty:ident) => {
                native!(get_device, d, $name, $ty)
            };
        }
        let create_buffer = load!(vkCreateBuffer, PFN_vkCreateBuffer);
        let buffer_requirements = load!(
            vkGetBufferMemoryRequirements,
            PFN_vkGetBufferMemoryRequirements
        );
        let allocate = load!(vkAllocateMemory, PFN_vkAllocateMemory);
        let free = load!(vkFreeMemory, PFN_vkFreeMemory);
        let bind_buffer = load!(vkBindBufferMemory, PFN_vkBindBufferMemory);
        let create_image = load!(vkCreateImage, PFN_vkCreateImage);
        let image_requirements = load!(
            vkGetImageMemoryRequirements,
            PFN_vkGetImageMemoryRequirements
        );
        let bind_image = load!(vkBindImageMemory, PFN_vkBindImageMemory);
        let create_semaphore = load!(vkCreateSemaphore, PFN_vkCreateSemaphore);
        let create_pool = load!(vkCreateCommandPool, PFN_vkCreateCommandPool);
        let destroy_pool = load!(vkDestroyCommandPool, PFN_vkDestroyCommandPool);
        let allocate_commands = load!(vkAllocateCommandBuffers, PFN_vkAllocateCommandBuffers);
        let begin = load!(vkBeginCommandBuffer, PFN_vkBeginCommandBuffer);
        let end = load!(vkEndCommandBuffer, PFN_vkEndCommandBuffer);
        let fill = load!(vkCmdFillBuffer, PFN_vkCmdFillBuffer);
        let clear = load!(vkCmdClearColorImage, PFN_vkCmdClearColorImage);
        let barrier = load!(vkCmdPipelineBarrier2, PFN_vkCmdPipelineBarrier2);
        let submit = load!(vkQueueSubmit2, PFN_vkQueueSubmit2);
        let mut buffer = ptr::null_mut();
        check(create_buffer(
            d,
            &vk::VkBufferCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO,
                size: 256,
                usage: vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_TRANSFER_SRC_BIT
                    | vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_TRANSFER_DST_BIT
                    | vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_SHADER_DEVICE_ADDRESS_BIT,
                ..Default::default()
            },
            ptr::null(),
            &mut buffer,
        ))?;
        let mut br = vk::VkMemoryRequirements::default();
        buffer_requirements(d, buffer, &mut br);
        let type_id = gpu::adapter_memory_types(adapter)
            .iter()
            .find(|m| br.memoryTypeBits & (1 << m.id) != 0 && m.properties & (32 | 64 | 128) == 0)
            .ok_or(UNSUPPORTED)?
            .id;
        let flags = vk::VkMemoryAllocateFlagsInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_FLAGS_INFO,
            flags: vk::VkMemoryAllocateFlagBits_VK_MEMORY_ALLOCATE_DEVICE_ADDRESS_BIT,
            ..Default::default()
        };
        let mut memory = ptr::null_mut();
        check(allocate(
            d,
            &vk::VkMemoryAllocateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
                pNext: ptr::from_ref(&flags).cast(),
                allocationSize: br.size + br.alignment,
                memoryTypeIndex: type_id,
            },
            ptr::null(),
            &mut memory,
        ))?;
        check(bind_buffer(d, buffer, memory, br.alignment))?;
        let md = MemoryDesc {
            header: Record::new::<MemoryDesc>(MEMORY_DESC),
            size: 128,
            alignment: 4,
            usage: USAGE_COPY_SRC | USAGE_COPY_DST,
            memory_type: type_id,
            kind: MEMORY_LINEAR,
            concurrent_domains: ptr::null(),
            concurrent_domain_count: 0,
            flags: 0,
        };
        let native_memory = VulkanMemory {
            memory: memory as u64,
            buffer: buffer as u64,
            allocation_size: br.size + br.alignment,
            allocation_offset: br.alignment + 64,
            buffer_size: 256,
            buffer_offset: 64,
        };
        assert_eq!(
            gpu::memory_import_vulkan(device, &md, &native_memory, 4),
            Err(INVALID)
        );
        assert_eq!(
            gpu::memory_import_vulkan(
                device,
                &md,
                &VulkanMemory {
                    buffer_offset: 200,
                    ..native_memory
                },
                NATIVE_OWN | NATIVE_OWN_BUFFER
            ),
            Err(INVALID)
        );
        let imported_memory =
            gpu::memory_import_vulkan(device, &md, &native_memory, NATIVE_OWN | NATIVE_OWN_BUFFER)?;
        assert_eq!(
            gpu::memory_info_query(imported_memory)?.allocation_offset,
            br.alignment + 64
        );
        assert_eq!(
            gpu::memory_native_vulkan(imported_memory)?.buffer_offset,
            64
        );
        let buffer_only = gpu::memory_import_vulkan(
            device,
            &md,
            &VulkanMemory {
                memory: 0,
                allocation_size: 0,
                allocation_offset: 0,
                ..native_memory
            },
            NATIVE_BORROW,
        )?;
        assert_eq!(
            gpu::memory_address(imported_memory.span(0, 128))?,
            gpu::memory_address(buffer_only.span(0, 128))?
        );
        assert!(matches!(
            gpu::memory_map(buffer_only.span(0, 128)),
            Err(UNSUPPORTED)
        ));
        // The owner survives wrapper destruction; recreate a second borrowed view.
        gpu::memory_destroy(buffer_only);
        let buffer_only = gpu::memory_import_vulkan(
            device,
            &md,
            &VulkanMemory {
                memory: 0,
                allocation_size: 0,
                allocation_offset: 0,
                ..native_memory
            },
            NATIVE_BORROW,
        )?;
        let mut image = ptr::null_mut();
        check(create_image(
            d,
            &vk::VkImageCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO,
                imageType: vk::VkImageType_VK_IMAGE_TYPE_2D,
                format: vk::VkFormat_VK_FORMAT_R8G8B8A8_UNORM,
                extent: vk::VkExtent3D {
                    width: 8,
                    height: 8,
                    depth: 1,
                },
                mipLevels: 1,
                arrayLayers: 1,
                samples: vk::VkSampleCountFlagBits_VK_SAMPLE_COUNT_1_BIT,
                usage: vk::VkImageUsageFlagBits_VK_IMAGE_USAGE_TRANSFER_SRC_BIT
                    | vk::VkImageUsageFlagBits_VK_IMAGE_USAGE_TRANSFER_DST_BIT,
                ..Default::default()
            },
            ptr::null(),
            &mut image,
        ))?;
        let mut ir = vk::VkMemoryRequirements::default();
        image_requirements(d, image, &mut ir);
        let image_type = gpu::adapter_memory_types(adapter)
            .iter()
            .find(|m| ir.memoryTypeBits & (1 << m.id) != 0 && m.properties & (32 | 64 | 128) == 0)
            .ok_or(UNSUPPORTED)?
            .id;
        let dedicated = vk::VkMemoryDedicatedAllocateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_DEDICATED_ALLOCATE_INFO,
            image,
            ..Default::default()
        };
        let mut image_memory = ptr::null_mut();
        check(allocate(
            d,
            &vk::VkMemoryAllocateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
                pNext: ptr::from_ref(&dedicated).cast(),
                allocationSize: ir.size,
                memoryTypeIndex: image_type,
            },
            ptr::null(),
            &mut image_memory,
        ))?;
        check(bind_image(d, image, image_memory, 0))?;
        let id = ImageDesc {
            header: Record::new::<ImageDesc>(IMAGE_DESC),
            format: RGBA8_UNORM,
            dimension: IMAGE_2D,
            mip_count: 1,
            layer_count: 1,
            sample_count: 1,
            extent: Extent { x: 8, y: 8, z: 1 },
            usage: IMAGE_COPY_SRC | IMAGE_COPY_DST,
            flags: 0,
            view_formats: ptr::null(),
            view_format_count: 0,
            concurrent_domains: ptr::null(),
            concurrent_domain_count: 0,
        };
        let imported_image = gpu::image_import_vulkan(
            device,
            &id,
            &VulkanImage {
                image: image as u64,
                bound: 1,
                presentable: 0,
            },
            NATIVE_OWN,
        )?;
        assert_eq!(
            gpu::image_native_vulkan(imported_image)?.image,
            image as u64
        );
        let mut semaphore = ptr::null_mut();
        let timeline_type = vk::VkSemaphoreTypeCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_TYPE_CREATE_INFO,
            semaphoreType: vk::VkSemaphoreType_VK_SEMAPHORE_TYPE_TIMELINE,
            ..Default::default()
        };
        check(create_semaphore(
            d,
            &vk::VkSemaphoreCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO,
                pNext: ptr::from_ref(&timeline_type).cast(),
                ..Default::default()
            },
            ptr::null(),
            &mut semaphore,
        ))?;
        let imported_timeline = gpu::timeline_import_vulkan(
            device,
            &VulkanSemaphore {
                semaphore: semaphore as u64,
            },
            NATIVE_OWN,
        )?;
        let borrowed_timeline = gpu::timeline_import_vulkan(
            device,
            &gpu::timeline_native_vulkan(imported_timeline)?,
            NATIVE_BORROW,
        )?;
        assert_eq!(gpu::timeline_poll(borrowed_timeline)?, 0);
        gpu::timeline_destroy(borrowed_timeline);
        let mut binary = ptr::null_mut();
        check(create_semaphore(
            d,
            &vk::VkSemaphoreCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO,
                ..Default::default()
            },
            ptr::null(),
            &mut binary,
        ))?;
        let imported_binary = gpu::binary_import_vulkan(
            device,
            &VulkanSemaphore {
                semaphore: binary as u64,
            },
            NATIVE_OWN,
        )?;
        assert_eq!(
            gpu::binary_native_vulkan(imported_binary)?.semaphore,
            binary as u64
        );
        let mut pool = ptr::null_mut();
        check(create_pool(
            d,
            &vk::VkCommandPoolCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO,
                queueFamilyIndex: domain,
                ..Default::default()
            },
            ptr::null(),
            &mut pool,
        ))?;
        let mut command = ptr::null_mut();
        check(allocate_commands(
            d,
            &vk::VkCommandBufferAllocateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
                commandPool: pool,
                level: vk::VkCommandBufferLevel_VK_COMMAND_BUFFER_LEVEL_PRIMARY,
                commandBufferCount: 1,
                ..Default::default()
            },
            &mut command,
        ))?;
        check(begin(
            command,
            &vk::VkCommandBufferBeginInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,
                ..Default::default()
            },
        ))?;
        fill(command, buffer, 64, 128, 0x1234_abcd);
        let range = vk::VkImageSubresourceRange {
            aspectMask: vk::VkImageAspectFlagBits_VK_IMAGE_ASPECT_COLOR_BIT,
            baseMipLevel: 0,
            levelCount: 1,
            baseArrayLayer: 0,
            layerCount: 1,
        };
        let mut ib = vk::VkImageMemoryBarrier2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER_2,
            dstStageMask: vk::VK_PIPELINE_STAGE_2_ALL_TRANSFER_BIT,
            dstAccessMask: vk::VK_ACCESS_2_TRANSFER_WRITE_BIT,
            oldLayout: vk::VkImageLayout_VK_IMAGE_LAYOUT_UNDEFINED,
            newLayout: vk::VkImageLayout_VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
            srcQueueFamilyIndex: vk::VK_QUEUE_FAMILY_IGNORED as u32,
            dstQueueFamilyIndex: vk::VK_QUEUE_FAMILY_IGNORED as u32,
            image,
            subresourceRange: range,
            ..Default::default()
        };
        let mut dep = vk::VkDependencyInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEPENDENCY_INFO,
            imageMemoryBarrierCount: 1,
            pImageMemoryBarriers: &ib,
            ..Default::default()
        };
        barrier(command, &dep);
        clear(
            command,
            image,
            vk::VkImageLayout_VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
            &vk::VkClearColorValue {
                float32: [0.0, 1.0, 0.0, 1.0],
            },
            1,
            &range,
        );
        ib.srcStageMask = vk::VK_PIPELINE_STAGE_2_ALL_TRANSFER_BIT;
        ib.srcAccessMask = vk::VK_ACCESS_2_TRANSFER_WRITE_BIT;
        ib.dstAccessMask = vk::VK_ACCESS_2_TRANSFER_READ_BIT;
        ib.oldLayout = vk::VkImageLayout_VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL;
        ib.newLayout = vk::VkImageLayout_VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL;
        dep.pImageMemoryBarriers = &ib;
        barrier(command, &dep);
        check(end(command))?;
        let native_signals = [
            vk::VkSemaphoreSubmitInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_SUBMIT_INFO,
                semaphore,
                value: 1,
                stageMask: vk::VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
                ..Default::default()
            },
            vk::VkSemaphoreSubmitInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_SUBMIT_INFO,
                semaphore: binary,
                stageMask: vk::VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
                ..Default::default()
            },
        ];
        let cb = vk::VkCommandBufferSubmitInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_BUFFER_SUBMIT_INFO,
            commandBuffer: command,
            ..Default::default()
        };
        check(submit(
            nq.queue.cast(),
            1,
            &vk::VkSubmitInfo2 {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SUBMIT_INFO_2,
                commandBufferInfoCount: 1,
                pCommandBufferInfos: &cb,
                signalSemaphoreInfoCount: 2,
                pSignalSemaphoreInfos: native_signals.as_ptr(),
                ..Default::default()
            },
            ptr::null_mut(),
        ))?;
        // Final readback is for verification only; the producer-to-consumer edge
        // above/below contains GPU data and explicit native semaphore dependencies.
        let mut out_desc = MemoryDesc {
            size: 1024,
            memory_type: 0,
            ..md
        };
        let mut req = Requirements::default();
        gpu::memory_requirements(device, &out_desc, &mut req)?;
        let mut types = vec![0; req.compatible_type_count as usize];
        req.compatible_type_capacity = types.len() as u32;
        req.compatible_types = types.as_mut_ptr();
        gpu::memory_requirements(device, &out_desc, &mut req)?;
        out_desc.memory_type = gpu::adapter_memory_types(adapter)
            .iter()
            .find(|m| m.properties & MEMORY_HOST_VISIBLE != 0 && types.contains(&m.id))
            .ok_or(UNSUPPORTED)?
            .id;
        let output = gpu::memory_create(device, &out_desc)?;
        let mapping = gpu::memory_map(output.span(0, 1024))?;
        ptr::write_bytes(mapping.data, 0xa5, 1024);
        gpu::memory_flush(output.span(0, 1024))?;
        let arena = gpu::arena_create(
            device,
            &ArenaDesc {
                header: Record::new::<ArenaDesc>(ARENA_DESC),
                domain,
                list_capacity: 1,
                level: 0,
                reserved: 0,
            },
        )?;
        let list = gpu::commands_begin(
            arena,
            &RecordingDesc {
                header: Record::new::<RecordingDesc>(RECORDING_DESC),
                replay_mode: 1,
                level: 0,
                inheritance: ptr::null(),
            },
        )?;
        gpu::copy_memory(list, output.span(0, 128), buffer_only.span(0, 128));
        gpu::copy_from_image(
            list,
            output.span(256, 256),
            imported_image,
            &ImageCopy {
                region: ImageRegion {
                    aspect: ASPECT_COLOR,
                    mip: 0,
                    first_layer: 0,
                    layer_count: 1,
                    offset: Offset { x: 0, y: 0, z: 0 },
                    extent: id.extent,
                },
                row_pitch: 0,
                slice_pitch: 0,
                state: STATE_COPY_SRC,
                reserved: 0,
            },
        );
        let mut barrier_scratch = [0u64; 128];
        let scope = BarrierScope {
            stages_before: STAGE_TRANSFER,
            stages_after: STAGE_HOST,
            access_before: ACCESS_COPY_WRITE,
            access_after: ACCESS_HOST_READ,
        };
        gpu::barrier(
            list,
            &Dependency {
                global_count: 1,
                global: &scope,
                ..Default::default()
            },
            HostSpan {
                data: barrier_scratch.as_mut_ptr().cast(),
                size: size_of_val(&barrier_scratch) as u64,
            },
        );
        let list = gpu::commands_end(list)?;
        let wait = SyncPoint {
            point: Point {
                timeline: imported_timeline.as_raw(),
                value: 1,
            },
            stages: STAGE_TRANSFER,
        };
        let signal = SyncPoint {
            point: Point {
                timeline: imported_timeline.as_raw(),
                value: 2,
            },
            stages: STAGE_ALL,
        };
        let binary_wait = BinaryPoint {
            binary: imported_binary.as_raw(),
            stages: STAGE_TRANSFER,
        };
        let req = gpu::submit_scratch_requirements(
            device,
            &[SubmitDesc {
                list_count: 1,
                wait_count: 1,
                signal_count: 1,
                binary_wait_count: 1,
                ..Default::default()
            }],
        )?;
        assert!(req.alignment <= 8);
        let mut scratch = vec![0u64; req.size.div_ceil(8) as usize];
        submission::one(
            queue,
            &[list],
            &[wait],
            &[signal],
            &[binary_wait],
            &[],
            HostSpan {
                data: scratch.as_mut_ptr().cast(),
                size: size_of_val(scratch.as_slice()) as u64,
            },
        )?;
        gpu::timeline_wait(imported_timeline, 2, 10_000_000_000)?;
        gpu::memory_invalidate(output.span(0, 1024))?;
        for word in 0..32 {
            assert_eq!(mapping.data.cast::<u32>().add(word).read(), 0x1234_abcd);
        }
        for pixel in 0..64 {
            assert_eq!(
                mapping.data.cast::<u32>().add(64 + pixel).read(),
                0xff00_ff00
            );
        }
        assert_eq!(mapping.data.cast::<u8>().add(128).read(), 0xa5);
        assert_eq!(mapping.data.cast::<u8>().add(512).read(), 0xa5);
        // Known completion: release command pools before the resources they name.
        gpu::arena_destroy(arena);
        destroy_pool(d, pool, ptr::null());
        gpu::memory_destroy(buffer_only);
        gpu::memory_destroy(imported_memory);
        gpu::image_destroy(imported_image);
        free(d, image_memory, ptr::null());
        gpu::binary_destroy(imported_binary);
        gpu::timeline_destroy(imported_timeline);
        gpu::memory_destroy(output);
        gpu::device_destroy(device);
        // Borrowed wrapper destruction did not destroy the real native device.
        assert_eq!(gpu::device_native_vulkan(native_owner)?.device, nd.device);
        let still_live = gpu::timeline_create(native_owner, 7)?;
        assert_eq!(gpu::timeline_poll(still_live)?, 7);
        gpu::timeline_destroy(still_live);
        gpu::device_destroy(native_owner);
        let name = CStr::from_ptr(gpu::adapter_info(adapter).name.as_ptr());
        println!("NATIVE_INTEROP_PASS device={} buffer_words=32 image_pixels=64 buffer_only=1 separate_origins=1 borrowed_device=1 ownership=explicit timeline_binary_handoff=1", name.to_string_lossy());
    }
    Ok(())
}
#[allow(dead_code)]
fn main() {
    if let Err(status) = run() {
        eprintln!("NATIVE_INTEROP_FAIL status={status}");
        std::process::exit(if status == UNSUPPORTED { 77 } else { 1 });
    }
}
