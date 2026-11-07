//! Linux OPAQUE_FD memory/timeline composition across two native logical devices.
use ogpu::foundation::{rust as gpu, types::*};
use ogpu_vulkan_sys as vk;
use std::{
    os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd},
    ptr,
};
#[path = "../native_support.rs"]
mod native_support;
#[path = "../submission.rs"]
mod submission;
use native_support::{check, Loader};
macro_rules! function {
    ($loader:expr, $name:ident, $ty:ident) => {{
        let f: vk::$ty = std::mem::transmute(
            $loader.proc(
                std::ffi::CStr::from_bytes_with_nul(concat!(stringify!($name), "\0").as_bytes())
                    .unwrap(),
            ),
        );
        f.ok_or(UNSUPPORTED)?
    }};
}
unsafe fn readback(
    device: gpu::DeviceHandle,
    adapter: &ogpu::foundation::Adapter,
) -> gpu::Result<(gpu::MemoryHandle, Mapping)> {
    unsafe {
        let mut desc = MemoryDesc {
            header: Record::new::<MemoryDesc>(MEMORY_DESC),
            size: 512,
            alignment: 4,
            usage: USAGE_COPY_DST,
            memory_type: 0,
            kind: MEMORY_LINEAR,
            concurrent_domains: ptr::null(),
            concurrent_domain_count: 0,
            flags: 0,
        };
        let mut req = Requirements::default();
        gpu::memory_requirements(device, &desc, &mut req)?;
        let mut types = vec![0; req.compatible_type_count as usize];
        req.compatible_type_capacity = types.len() as u32;
        req.compatible_types = types.as_mut_ptr();
        gpu::memory_requirements(device, &desc, &mut req)?;
        desc.memory_type = gpu::adapter_memory_types(adapter)
            .iter()
            .find(|m| m.properties & MEMORY_HOST_VISIBLE != 0 && types.contains(&m.id))
            .ok_or(UNSUPPORTED)?
            .id;
        let memory = gpu::memory_create(device, &desc)?;
        let mapping = gpu::memory_map(memory.span(0, 512))?;
        ptr::write_bytes(mapping.data, 0xa5, 512);
        gpu::memory_flush(memory.span(0, 512))?;
        Ok((memory, mapping))
    }
}
pub fn run() -> gpu::Result<()> {
    // Diagnostic control only: current SyncVal cannot follow the external
    // timeline signal through another device. The default keeps native scopes.
    let local_control = std::env::var_os("OGPU_EXTERNAL_SYNC_CONTROL").as_deref()
        == Some(std::ffi::OsStr::new("1"));
    let discovery = gpu::discover()?;
    let adapter = gpu::adapters(&discovery)
        .iter()
        .find(|a| gpu::adapter_features(a).baseline_supported != 0)
        .ok_or(UNSUPPORTED)?;
    let domain = gpu::adapter_queues(adapter)
        .iter()
        .find(|q| q.count != 0 && q.flags & COMPUTE != 0)
        .ok_or(UNSUPPORTED)?
        .domain;
    let requests = [QueueRequest {
        domain,
        count: 1,
        priority: 1.0,
    }];
    let anchor = gpu::device_create(adapter, &requests, &[])?;
    // SAFETY: exact native metadata, same physical device/driver, selected loader.
    // OS handles transfer only after native import succeeds; failed native import
    // drops the still-owned FD. No unknown pending GPU work is freed on error.
    unsafe {
        let origin = gpu::device_native_vulkan(anchor)?;
        let loader = Loader::new(origin.instance)?;
        let physical = origin.physical_device.cast();
        let properties = function!(
            loader,
            vkGetPhysicalDeviceProperties2,
            PFN_vkGetPhysicalDeviceProperties2
        );
        let mut identity = vk::VkPhysicalDeviceIDProperties {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_ID_PROPERTIES,
            ..Default::default()
        };
        let mut props = vk::VkPhysicalDeviceProperties2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2,
            pNext: ptr::from_mut(&mut identity).cast(),
            ..Default::default()
        };
        properties(physical, &mut props);
        assert!(identity.deviceUUID.iter().any(|b| *b != 0));
        let query_buffer = function!(
            loader,
            vkGetPhysicalDeviceExternalBufferProperties,
            PFN_vkGetPhysicalDeviceExternalBufferProperties
        );
        let query_semaphore = function!(
            loader,
            vkGetPhysicalDeviceExternalSemaphoreProperties,
            PFN_vkGetPhysicalDeviceExternalSemaphoreProperties
        );
        let memory_type =
            vk::VkExternalMemoryHandleTypeFlagBits_VK_EXTERNAL_MEMORY_HANDLE_TYPE_OPAQUE_FD_BIT;
        let semaphore_type = vk::VkExternalSemaphoreHandleTypeFlagBits_VK_EXTERNAL_SEMAPHORE_HANDLE_TYPE_OPAQUE_FD_BIT;
        let usage = vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_TRANSFER_SRC_BIT
            | vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_TRANSFER_DST_BIT
            | vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_SHADER_DEVICE_ADDRESS_BIT;
        let mut bp = vk::VkExternalBufferProperties {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_EXTERNAL_BUFFER_PROPERTIES,
            ..Default::default()
        };
        query_buffer(
            physical,
            &vk::VkPhysicalDeviceExternalBufferInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_EXTERNAL_BUFFER_INFO,
                usage,
                handleType: memory_type,
                ..Default::default()
            },
            &mut bp,
        );
        let mut timeline_type = vk::VkSemaphoreTypeCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_TYPE_CREATE_INFO,
            semaphoreType: vk::VkSemaphoreType_VK_SEMAPHORE_TYPE_TIMELINE,
            ..Default::default()
        };
        let mut sp = vk::VkExternalSemaphoreProperties {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_EXTERNAL_SEMAPHORE_PROPERTIES,
            ..Default::default()
        };
        query_semaphore(
            physical,
            &vk::VkPhysicalDeviceExternalSemaphoreInfo {
                sType:
                    vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_EXTERNAL_SEMAPHORE_INFO,
                pNext: ptr::from_ref(&timeline_type).cast(),
                handleType: semaphore_type,
            },
            &mut sp,
        );
        if bp.externalMemoryProperties.externalMemoryFeatures & 6 != 6
            || sp.externalSemaphoreFeatures & 3 != 3
        {
            return Err(UNSUPPORTED);
        }
        let extensions = [
            c"VK_KHR_external_memory_fd",
            c"VK_KHR_external_semaphore_fd",
        ];
        let a = loader.create_device(physical, domain, &extensions)?;
        let b = loader.create_device(physical, domain, &extensions)?;
        let owner_a = gpu::device_import_vulkan(
            adapter,
            &requests,
            &[],
            &VulkanDevice {
                device: a.cast(),
                ..origin
            },
            NATIVE_OWN,
        )?;
        let owner_b = gpu::device_import_vulkan(
            adapter,
            &requests,
            &[],
            &VulkanDevice {
                device: b.cast(),
                ..origin
            },
            NATIVE_OWN,
        )?;
        gpu::device_destroy(anchor);
        let qa = gpu::device_queue(owner_a, domain, 0).ok_or(INVALID)?;
        let qb = gpu::device_queue(owner_b, domain, 0).ok_or(INVALID)?;
        let create_buffer = function!(loader, vkCreateBuffer, PFN_vkCreateBuffer);
        let buffer_requirements = function!(
            loader,
            vkGetBufferMemoryRequirements,
            PFN_vkGetBufferMemoryRequirements
        );
        let allocate = function!(loader, vkAllocateMemory, PFN_vkAllocateMemory);
        let bind = function!(loader, vkBindBufferMemory, PFN_vkBindBufferMemory);
        let get_fd = function!(loader, vkGetMemoryFdKHR, PFN_vkGetMemoryFdKHR);
        let external_buffer = vk::VkExternalMemoryBufferCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_EXTERNAL_MEMORY_BUFFER_CREATE_INFO,
            handleTypes: memory_type,
            ..Default::default()
        };
        let buffer_desc = vk::VkBufferCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO,
            pNext: ptr::from_ref(&external_buffer).cast(),
            size: 4096,
            usage,
            ..Default::default()
        };
        let mut ba = ptr::null_mut();
        let mut bb = ptr::null_mut();
        check(create_buffer(a, &buffer_desc, ptr::null(), &mut ba))?;
        check(create_buffer(b, &buffer_desc, ptr::null(), &mut bb))?;
        let mut ra = vk::VkMemoryRequirements::default();
        let mut rb = vk::VkMemoryRequirements::default();
        buffer_requirements(a, ba, &mut ra);
        buffer_requirements(b, bb, &mut rb);
        assert_eq!(
            (ra.size, ra.alignment, ra.memoryTypeBits),
            (rb.size, rb.alignment, rb.memoryTypeBits)
        );
        let mt = gpu::adapter_memory_types(adapter)
            .iter()
            .find(|m| ra.memoryTypeBits & (1 << m.id) != 0 && m.properties & (32 | 64 | 128) == 0)
            .ok_or(UNSUPPORTED)?
            .id;
        let export = vk::VkExportMemoryAllocateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_EXPORT_MEMORY_ALLOCATE_INFO,
            handleTypes: memory_type,
            ..Default::default()
        };
        let mut dedicated_a = vk::VkMemoryDedicatedAllocateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_DEDICATED_ALLOCATE_INFO,
            pNext: ptr::from_ref(&export).cast(),
            buffer: ba,
            ..Default::default()
        };
        let mut flags = vk::VkMemoryAllocateFlagsInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_FLAGS_INFO,
            pNext: ptr::from_mut(&mut dedicated_a).cast(),
            flags: vk::VkMemoryAllocateFlagBits_VK_MEMORY_ALLOCATE_DEVICE_ADDRESS_BIT,
            ..Default::default()
        };
        let mut ma = ptr::null_mut();
        let mut mb = ptr::null_mut();
        check(allocate(
            a,
            &vk::VkMemoryAllocateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
                pNext: ptr::from_ref(&flags).cast(),
                allocationSize: ra.size,
                memoryTypeIndex: mt,
            },
            ptr::null(),
            &mut ma,
        ))?;
        check(bind(a, ba, ma, 0))?;
        let mut fd = -1;
        check(get_fd(
            a,
            &vk::VkMemoryGetFdInfoKHR {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_GET_FD_INFO_KHR,
                memory: ma,
                handleType: memory_type,
                ..Default::default()
            },
            &mut fd,
        ))?;
        let fd = OwnedFd::from_raw_fd(fd);
        // Closing another reference does not affect the exported/imported payload.
        drop(fd.try_clone().map_err(|_| BACKEND_ERROR)?);
        let import = vk::VkImportMemoryFdInfoKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_IMPORT_MEMORY_FD_INFO_KHR,
            handleType: memory_type,
            fd: fd.as_raw_fd(),
            ..Default::default()
        };
        let dedicated_b = vk::VkMemoryDedicatedAllocateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_DEDICATED_ALLOCATE_INFO,
            pNext: ptr::from_ref(&import).cast(),
            buffer: bb,
            ..Default::default()
        };
        flags.pNext = ptr::from_ref(&dedicated_b).cast();
        check(allocate(
            b,
            &vk::VkMemoryAllocateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
                pNext: ptr::from_ref(&flags).cast(),
                allocationSize: ra.size,
                memoryTypeIndex: mt,
            },
            ptr::null(),
            &mut mb,
        ))?;
        let _consumed_fd = fd.into_raw_fd(); // Vulkan consumed this ownership on success.
        check(bind(b, bb, mb, 0))?;
        let md = MemoryDesc {
            header: Record::new::<MemoryDesc>(MEMORY_DESC),
            size: 4096,
            alignment: 4,
            usage: USAGE_COPY_SRC | USAGE_COPY_DST,
            memory_type: mt,
            kind: MEMORY_LINEAR,
            concurrent_domains: ptr::null(),
            concurrent_domain_count: 0,
            flags: 0,
        };
        let wa = gpu::memory_import_vulkan(
            owner_a,
            &md,
            &VulkanMemory {
                memory: ma as u64,
                buffer: ba as u64,
                allocation_size: ra.size,
                allocation_offset: 0,
                buffer_size: 4096,
                buffer_offset: 0,
            },
            NATIVE_OWN | NATIVE_OWN_BUFFER,
        )?;
        let wb = gpu::memory_import_vulkan(
            owner_b,
            &md,
            &VulkanMemory {
                memory: mb as u64,
                buffer: bb as u64,
                allocation_size: rb.size,
                allocation_offset: 0,
                buffer_size: 4096,
                buffer_offset: 0,
            },
            NATIVE_OWN | NATIVE_OWN_BUFFER,
        )?;
        let create_semaphore = function!(loader, vkCreateSemaphore, PFN_vkCreateSemaphore);
        let export_semaphore = function!(loader, vkGetSemaphoreFdKHR, PFN_vkGetSemaphoreFdKHR);
        let import_semaphore =
            function!(loader, vkImportSemaphoreFdKHR, PFN_vkImportSemaphoreFdKHR);
        let export = vk::VkExportSemaphoreCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_EXPORT_SEMAPHORE_CREATE_INFO,
            handleTypes: semaphore_type,
            ..Default::default()
        };
        timeline_type.pNext = ptr::from_ref(&export).cast();
        let mut sa = ptr::null_mut();
        let mut sb = ptr::null_mut();
        check(create_semaphore(
            a,
            &vk::VkSemaphoreCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO,
                pNext: ptr::from_ref(&timeline_type).cast(),
                ..Default::default()
            },
            ptr::null(),
            &mut sa,
        ))?;
        timeline_type.pNext = ptr::null();
        check(create_semaphore(
            b,
            &vk::VkSemaphoreCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO,
                pNext: ptr::from_ref(&timeline_type).cast(),
                ..Default::default()
            },
            ptr::null(),
            &mut sb,
        ))?;
        let mut fd = -1;
        check(export_semaphore(
            a,
            &vk::VkSemaphoreGetFdInfoKHR {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_GET_FD_INFO_KHR,
                semaphore: sa,
                handleType: semaphore_type,
                ..Default::default()
            },
            &mut fd,
        ))?;
        let fd = OwnedFd::from_raw_fd(fd);
        check(import_semaphore(
            b,
            &vk::VkImportSemaphoreFdInfoKHR {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_IMPORT_SEMAPHORE_FD_INFO_KHR,
                semaphore: sb,
                handleType: semaphore_type,
                fd: fd.as_raw_fd(),
                ..Default::default()
            },
        ))?;
        let _consumed_fd = fd.into_raw_fd();
        let ta = gpu::timeline_import_vulkan(
            owner_a,
            &VulkanSemaphore {
                semaphore: sa as u64,
            },
            NATIVE_OWN,
        )?;
        let tb = gpu::timeline_import_vulkan(
            owner_b,
            &VulkanSemaphore {
                semaphore: sb as u64,
            },
            NATIVE_OWN,
        )?;
        let (out_a, map_a) = readback(owner_a, adapter)?;
        let (out_b, map_b) = readback(owner_b, adapter)?;
        let create_pool = function!(loader, vkCreateCommandPool, PFN_vkCreateCommandPool);
        let destroy_pool = function!(loader, vkDestroyCommandPool, PFN_vkDestroyCommandPool);
        let allocate_commands = function!(
            loader,
            vkAllocateCommandBuffers,
            PFN_vkAllocateCommandBuffers
        );
        let begin = function!(loader, vkBeginCommandBuffer, PFN_vkBeginCommandBuffer);
        let end = function!(loader, vkEndCommandBuffer, PFN_vkEndCommandBuffer);
        let fill = function!(loader, vkCmdFillBuffer, PFN_vkCmdFillBuffer);
        let copy = function!(loader, vkCmdCopyBuffer2, PFN_vkCmdCopyBuffer2);
        let barrier = function!(loader, vkCmdPipelineBarrier2, PFN_vkCmdPipelineBarrier2);
        let submit = function!(loader, vkQueueSubmit2, PFN_vkQueueSubmit2);
        let mut pool = ptr::null_mut();
        check(create_pool(
            a,
            &vk::VkCommandPoolCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO,
                queueFamilyIndex: domain,
                ..Default::default()
            },
            ptr::null(),
            &mut pool,
        ))?;
        let mut commands = [ptr::null_mut(); 2];
        check(allocate_commands(
            a,
            &vk::VkCommandBufferAllocateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
                commandPool: pool,
                level: 0,
                commandBufferCount: 2,
                ..Default::default()
            },
            commands.as_mut_ptr(),
        ))?;
        let bi = vk::VkCommandBufferBeginInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,
            ..Default::default()
        };
        let native_queue = gpu::queue_native_vulkan(qa)?.queue.cast();
        let semaphore_point = |value| vk::VkSemaphoreSubmitInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_SUBMIT_INFO,
            semaphore: sa,
            value,
            stageMask: vk::VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
            ..Default::default()
        };
        let native_barrier = |src, dst, read| vk::VkBufferMemoryBarrier2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BUFFER_MEMORY_BARRIER_2,
            srcStageMask: if read {
                0
            } else {
                vk::VK_PIPELINE_STAGE_2_ALL_TRANSFER_BIT
            },
            srcAccessMask: if read {
                0
            } else {
                vk::VK_ACCESS_2_TRANSFER_WRITE_BIT
            },
            dstStageMask: if read {
                vk::VK_PIPELINE_STAGE_2_ALL_TRANSFER_BIT
            } else {
                0
            },
            dstAccessMask: if read {
                vk::VK_ACCESS_2_TRANSFER_READ_BIT
            } else {
                0
            },
            srcQueueFamilyIndex: src,
            dstQueueFamilyIndex: dst,
            buffer: ba,
            offset: 0,
            size: 4096,
            ..Default::default()
        };
        check(begin(commands[0], &bi))?;
        fill(commands[0], ba, 0, 256, 0x1234_abcd);
        let release = native_barrier(domain, DOMAIN_EXTERNAL, false);
        barrier(
            commands[0],
            &vk::VkDependencyInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEPENDENCY_INFO,
                bufferMemoryBarrierCount: 1,
                pBufferMemoryBarriers: &release,
                ..Default::default()
            },
        );
        check(end(commands[0]))?;
        let cb = vk::VkCommandBufferSubmitInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_BUFFER_SUBMIT_INFO,
            commandBuffer: commands[0],
            ..Default::default()
        };
        let signal = semaphore_point(1);
        check(submit(
            native_queue,
            1,
            &vk::VkSubmitInfo2 {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SUBMIT_INFO_2,
                commandBufferInfoCount: 1,
                pCommandBufferInfos: &cb,
                signalSemaphoreInfoCount: 1,
                pSignalSemaphoreInfos: &signal,
                ..Default::default()
            },
            ptr::null_mut(),
        ))?;
        let arena = gpu::arena_create(
            owner_b,
            &ArenaDesc {
                header: Record::new::<ArenaDesc>(ARENA_DESC),
                domain,
                list_capacity: 1,
                level: 0,
                reserved: 0,
            },
        )?;
        let bad = gpu::commands_begin(
            arena,
            &RecordingDesc {
                header: Record::new::<RecordingDesc>(RECORDING_DESC),
                replay_mode: 1,
                level: 0,
                inheritance: ptr::null(),
            },
        )?;
        let invalid = MemoryBarrier {
            range: wb.span(0, 4096),
            source_domain: DOMAIN_EXTERNAL,
            destination_domain: DOMAIN_EXTERNAL,
            scope: BarrierScope::default(),
        };
        let mut bad_storage = [0u64; 128];
        gpu::barrier(
            bad,
            &Dependency {
                memory_count: 1,
                memory: &invalid,
                ..Default::default()
            },
            HostSpan {
                data: bad_storage.as_mut_ptr().cast(),
                size: size_of_val(&bad_storage) as u64,
            },
        );
        assert_eq!(gpu::commands_end(bad), Err(INVALID));
        gpu::arena_reset(arena)?;
        let list = gpu::commands_begin(
            arena,
            &RecordingDesc {
                header: Record::new::<RecordingDesc>(RECORDING_DESC),
                replay_mode: 1,
                level: 0,
                inheritance: ptr::null(),
            },
        )?;
        let mut scratch = [0u64; 128];
        let scratch_span = HostSpan {
            data: scratch.as_mut_ptr().cast(),
            size: size_of_val(&scratch) as u64,
        };
        let acquire = MemoryBarrier {
            range: wb.span(0, 4096),
            source_domain: DOMAIN_EXTERNAL,
            destination_domain: domain,
            scope: BarrierScope {
                stages_before: 0,
                stages_after: STAGE_TRANSFER,
                access_before: 0,
                access_after: ACCESS_COPY_READ | ACCESS_COPY_WRITE,
            },
        };
        gpu::barrier(
            list,
            &Dependency {
                memory_count: 1,
                memory: &acquire,
                ..Default::default()
            },
            scratch_span,
        );
        gpu::copy_memory(list, out_b.span(0, 256), wb.span(0, 256));
        let scope = BarrierScope {
            stages_before: STAGE_TRANSFER,
            stages_after: STAGE_TRANSFER,
            access_before: ACCESS_COPY_READ,
            access_after: ACCESS_COPY_WRITE,
        };
        gpu::barrier(
            list,
            &Dependency {
                global_count: 1,
                global: &scope,
                ..Default::default()
            },
            scratch_span,
        );
        gpu::fill_memory(list, wb.span(0, 256), 0x7654_3210);
        let release = MemoryBarrier {
            range: wb.span(0, 4096),
            source_domain: domain,
            destination_domain: DOMAIN_EXTERNAL,
            scope: BarrierScope {
                stages_before: STAGE_TRANSFER,
                stages_after: 0,
                access_before: ACCESS_COPY_WRITE,
                access_after: 0,
            },
        };
        let host = BarrierScope {
            stages_before: STAGE_TRANSFER,
            stages_after: STAGE_HOST,
            access_before: ACCESS_COPY_WRITE,
            access_after: ACCESS_HOST_READ,
        };
        gpu::barrier(
            list,
            &Dependency {
                memory_count: 1,
                memory: &release,
                global_count: 1,
                global: &host,
                ..Default::default()
            },
            scratch_span,
        );
        let list = gpu::commands_end(list)?;
        let wait = SyncPoint {
            point: Point {
                timeline: tb.as_raw(),
                value: 1,
            },
            stages: STAGE_ALL,
        };
        let signal = SyncPoint {
            point: Point {
                timeline: tb.as_raw(),
                value: 2,
            },
            stages: STAGE_ALL,
        };
        let req = gpu::submit_scratch_requirements(
            owner_b,
            &[SubmitDesc {
                list_count: 1,
                wait_count: 1,
                signal_count: 1,
                ..Default::default()
            }],
        )?;
        assert!(req.alignment <= 8);
        let mut storage = vec![0u64; req.size.div_ceil(8) as usize];
        submission::one(
            qb,
            &[list],
            &[wait],
            &[signal],
            &[],
            &[],
            HostSpan {
                data: storage.as_mut_ptr().cast(),
                size: size_of_val(storage.as_slice()) as u64,
            },
        )?;
        check(begin(commands[1], &bi))?;
        let acquire = native_barrier(DOMAIN_EXTERNAL, domain, true);
        barrier(
            commands[1],
            &vk::VkDependencyInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEPENDENCY_INFO,
                bufferMemoryBarrierCount: 1,
                pBufferMemoryBarriers: &acquire,
                ..Default::default()
            },
        );
        if local_control {
            let local = vk::VkMemoryBarrier2 {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_BARRIER_2,
                srcStageMask: vk::VK_PIPELINE_STAGE_2_ALL_TRANSFER_BIT,
                srcAccessMask: vk::VK_ACCESS_2_TRANSFER_WRITE_BIT,
                dstStageMask: vk::VK_PIPELINE_STAGE_2_ALL_TRANSFER_BIT,
                dstAccessMask: vk::VK_ACCESS_2_TRANSFER_READ_BIT,
                ..Default::default()
            };
            barrier(
                commands[1],
                &vk::VkDependencyInfo {
                    sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEPENDENCY_INFO,
                    memoryBarrierCount: 1,
                    pMemoryBarriers: &local,
                    ..Default::default()
                },
            );
        }
        let output_native = gpu::memory_native_vulkan(out_a)?;
        let region = vk::VkBufferCopy2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BUFFER_COPY_2,
            srcOffset: 0,
            dstOffset: output_native.buffer_offset,
            size: 256,
            ..Default::default()
        };
        copy(
            commands[1],
            &vk::VkCopyBufferInfo2 {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COPY_BUFFER_INFO_2,
                srcBuffer: ba,
                dstBuffer: output_native.buffer as vk::VkBuffer,
                regionCount: 1,
                pRegions: &region,
                ..Default::default()
            },
        );
        let host = vk::VkMemoryBarrier2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_BARRIER_2,
            srcStageMask: vk::VK_PIPELINE_STAGE_2_ALL_TRANSFER_BIT,
            srcAccessMask: vk::VK_ACCESS_2_TRANSFER_WRITE_BIT,
            dstStageMask: vk::VK_PIPELINE_STAGE_2_HOST_BIT,
            dstAccessMask: vk::VK_ACCESS_2_HOST_READ_BIT,
            ..Default::default()
        };
        barrier(
            commands[1],
            &vk::VkDependencyInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEPENDENCY_INFO,
                memoryBarrierCount: 1,
                pMemoryBarriers: &host,
                ..Default::default()
            },
        );
        check(end(commands[1]))?;
        let cb = vk::VkCommandBufferSubmitInfo {
            commandBuffer: commands[1],
            ..cb
        };
        let wait = semaphore_point(2);
        let signal = semaphore_point(3);
        check(submit(
            native_queue,
            1,
            &vk::VkSubmitInfo2 {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SUBMIT_INFO_2,
                commandBufferInfoCount: 1,
                pCommandBufferInfos: &cb,
                waitSemaphoreInfoCount: 1,
                pWaitSemaphoreInfos: &wait,
                signalSemaphoreInfoCount: 1,
                pSignalSemaphoreInfos: &signal,
                ..Default::default()
            },
            ptr::null_mut(),
        ))?;
        gpu::timeline_wait(ta, 3, 10_000_000_000)?;
        // Retire each logical device explicitly before native teardown. This also
        // lets core validation retire the imported semaphore's own submit state.
        gpu::timeline_wait(tb, 2, 10_000_000_000)?;
        gpu::memory_invalidate(out_a.span(0, 512))?;
        gpu::memory_invalidate(out_b.span(0, 512))?;
        for i in 0..64 {
            assert_eq!(map_a.data.cast::<u32>().add(i).read(), 0x7654_3210);
            assert_eq!(map_b.data.cast::<u32>().add(i).read(), 0x1234_abcd);
        }
        assert_eq!(map_a.data.cast::<u32>().add(64).read(), 0xa5a5_a5a5);
        assert_eq!(map_b.data.cast::<u32>().add(64).read(), 0xa5a5_a5a5);
        gpu::arena_destroy(arena);
        destroy_pool(a, pool, ptr::null());
        gpu::memory_destroy(wa);
        gpu::memory_destroy(wb);
        gpu::memory_destroy(out_a);
        gpu::memory_destroy(out_b);
        gpu::timeline_destroy(ta);
        gpu::timeline_destroy(tb);
        gpu::device_destroy(owner_a);
        gpu::device_destroy(owner_b);
        println!("EXTERNAL_FD_PASS local_sync_control={local_control} logical_devices=2 physical_devices=1 memory_payloads=1 timeline_payloads=1 ownership_transfers=4 checked_words=128 device_uuid={:02x?} driver_uuid={:02x?}", identity.deviceUUID, identity.driverUUID);
    }
    Ok(())
}
#[allow(dead_code)]
fn main() {
    if let Err(status) = run() {
        eprintln!("EXTERNAL_FD_FAIL status={status}");
        std::process::exit(if status == UNSUPPORTED { 77 } else { 1 });
    }
}
