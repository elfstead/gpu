//! Direct Vulkan reduction of the combined preprocessing/ray-buffer observation.
//! OGPU supplies device discovery/bootstrap only; all GPU resources, pipeline,
//! commands and synchronization below use native Vulkan entry points.
use ogpu::foundation::{rust as gpu, types::*};
use ogpu_vulkan_sys as vk;
use std::ptr;
#[allow(dead_code)]
#[path = "../native_support.rs"]
mod native_support;
use native_support::{check, Loader};
#[path = "../ray_native_scene.rs"]
mod scene;

pub fn run(mode: &str, scene: bool, forced_type: Option<u32>) -> gpu::Result<()> {
    if !matches!(mode, "plain" | "flags2" | "combined") {
        return Err(INVALID);
    }
    let discovery = gpu::discover()?;
    let requested = [
        feature_ids::ACCELERATION_STRUCTURE,
        feature_ids::RAY_PIPELINE,
        feature_ids::GENERATED_COMMANDS,
    ];
    let adapter = gpu::adapters(&discovery)
        .iter()
        .find(|a| {
            gpu::adapter_features(a).baseline_supported != 0
                && requested
                    .iter()
                    .all(|f| gpu::adapter_features(a).available.contains(f))
        })
        .ok_or(UNSUPPORTED)?;
    let domain = gpu::adapter_queues(adapter)
        .iter()
        .find(|q| q.count > 0 && q.flags & COMPUTE != 0)
        .ok_or(UNSUPPORTED)?
        .domain;
    let owner = gpu::device_create(
        adapter,
        &[QueueRequest {
            domain,
            count: 1,
            priority: 1.0,
        }],
        &requested,
    )?;
    unsafe {
        let native = gpu::device_native_vulkan(owner)?;
        let d = native.device.cast();
        let q = gpu::queue_native_vulkan(gpu::device_queue(owner, domain, 0).ok_or(INVALID)?)?
            .queue
            .cast();
        let loader = Loader::new(native.instance)?;
        macro_rules! load {
            ($n:ident, $t:ident) => {{
                let f: vk::$t = std::mem::transmute(loader.proc(
                    std::ffi::CStr::from_bytes_with_nul_unchecked(
                        concat!(stringify!($n), "\0").as_bytes(),
                    ),
                ));
                f.ok_or(UNSUPPORTED)?
            }};
        }
        let create_buffer = load!(vkCreateBuffer, PFN_vkCreateBuffer);
        let requirements = load!(
            vkGetBufferMemoryRequirements,
            PFN_vkGetBufferMemoryRequirements
        );
        let allocate = load!(vkAllocateMemory, PFN_vkAllocateMemory);
        let bind = load!(vkBindBufferMemory, PFN_vkBindBufferMemory);
        let address = load!(vkGetBufferDeviceAddress, PFN_vkGetBufferDeviceAddress);
        let map = load!(vkMapMemory, PFN_vkMapMemory);
        let flush = load!(vkFlushMappedMemoryRanges, PFN_vkFlushMappedMemoryRanges);
        let invalidate = load!(
            vkInvalidateMappedMemoryRanges,
            PFN_vkInvalidateMappedMemoryRanges
        );
        let create_shader = load!(vkCreateShaderModule, PFN_vkCreateShaderModule);
        let create_pipeline = load!(
            vkCreateRayTracingPipelinesKHR,
            PFN_vkCreateRayTracingPipelinesKHR
        );
        let handles = load!(
            vkGetRayTracingShaderGroupHandlesKHR,
            PFN_vkGetRayTracingShaderGroupHandlesKHR
        );
        let create_pool = load!(vkCreateCommandPool, PFN_vkCreateCommandPool);
        let commands = load!(vkAllocateCommandBuffers, PFN_vkAllocateCommandBuffers);
        let begin = load!(vkBeginCommandBuffer, PFN_vkBeginCommandBuffer);
        let end = load!(vkEndCommandBuffer, PFN_vkEndCommandBuffer);
        let bind_pipeline = load!(vkCmdBindPipeline, PFN_vkCmdBindPipeline);
        let push = load!(vkCmdPushDataEXT, PFN_vkCmdPushDataEXT);
        let trace = load!(vkCmdTraceRaysKHR, PFN_vkCmdTraceRaysKHR);
        let barrier = load!(vkCmdPipelineBarrier2, PFN_vkCmdPipelineBarrier2);
        let create_semaphore = load!(vkCreateSemaphore, PFN_vkCreateSemaphore);
        let submit = load!(vkQueueSubmit2, PFN_vkQueueSubmit2);
        let wait = load!(vkWaitSemaphores, PFN_vkWaitSemaphores);
        let mut limits = RayLimits::default();
        gpu::device_query(
            owner,
            &mut Query {
                header: Record::new::<Query>(RAY_LIMITS),
                data: ptr::from_mut(&mut limits).cast(),
                capacity: 1,
                count: 0,
                element_size: size_of::<RayLimits>() as u32,
                reserved: 0,
            },
        )?;
        let usage = vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_TRANSFER_SRC_BIT
            | vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_TRANSFER_DST_BIT
            | vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_STORAGE_BUFFER_BIT
            | vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_INDIRECT_BUFFER_BIT
            | vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_SHADER_DEVICE_ADDRESS_BIT
            | vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_BIT_KHR
            | vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_ACCELERATION_STRUCTURE_STORAGE_BIT_KHR
            | vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_SHADER_BINDING_TABLE_BIT_KHR;
        let usage2 = vk::VkBufferUsageFlags2CreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BUFFER_USAGE_FLAGS_2_CREATE_INFO,
            usage: u64::from(usage)
                | if mode == "combined" {
                    vk::VK_BUFFER_USAGE_2_PREPROCESS_BUFFER_BIT_EXT
                } else {
                    0
                },
            ..Default::default()
        };
        let mut buffer = ptr::null_mut();
        check(create_buffer(
            d,
            &vk::VkBufferCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO,
                pNext: if mode == "plain" {
                    ptr::null()
                } else {
                    ptr::from_ref(&usage2).cast()
                },
                size: 1 << 20,
                usage: if mode == "plain" { usage } else { 0 },
                ..Default::default()
            },
            ptr::null(),
            &mut buffer,
        ))?;
        let mut req = vk::VkMemoryRequirements::default();
        requirements(d, buffer, &mut req);
        let memory_type = gpu::adapter_memory_types(adapter)
            .iter()
            .find(|m| {
                forced_type.is_none_or(|id| id == m.id)
                    && req.memoryTypeBits & (1 << m.id) != 0
                    && m.properties & MEMORY_HOST_VISIBLE != 0
                    && m.properties & (16 | 32 | 64 | 128) == 0
            })
            .ok_or(UNSUPPORTED)?
            .id;
        let selected = gpu::adapter_memory_types(adapter)
            .iter()
            .find(|m| m.id == memory_type)
            .unwrap();
        println!("RAY_NATIVE_MEMORY mode={mode} scene={scene} type_bits={:#x} selected={} properties={:#x} heap={} forced={forced_type:?}", req.memoryTypeBits, memory_type, selected.properties, selected.heap);
        let dedicated = vk::VkMemoryDedicatedAllocateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_DEDICATED_ALLOCATE_INFO,
            buffer,
            ..Default::default()
        };
        let flags = vk::VkMemoryAllocateFlagsInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_FLAGS_INFO,
            pNext: ptr::from_ref(&dedicated).cast(),
            flags: vk::VkMemoryAllocateFlagBits_VK_MEMORY_ALLOCATE_DEVICE_ADDRESS_BIT,
            ..Default::default()
        };
        let mut memory = ptr::null_mut();
        check(allocate(
            d,
            &vk::VkMemoryAllocateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
                pNext: ptr::from_ref(&flags).cast(),
                allocationSize: req.size,
                memoryTypeIndex: memory_type,
            },
            ptr::null(),
            &mut memory,
        ))?;
        check(bind(d, buffer, memory, 0))?;
        let base = address(
            d,
            &vk::VkBufferDeviceAddressInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BUFFER_DEVICE_ADDRESS_INFO,
                buffer,
                ..Default::default()
            },
        );
        println!("RAY_NATIVE_ADDRESS={base:#x}");
        let mut mapped = ptr::null_mut();
        check(map(d, memory, 0, req.size, 0, &mut mapped))?;
        ptr::write_bytes(mapped.cast::<u8>(), 0xcd, req.size as usize);
        let sources: Vec<(&[u8], u32, &std::ffi::CStr)> = if scene {
            vec![
                (
                    include_bytes!("../../../shaders/foundation-ray-pipeline.rgen.spv"),
                    vk::VkShaderStageFlagBits_VK_SHADER_STAGE_RAYGEN_BIT_KHR,
                    c"rayMain",
                ),
                (
                    include_bytes!("../../../shaders/foundation-ray-pipeline.rmiss.spv"),
                    vk::VkShaderStageFlagBits_VK_SHADER_STAGE_MISS_BIT_KHR,
                    c"missMain",
                ),
                (
                    include_bytes!("../../../shaders/foundation-ray-pipeline.rchit.spv"),
                    vk::VkShaderStageFlagBits_VK_SHADER_STAGE_CLOSEST_HIT_BIT_KHR,
                    c"hitMain",
                ),
                (
                    include_bytes!("../../../shaders/foundation-ray-pipeline.rcall.spv"),
                    vk::VkShaderStageFlagBits_VK_SHADER_STAGE_CALLABLE_BIT_KHR,
                    c"initialize",
                ),
            ]
        } else {
            vec![(
                include_bytes!("../../../shaders/foundation-ray-native.rgen.spv"),
                vk::VkShaderStageFlagBits_VK_SHADER_STAGE_RAYGEN_BIT_KHR,
                c"rayMain",
            )]
        };
        let mut modules = Vec::new();
        let mut stages = Vec::new();
        let mut groups = Vec::new();
        let mapping = vk::VkShaderDescriptorSetAndBindingMappingInfoEXT { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SHADER_DESCRIPTOR_SET_AND_BINDING_MAPPING_INFO_EXT, ..Default::default() };
        for (i, (bytes, stage, entry)) in sources.iter().enumerate() {
            let code: Vec<u32> = bytes
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .collect();
            let mut module = ptr::null_mut();
            check(create_shader(
                d,
                &vk::VkShaderModuleCreateInfo {
                    sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO,
                    codeSize: bytes.len(),
                    pCode: code.as_ptr(),
                    ..Default::default()
                },
                ptr::null(),
                &mut module,
            ))?;
            modules.push(module);
            stages.push(vk::VkPipelineShaderStageCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
                pNext: ptr::from_ref(&mapping).cast(),
                stage: *stage,
                module,
                pName: entry.as_ptr(),
                ..Default::default()
            });
            let hit = scene && i == 2;
            groups.push(vk::VkRayTracingShaderGroupCreateInfoKHR { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_RAY_TRACING_SHADER_GROUP_CREATE_INFO_KHR,
                type_: if hit { vk::VkRayTracingShaderGroupTypeKHR_VK_RAY_TRACING_SHADER_GROUP_TYPE_TRIANGLES_HIT_GROUP_KHR } else { vk::VkRayTracingShaderGroupTypeKHR_VK_RAY_TRACING_SHADER_GROUP_TYPE_GENERAL_KHR },
                generalShader: if hit { u32::MAX } else { i as u32 }, closestHitShader: if hit { i as u32 } else { u32::MAX }, anyHitShader: u32::MAX, intersectionShader: u32::MAX, ..Default::default() });
        }
        let pf = vk::VkPipelineCreateFlags2CreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_CREATE_FLAGS_2_CREATE_INFO,
            flags: vk::VK_PIPELINE_CREATE_2_DESCRIPTOR_HEAP_BIT_EXT,
            ..Default::default()
        };
        let mut pipeline = ptr::null_mut();
        check(create_pipeline(
            d,
            ptr::null_mut(),
            ptr::null_mut(),
            1,
            &vk::VkRayTracingPipelineCreateInfoKHR {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_RAY_TRACING_PIPELINE_CREATE_INFO_KHR,
                pNext: ptr::from_ref(&pf).cast(),
                stageCount: stages.len() as u32,
                pStages: stages.as_ptr(),
                groupCount: groups.len() as u32,
                pGroups: groups.as_ptr(),
                maxPipelineRayRecursionDepth: 1,
                basePipelineIndex: -1,
                ..Default::default()
            },
            ptr::null(),
            &mut pipeline,
        ))?;
        let handle = u64::from(limits.group_handle_size);
        let stride = (handle + 4).div_ceil(u64::from(limits.group_handle_alignment))
            * u64::from(limits.group_handle_alignment);
        let spacing =
            stride.div_ceil(u64::from(limits.table_alignment)) * u64::from(limits.table_alignment);
        assert!(spacing * 4 <= 4096);
        assert_eq!((base + 4096) % u64::from(limits.table_alignment), 0);
        for i in 0..groups.len() {
            check(handles(
                d,
                pipeline,
                i as u32,
                1,
                handle as usize,
                mapped.cast::<u8>().add(4096 + i * spacing as usize).cast(),
            ))?;
        }
        if scene {
            mapped
                .cast::<u8>()
                .add((4096 + 2 * spacing + handle) as usize)
                .cast::<f32>()
                .write(0.125);
        }
        let cache = vk::VkMappedMemoryRange {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MAPPED_MEMORY_RANGE,
            memory,
            offset: 0,
            size: u64::MAX,
            ..Default::default()
        };
        check(flush(d, 1, &cache))?;
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
        check(commands(
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
        let scene_data = if scene {
            Some(scene::Scene::record(
                &loader,
                d,
                command,
                base,
                mapped.cast(),
                limits.scratch_alignment,
            )?)
        } else {
            None
        };
        bind_pipeline(
            command,
            vk::VkPipelineBindPoint_VK_PIPELINE_BIND_POINT_RAY_TRACING_KHR,
            pipeline,
        );
        let output = base + 8192;
        let control = [scene_data.as_ref().map_or(0, |s| s.address), output, 0];
        let (data, size) = if scene {
            (control.as_ptr().cast(), 24)
        } else {
            (ptr::from_ref(&output).cast(), 8)
        };
        push(
            command,
            &vk::VkPushDataInfoEXT {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PUSH_DATA_INFO_EXT,
                offset: 0,
                data: vk::VkHostAddressRangeConstEXT {
                    address: data,
                    size,
                },
                ..Default::default()
            },
        );
        let table = |i| vk::VkStridedDeviceAddressRegionKHR {
            deviceAddress: base + 4096 + i * spacing,
            stride,
            size: stride,
        };
        let empty = vk::VkStridedDeviceAddressRegionKHR::default();
        let hit = vk::VkStridedDeviceAddressRegionKHR {
            stride: 0,
            ..table(2)
        };
        trace(
            command,
            &table(0),
            if scene { &table(1) } else { &empty },
            if scene { &hit } else { &empty },
            if scene { &table(3) } else { &empty },
            if scene { 4 } else { 16 },
            1,
            1,
        );
        check(flush(d, 1, &cache))?;
        let edge = vk::VkMemoryBarrier2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_BARRIER_2,
            srcStageMask: vk::VK_PIPELINE_STAGE_2_RAY_TRACING_SHADER_BIT_KHR,
            srcAccessMask: vk::VK_ACCESS_2_SHADER_WRITE_BIT,
            dstStageMask: vk::VK_PIPELINE_STAGE_2_HOST_BIT,
            dstAccessMask: vk::VK_ACCESS_2_HOST_READ_BIT,
            ..Default::default()
        };
        barrier(
            command,
            &vk::VkDependencyInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEPENDENCY_INFO,
                memoryBarrierCount: 1,
                pMemoryBarriers: &edge,
                ..Default::default()
            },
        );
        check(end(command))?;
        let ti = vk::VkSemaphoreTypeCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_TYPE_CREATE_INFO,
            semaphoreType: vk::VkSemaphoreType_VK_SEMAPHORE_TYPE_TIMELINE,
            initialValue: 0,
            ..Default::default()
        };
        let mut timeline = ptr::null_mut();
        check(create_semaphore(
            d,
            &vk::VkSemaphoreCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO,
                pNext: ptr::from_ref(&ti).cast(),
                ..Default::default()
            },
            ptr::null(),
            &mut timeline,
        ))?;
        let cmd = vk::VkCommandBufferSubmitInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_BUFFER_SUBMIT_INFO,
            commandBuffer: command,
            ..Default::default()
        };
        let signal = vk::VkSemaphoreSubmitInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_SUBMIT_INFO,
            semaphore: timeline,
            value: 1,
            stageMask: vk::VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
            ..Default::default()
        };
        check(submit(
            q,
            1,
            &vk::VkSubmitInfo2 {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SUBMIT_INFO_2,
                commandBufferInfoCount: 1,
                pCommandBufferInfos: &cmd,
                signalSemaphoreInfoCount: 1,
                pSignalSemaphoreInfos: &signal,
                ..Default::default()
            },
            ptr::null_mut(),
        ))?;
        let value = 1;
        check(wait(
            d,
            &vk::VkSemaphoreWaitInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_WAIT_INFO,
                semaphoreCount: 1,
                pSemaphores: &timeline,
                pValues: &value,
                ..Default::default()
            },
            30_000_000_000,
        ))?;
        check(invalidate(d, 1, &cache))?;
        let output = std::slice::from_raw_parts(mapped.cast::<u8>().add(8192).cast::<u32>(), 16);
        println!("RAY_NATIVE mode={mode} scene={scene} usage={:#x} allocation={} memory_type={} output={output:08x?}", usage2.usage, req.size, memory_type);
        let expected: [u32; 16] = [
            1,
            1.125f32.to_bits(),
            37,
            0,
            0,
            (-1f32).to_bits(),
            u32::MAX,
            u32::MAX,
            0,
            (-1f32).to_bits(),
            u32::MAX,
            u32::MAX,
            1,
            1.125f32.to_bits(),
            37,
            1 << 16,
        ];
        let good = if scene {
            output == expected
        } else {
            output
                .iter()
                .enumerate()
                .all(|(i, &v)| v == 0x12340000 + i as u32)
        };
        let good = good
            && mapped.cast::<u8>().add(8191).read() == 0xcd
            && mapped.cast::<u8>().add(8256).read() == 0xcd;
        if let Some(s) = scene_data {
            s.destroy(&loader, d);
        }
        load!(vkDestroyCommandPool, PFN_vkDestroyCommandPool)(d, pool, ptr::null());
        load!(vkDestroySemaphore, PFN_vkDestroySemaphore)(d, timeline, ptr::null());
        load!(vkDestroyPipeline, PFN_vkDestroyPipeline)(d, pipeline, ptr::null());
        for module in modules {
            load!(vkDestroyShaderModule, PFN_vkDestroyShaderModule)(d, module, ptr::null());
        }
        load!(vkUnmapMemory, PFN_vkUnmapMemory)(d, memory);
        load!(vkDestroyBuffer, PFN_vkDestroyBuffer)(d, buffer, ptr::null());
        load!(vkFreeMemory, PFN_vkFreeMemory)(d, memory, ptr::null());
        gpu::device_destroy(owner);
        if !good {
            return Err(BACKEND_ERROR);
        }
    }
    println!("RAY_NATIVE_PASS mode={mode} scene={scene}");
    Ok(())
}
fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "plain".into());
    if let Err(status) = run(
        &mode,
        std::env::args().any(|a| a == "--scene"),
        std::env::args().find_map(|a| {
            a.strip_prefix("--memory-type=")
                .map(|s| s.parse().expect("memory type integer"))
        }),
    ) {
        eprintln!("RAY_NATIVE_STATUS={status}");
        std::process::exit(if status == UNSUPPORTED { 77 } else { 1 });
    }
}
