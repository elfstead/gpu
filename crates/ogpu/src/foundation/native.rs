//! Vulkan foundation: immutable device state, externally synchronized queues,
//! independent timelines. No implicit queue, completion counter or resource cache.
use super::types::*;
use super::{loader_error, Adapter, Snapshot};
use crate::vulkan::Instance;
use ogpu_vulkan_sys as vk;
use std::{
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicPtr, Ordering},
        Arc,
    },
};

#[path = "memory.rs"]
mod memory;
pub use memory::Memory;
#[path = "commands.rs"]
mod commands;
pub(super) use commands::{barrier_scratch, submit_scratch, vertex_scratch};
pub use commands::{Arena, List};
#[path = "images.rs"]
mod images;
pub use images::{Image, View};
#[path = "descriptors.rs"]
mod descriptors;
pub(super) use descriptors::descriptor_scratch;
#[path = "executables.rs"]
mod executables;
pub(super) use executables::record as executable_record;
#[path = "rendering.rs"]
mod rendering;
pub use executables::Executable;
#[path = "executable_cache.rs"]
mod executable_cache;
pub use executable_cache::ExecutableCache;
pub(super) use rendering::render_scratch;
use rendering::samples;
#[path = "queries.rs"]
mod queries;
pub use queries::QueryPool;

macro_rules! functions {
    ($($name:ident: $ty:ident),* $(,)?) => {
        #[allow(non_snake_case)]
        struct Functions { $($name: vk::$ty,)* }
        impl Functions {
            #[allow(non_snake_case)]
            fn load(instance: &Instance) -> Result<Self, Status> {
                $(
                    // SAFETY: each native name is paired with its generated PFN.
                    let $name: vk::$ty = unsafe { std::mem::transmute(instance.proc(
                        std::ffi::CStr::from_bytes_with_nul(concat!(stringify!($name), "\0").as_bytes()).unwrap()
                    )) };
                    if $name.is_none() { return Err(BACKEND_ERROR); }
                )*
                Ok(Self { $($name,)* })
            }
        }
    };
}
functions! {
    vkGetPhysicalDeviceQueueFamilyProperties: PFN_vkGetPhysicalDeviceQueueFamilyProperties,
    vkGetPhysicalDeviceMemoryProperties: PFN_vkGetPhysicalDeviceMemoryProperties,
    vkGetPhysicalDeviceFeatures2: PFN_vkGetPhysicalDeviceFeatures2,
    vkGetPhysicalDeviceProperties2: PFN_vkGetPhysicalDeviceProperties2,
    vkCreateDevice: PFN_vkCreateDevice, vkDestroyDevice: PFN_vkDestroyDevice,
    vkGetDeviceQueue: PFN_vkGetDeviceQueue,
    vkCreateSemaphore: PFN_vkCreateSemaphore, vkDestroySemaphore: PFN_vkDestroySemaphore,
    vkWaitSemaphores: PFN_vkWaitSemaphores,
    vkGetSemaphoreCounterValue: PFN_vkGetSemaphoreCounterValue,
    vkSignalSemaphore: PFN_vkSignalSemaphore,
    vkGetDeviceBufferMemoryRequirements: PFN_vkGetDeviceBufferMemoryRequirements,
    vkCreateBuffer: PFN_vkCreateBuffer, vkDestroyBuffer: PFN_vkDestroyBuffer,
    vkAllocateMemory: PFN_vkAllocateMemory, vkFreeMemory: PFN_vkFreeMemory,
    vkBindBufferMemory: PFN_vkBindBufferMemory,
    vkMapMemory: PFN_vkMapMemory, vkUnmapMemory: PFN_vkUnmapMemory,
    vkFlushMappedMemoryRanges: PFN_vkFlushMappedMemoryRanges,
    vkInvalidateMappedMemoryRanges: PFN_vkInvalidateMappedMemoryRanges,
    vkGetBufferDeviceAddress: PFN_vkGetBufferDeviceAddress,
    vkCreateCommandPool: PFN_vkCreateCommandPool,
    vkDestroyCommandPool: PFN_vkDestroyCommandPool,
    vkResetCommandPool: PFN_vkResetCommandPool,
    vkTrimCommandPool: PFN_vkTrimCommandPool,
    vkAllocateCommandBuffers: PFN_vkAllocateCommandBuffers,
    vkFreeCommandBuffers: PFN_vkFreeCommandBuffers,
    vkBeginCommandBuffer: PFN_vkBeginCommandBuffer,
    vkEndCommandBuffer: PFN_vkEndCommandBuffer,
    vkCmdCopyMemoryKHR: PFN_vkCmdCopyMemoryKHR,
    vkCmdFillBuffer: PFN_vkCmdFillBuffer,
    vkCmdPipelineBarrier2: PFN_vkCmdPipelineBarrier2,
    vkQueueSubmit2: PFN_vkQueueSubmit2,
    vkGetPhysicalDeviceImageFormatProperties: PFN_vkGetPhysicalDeviceImageFormatProperties,
    vkGetDeviceImageMemoryRequirements: PFN_vkGetDeviceImageMemoryRequirements,
    vkCreateImage: PFN_vkCreateImage, vkDestroyImage: PFN_vkDestroyImage,
    vkBindImageMemory: PFN_vkBindImageMemory,
    vkCreateImageView: PFN_vkCreateImageView, vkDestroyImageView: PFN_vkDestroyImageView,
    vkCmdCopyImageToMemoryKHR: PFN_vkCmdCopyImageToMemoryKHR,
    vkCmdCopyMemoryToImageKHR: PFN_vkCmdCopyMemoryToImageKHR,
    vkCmdClearColorImage: PFN_vkCmdClearColorImage,
    vkCmdClearDepthStencilImage: PFN_vkCmdClearDepthStencilImage,
    vkWriteResourceDescriptorsEXT: PFN_vkWriteResourceDescriptorsEXT,
    vkWriteSamplerDescriptorsEXT: PFN_vkWriteSamplerDescriptorsEXT,
    vkCmdBindResourceHeapEXT: PFN_vkCmdBindResourceHeapEXT,
    vkCmdBindSamplerHeapEXT: PFN_vkCmdBindSamplerHeapEXT,
    vkCreateShaderModule: PFN_vkCreateShaderModule,
    vkDestroyShaderModule: PFN_vkDestroyShaderModule,
    vkCreateComputePipelines: PFN_vkCreateComputePipelines,
    vkCreatePipelineCache: PFN_vkCreatePipelineCache,
    vkDestroyPipelineCache: PFN_vkDestroyPipelineCache,
    vkGetPipelineCacheData: PFN_vkGetPipelineCacheData,
    vkMergePipelineCaches: PFN_vkMergePipelineCaches,
    vkDestroyPipeline: PFN_vkDestroyPipeline,
    vkCmdBindPipeline: PFN_vkCmdBindPipeline,
    vkCmdPushDataEXT: PFN_vkCmdPushDataEXT,
    vkCmdDispatch: PFN_vkCmdDispatch,
    vkCmdDispatchIndirect: PFN_vkCmdDispatchIndirect,
    vkCreateGraphicsPipelines: PFN_vkCreateGraphicsPipelines,
    vkCmdBeginRendering: PFN_vkCmdBeginRendering,
    vkCmdEndRendering: PFN_vkCmdEndRendering,
    vkCmdSetViewport: PFN_vkCmdSetViewport,
    vkCmdSetScissor: PFN_vkCmdSetScissor,
    vkCmdDraw: PFN_vkCmdDraw,
    vkCmdDrawIndexed: PFN_vkCmdDrawIndexed,
    vkCmdBindIndexBuffer3KHR: PFN_vkCmdBindIndexBuffer3KHR,
    vkCmdBindVertexBuffers3KHR: PFN_vkCmdBindVertexBuffers3KHR,
    vkCmdDrawIndirect2KHR: PFN_vkCmdDrawIndirect2KHR,
    vkCmdDrawIndexedIndirect2KHR: PFN_vkCmdDrawIndexedIndirect2KHR,
    vkCmdDrawIndirectCount2KHR: PFN_vkCmdDrawIndirectCount2KHR,
    vkCmdDrawIndexedIndirectCount2KHR: PFN_vkCmdDrawIndexedIndirectCount2KHR,
    vkGetPhysicalDeviceFormatProperties: PFN_vkGetPhysicalDeviceFormatProperties,
    vkCreateQueryPool: PFN_vkCreateQueryPool,
    vkDestroyQueryPool: PFN_vkDestroyQueryPool,
    vkCmdResetQueryPool: PFN_vkCmdResetQueryPool,
    vkCmdBeginQuery: PFN_vkCmdBeginQuery,
    vkCmdEndQuery: PFN_vkCmdEndQuery,
    vkCmdWriteTimestamp2: PFN_vkCmdWriteTimestamp2,
    vkCmdCopyQueryPoolResults: PFN_vkCmdCopyQueryPoolResults,
    vkCmdCopyImage2: PFN_vkCmdCopyImage2,
    vkCmdResolveImage2: PFN_vkCmdResolveImage2,
}

fn status(result: vk::VkResult) -> Result<(), Status> {
    match result {
        vk::VkResult_VK_SUCCESS => Ok(()),
        vk::VkResult_VK_TIMEOUT => Err(TIMEOUT),
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY | vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY => {
            Err(OUT_OF_MEMORY)
        }
        vk::VkResult_VK_ERROR_DEVICE_LOST => Err(DEVICE_LOST),
        vk::VkResult_VK_ERROR_FEATURE_NOT_PRESENT
        | vk::VkResult_VK_ERROR_EXTENSION_NOT_PRESENT
        | vk::VkResult_VK_ERROR_FORMAT_NOT_SUPPORTED => Err(UNSUPPORTED),
        _ => Err(BACKEND_ERROR),
    }
}

pub(super) fn snapshot(
    instance: &Instance,
    physical: vk::VkPhysicalDevice,
) -> Result<Snapshot, Status> {
    let f = Functions::load(instance)?;
    let info = instance.device_info(physical).map_err(loader_error)?;
    let mut families = Vec::new();
    let mut memory = vk::VkPhysicalDeviceMemoryProperties::default();
    // SAFETY: retained physical device; native structs sized by generated bindings.
    unsafe {
        let mut count = 0;
        (f.vkGetPhysicalDeviceQueueFamilyProperties.unwrap())(
            physical,
            &mut count,
            ptr::null_mut(),
        );
        families.resize(count as usize, vk::VkQueueFamilyProperties::default());
        if count != 0 {
            (f.vkGetPhysicalDeviceQueueFamilyProperties.unwrap())(
                physical,
                &mut count,
                families.as_mut_ptr(),
            );
            families.truncate(count as usize);
        }
        (f.vkGetPhysicalDeviceMemoryProperties.unwrap())(physical, &mut memory);
    }
    let mut features = FeatureInfo::default();
    let mut memory_limits = MemoryLimits::default();
    let mut descriptor_limits = DescriptorLimits::default();
    let mut execution_limits = ExecutionLimits::default();
    let mut graphics_limits = GraphicsLimits::default();
    let mut query_limits = QueryLimits::default();
    let mut cache_uuid = [0; 16];
    if crate::compute::require_baseline(&info).is_ok() {
        let has_unified = instance
            .supports_extension(physical, c"VK_KHR_unified_image_layouts")
            .map_err(loader_error)?;
        let mut unified = vk::VkPhysicalDeviceUnifiedImageLayoutsFeaturesKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_UNIFIED_IMAGE_LAYOUTS_FEATURES_KHR,
            ..Default::default()
        };
        let mut v14 = vk::VkPhysicalDeviceVulkan14Features {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_4_FEATURES,
            pNext: if has_unified {
                ptr::from_mut(&mut unified).cast()
            } else {
                ptr::null_mut()
            },
            ..Default::default()
        };
        let mut v13 = vk::VkPhysicalDeviceVulkan13Features {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_3_FEATURES,
            pNext: ptr::from_mut(&mut v14).cast(),
            ..Default::default()
        };
        let mut root = vk::VkPhysicalDeviceFeatures2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2,
            pNext: ptr::from_mut(&mut v13).cast(),
            ..Default::default()
        };
        let mut v12_properties = vk::VkPhysicalDeviceVulkan12Properties {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_2_PROPERTIES,
            ..Default::default()
        };
        let mut heaps = vk::VkPhysicalDeviceDescriptorHeapPropertiesEXT {
            pNext: ptr::from_mut(&mut v12_properties).cast(),
            sType:
                vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DESCRIPTOR_HEAP_PROPERTIES_EXT,
            ..Default::default()
        };
        let mut maintenance3 = vk::VkPhysicalDeviceMaintenance3Properties {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_MAINTENANCE_3_PROPERTIES,
            pNext: ptr::from_mut(&mut heaps).cast(),
            ..Default::default()
        };
        let mut maintenance4 = vk::VkPhysicalDeviceMaintenance4Properties {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_MAINTENANCE_4_PROPERTIES,
            pNext: ptr::from_mut(&mut maintenance3).cast(),
            ..Default::default()
        };
        let mut properties = vk::VkPhysicalDeviceProperties2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2,
            pNext: ptr::from_mut(&mut maintenance4).cast(),
            ..Default::default()
        };
        unsafe {
            (f.vkGetPhysicalDeviceFeatures2.unwrap())(physical, &mut root);
            (f.vkGetPhysicalDeviceProperties2.unwrap())(physical, &mut properties);
        }
        features.baseline_supported = u32::from(v14.maintenance5 != 0 && v13.maintenance4 != 0);
        cache_uuid = properties.properties.pipelineCacheUUID;
        if v13.pipelineCreationCacheControl != 0 {
            features.available |= CACHE_CONTROL;
        }
        memory_limits = MemoryLimits {
            max_buffer_size: maintenance4.maxBufferSize,
            max_allocation_size: maintenance3.maxMemoryAllocationSize,
            cache_atom_size: properties.properties.limits.nonCoherentAtomSize,
            map_alignment: properties.properties.limits.minMemoryMapAlignment as u64,
        };
        descriptor_limits = DescriptorLimits {
            buffer_size: heaps.bufferDescriptorSize,
            buffer_alignment: heaps.bufferDescriptorAlignment,
            image_size: heaps.imageDescriptorSize,
            image_alignment: heaps.imageDescriptorAlignment,
            sampler_size: heaps.samplerDescriptorSize,
            sampler_alignment: heaps.samplerDescriptorAlignment,
            resource_heap_alignment: heaps.resourceHeapAlignment,
            resource_heap_max_size: heaps.maxResourceHeapSize,
            resource_reserved_size: heaps.minResourceHeapReservedRange,
            resource_reserved_alignment: heaps
                .imageDescriptorAlignment
                .max(heaps.bufferDescriptorAlignment),
            sampler_heap_alignment: heaps.samplerHeapAlignment,
            sampler_heap_max_size: heaps.maxSamplerHeapSize,
            sampler_reserved_size: heaps.minSamplerHeapReservedRange,
            sampler_reserved_alignment: heaps.samplerDescriptorAlignment,
            uniform_address_alignment: properties.properties.limits.minUniformBufferOffsetAlignment,
            storage_address_alignment: properties.properties.limits.minStorageBufferOffsetAlignment,
            max_uniform_range: u64::from(properties.properties.limits.maxUniformBufferRange),
            max_storage_range: u64::from(properties.properties.limits.maxStorageBufferRange),
            max_sampler_lod_bias: properties.properties.limits.maxSamplerLodBias,
            max_sampler_anisotropy: properties.properties.limits.maxSamplerAnisotropy,
        };
        if root.features.samplerAnisotropy != 0 {
            features.available |= SAMPLER_ANISOTROPY;
        }
        execution_limits = ExecutionLimits {
            max_inline_size: heaps.maxPushDataSize,
            max_groups: properties.properties.limits.maxComputeWorkGroupCount,
            max_local_size: properties.properties.limits.maxComputeWorkGroupSize,
            max_local_invocations: properties.properties.limits.maxComputeWorkGroupInvocations,
            max_shared_memory: properties.properties.limits.maxComputeSharedMemorySize,
            argument_flags: 1, // One native byte namespace, not isolated stage banks.
        };
        let p = &properties.properties.limits;
        query_limits = QueryLimits {
            timestamp_period_ns: p.timestampPeriod,
            timestamp_compute_graphics: p.timestampComputeAndGraphics,
        };
        graphics_limits = GraphicsLimits {
            max_colors: p.maxColorAttachments,
            max_width: p.maxFramebufferWidth,
            max_height: p.maxFramebufferHeight,
            max_layers: p.maxFramebufferLayers,
            max_viewport: p.maxViewportDimensions,
            viewport_bounds: p.viewportBoundsRange,
            color_samples: p.framebufferColorSampleCounts,
            depth_samples: p.framebufferDepthSampleCounts,
            no_attachment_samples: p.framebufferNoAttachmentsSampleCounts,
            max_indirect_count: p.maxDrawIndirectCount,
            integer_color_samples: v12_properties.framebufferIntegerColorSampleCounts,
            stencil_samples: p.framebufferStencilSampleCounts,
            max_vertex_bindings: p.maxVertexInputBindings,
            max_vertex_attributes: p.maxVertexInputAttributes,
            max_vertex_attribute_offset: p.maxVertexInputAttributeOffset,
            max_vertex_stride: p.maxVertexInputBindingStride,
        };
        let caps = info.capabilities;
        if v13.dynamicRendering != 0
            && caps.multi_draw_indirect != 0
            && caps.draw_indirect_count != 0
            && caps.shader_draw_parameters != 0
            && caps.graphics_queue != 0
        {
            features.available |= RASTER;
        }
        if caps.shader_float16 != 0 {
            features.available |= FLOAT16;
        }
        if has_unified && unified.unifiedImageLayouts != 0 {
            features.available |= UNIFIED_IMAGES;
        }
        features.max_timeline_difference = v12_properties.maxTimelineSemaphoreValueDifference;
    }
    Ok(Snapshot {
        cache_uuid,
        info: AdapterInfo {
            name: info.name,
            backend: crate::BACKEND_VULKAN,
            vendor_id: info.vendor_id,
            device_id: info.device_id,
            device_type: info.device_type,
            native_api_version: (info.vulkan_api_major << 22)
                | (info.vulkan_api_minor << 12)
                | info.vulkan_api_patch,
        },
        queues: families
            .iter()
            .enumerate()
            .map(|(index, q)| QueueInfo {
                domain: index as u32,
                // Core graphics/compute queues also support transfer commands even
                // when a driver does not explicitly include the TRANSFER flag.
                flags: (q.queueFlags & 31)
                    | if q.queueFlags & (GRAPHICS | COMPUTE) != 0 {
                        TRANSFER
                    } else {
                        0
                    },
                count: q.queueCount,
                timestamp_bits: q.timestampValidBits,
                copy_granularity: [
                    q.minImageTransferGranularity.width,
                    q.minImageTransferGranularity.height,
                    q.minImageTransferGranularity.depth,
                ],
                reserved: 0,
            })
            .collect(),
        memory_types: memory.memoryTypes[..memory.memoryTypeCount as usize]
            .iter()
            .enumerate()
            .map(|(id, m)| MemoryTypeInfo {
                id: id as u32,
                heap: m.heapIndex,
                properties: m.propertyFlags & 255,
                reserved: 0,
            })
            .collect(),
        memory_heaps: memory.memoryHeaps[..memory.memoryHeapCount as usize]
            .iter()
            .enumerate()
            .map(|(id, m)| MemoryHeapInfo {
                id: id as u32,
                size: m.size,
                device_local: m.flags & 1,
            })
            .collect(),
        features,
        memory_limits,
        descriptor_limits,
        execution_limits,
        graphics_limits,
        query_limits,
    })
}

pub struct Device {
    physical: vk::VkPhysicalDevice,
    handle: vk::VkDevice,
    f: Functions,
    pub(super) snapshot: Snapshot,
    queues: Vec<Queue>,
    lost: AtomicBool,
    _instance: Arc<Instance>,
}
pub struct Queue {
    handle: vk::VkQueue,
    device: AtomicPtr<Device>,
    domain: u32,
    index: u32,
}
// SAFETY: device data and queue handles are immutable after construction. Native
// independent creation/query/wait operations allow concurrent use. The only
// mutated state is atomic loss and atomic borrowed queue-parent binding.
// Per-queue access and destruction are externally synchronized by the unsafe C
// contract, not a device-wide lock.
unsafe impl Send for Device {}
unsafe impl Sync for Device {}

impl Drop for Device {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: C caller has destroyed children and completed all operations.
            unsafe {
                (self.f.vkDestroyDevice.unwrap())(self.handle, ptr::null());
            }
        }
    }
}
impl Device {
    pub(super) fn create(
        adapter: &Adapter,
        requests: &[QueueRequest],
        enabled: u64,
    ) -> Result<Box<Self>, Status> {
        let mut snapshot = adapter.snapshot.clone();
        snapshot.features.enabled = enabled;
        snapshot.features.device_scope = 1;
        // Device queries report created queue counts, not physical capacity.
        for q in &mut snapshot.queues {
            q.count = requests
                .iter()
                .find(|r| r.domain == q.domain)
                .map_or(0, |r| r.count);
        }
        let mut result = Box::new(Self {
            physical: adapter.physical,
            handle: ptr::null_mut(),
            f: Functions::load(&adapter.instance)?,
            snapshot,
            queues: Vec::new(),
            lost: AtomicBool::new(false),
            _instance: adapter.instance.clone(),
        });
        let priorities: Vec<Vec<f32>> = requests
            .iter()
            .map(|r| vec![r.priority; r.count as usize])
            .collect();
        let queue_infos: Vec<_> = requests
            .iter()
            .zip(&priorities)
            .map(|(r, p)| vk::VkDeviceQueueCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
                queueFamilyIndex: r.domain,
                queueCount: r.count,
                pQueuePriorities: p.as_ptr(),
                ..Default::default()
            })
            .collect();
        let mut unified = vk::VkPhysicalDeviceUnifiedImageLayoutsFeaturesKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_UNIFIED_IMAGE_LAYOUTS_FEATURES_KHR,
            unifiedImageLayouts: 1, ..Default::default()
        };
        let mut untyped = vk::VkPhysicalDeviceShaderUntypedPointersFeaturesKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SHADER_UNTYPED_POINTERS_FEATURES_KHR,
            shaderUntypedPointers: 1,
            pNext: if enabled & UNIFIED_IMAGES != 0 { ptr::from_mut(&mut unified).cast() } else { ptr::null_mut() },
        };
        let mut addresses = vk::VkPhysicalDeviceDeviceAddressCommandsFeaturesKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DEVICE_ADDRESS_COMMANDS_FEATURES_KHR,
            deviceAddressCommands: 1, pNext: ptr::from_mut(&mut untyped).cast(),
        };
        let mut heaps = vk::VkPhysicalDeviceDescriptorHeapFeaturesEXT {
            sType:
                vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DESCRIPTOR_HEAP_FEATURES_EXT,
            descriptorHeap: 1,
            pNext: ptr::from_mut(&mut addresses).cast(),
            ..Default::default()
        };
        let mut v14 = vk::VkPhysicalDeviceVulkan14Features {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_4_FEATURES,
            maintenance5: 1,
            pNext: ptr::from_mut(&mut heaps).cast(),
            ..Default::default()
        };
        let mut v13 = vk::VkPhysicalDeviceVulkan13Features {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_3_FEATURES,
            synchronization2: 1,
            maintenance4: 1,
            dynamicRendering: u32::from(enabled & RASTER != 0),
            pipelineCreationCacheControl: u32::from(enabled & CACHE_CONTROL != 0),
            pNext: ptr::from_mut(&mut v14).cast(),
            ..Default::default()
        };
        let mut v12 = vk::VkPhysicalDeviceVulkan12Features {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_2_FEATURES,
            bufferDeviceAddress: 1,
            timelineSemaphore: 1,
            shaderFloat16: u32::from(enabled & FLOAT16 != 0),
            drawIndirectCount: u32::from(enabled & RASTER != 0),
            pNext: ptr::from_mut(&mut v13).cast(),
            ..Default::default()
        };
        let mut storage16 = vk::VkPhysicalDevice16BitStorageFeatures {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_16BIT_STORAGE_FEATURES,
            storageBuffer16BitAccess: 1,
            pNext: ptr::from_mut(&mut v12).cast(),
            ..Default::default()
        };
        let draw_parameters = vk::VkPhysicalDeviceShaderDrawParametersFeatures {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SHADER_DRAW_PARAMETERS_FEATURES,
            shaderDrawParameters: u32::from(enabled & RASTER != 0), pNext: ptr::from_mut(&mut storage16).cast(),
        };
        let features = vk::VkPhysicalDeviceFeatures {
            multiDrawIndirect: u32::from(enabled & RASTER != 0),
            samplerAnisotropy: u32::from(enabled & SAMPLER_ANISOTROPY != 0),
            ..Default::default()
        };
        let mut extensions = vec![
            c"VK_EXT_descriptor_heap".as_ptr(),
            c"VK_KHR_device_address_commands".as_ptr(),
            c"VK_KHR_shader_untyped_pointers".as_ptr(),
        ];
        if enabled & UNIFIED_IMAGES != 0 {
            extensions.push(c"VK_KHR_unified_image_layouts".as_ptr());
        }
        let create = vk::VkDeviceCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,
            pNext: ptr::from_ref(&draw_parameters).cast(),
            pEnabledFeatures: &features,
            queueCreateInfoCount: queue_infos.len() as u32,
            pQueueCreateInfos: queue_infos.as_ptr(),
            enabledExtensionCount: extensions.len() as u32,
            ppEnabledExtensionNames: extensions.as_ptr(),
            ..Default::default()
        };
        // SAFETY: requests and enabling were validated against this adapter;
        // pointers and extension chains stay live through the synchronous call.
        let mut handle = ptr::null_mut();
        unsafe {
            status((result.f.vkCreateDevice.unwrap())(
                adapter.physical,
                &create,
                ptr::null(),
                &mut handle,
            ))?;
        }
        result.handle = handle;
        for r in requests {
            for index in 0..r.count {
                let mut handle = ptr::null_mut();
                unsafe {
                    (result.f.vkGetDeviceQueue.unwrap())(
                        result.handle,
                        r.domain,
                        index,
                        &mut handle,
                    );
                }
                result.queues.push(Queue {
                    handle,
                    device: AtomicPtr::new(ptr::null_mut()),
                    domain: r.domain,
                    index,
                });
            }
        }
        Ok(result)
    }
    pub(super) fn queue(&self, domain: u32, index: u32) -> Option<&Queue> {
        let queue = self
            .queues
            .iter()
            .find(|q| q.domain == domain && q.index == index)?;
        // Bind after construction, from the stable borrowed device address.
        // Concurrent lookup is permitted: no constructor-Box self-reference,
        // queue allocation or device lock is needed.
        queue
            .device
            .store(ptr::from_ref(self).cast_mut(), Ordering::Relaxed);
        Some(queue)
    }
    fn ready(&self) -> Result<(), Status> {
        if self.lost.load(Ordering::Relaxed) {
            Err(DEVICE_LOST)
        } else {
            Ok(())
        }
    }
    fn result(&self, native: vk::VkResult) -> Result<(), Status> {
        let result = status(native);
        if result == Err(DEVICE_LOST) {
            self.lost.store(true, Ordering::Relaxed);
        }
        result
    }
}

pub struct Timeline {
    // Non-owning. Only unsafe C entry points can construct/use this object.
    device: *const Device,
    handle: vk::VkSemaphore,
}
impl Drop for Timeline {
    fn drop(&mut self) {
        // SAFETY: caller keeps device alive and excludes pending timeline use.
        unsafe {
            let d = &*self.device;
            if !self.handle.is_null() {
                (d.f.vkDestroySemaphore.unwrap())(d.handle, self.handle, ptr::null());
            }
        }
    }
}
impl Timeline {
    pub(super) fn create(device: &Device, initial: u64) -> Result<Box<Self>, Status> {
        device.ready()?;
        let mut result = Box::new(Self {
            device,
            handle: ptr::null_mut(),
        });
        let timeline = vk::VkSemaphoreTypeCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_TYPE_CREATE_INFO,
            semaphoreType: vk::VkSemaphoreType_VK_SEMAPHORE_TYPE_TIMELINE,
            initialValue: initial,
            ..Default::default()
        };
        let create = vk::VkSemaphoreCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO,
            pNext: ptr::from_ref(&timeline).cast(),
            ..Default::default()
        };
        let mut handle = ptr::null_mut();
        unsafe {
            device.result((device.f.vkCreateSemaphore.unwrap())(
                device.handle,
                &create,
                ptr::null(),
                &mut handle,
            ))?;
        }
        result.handle = handle;
        Ok(result)
    }
    pub(super) fn poll(&self) -> Result<u64, Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        let mut value = 0;
        unsafe {
            d.result((d.f.vkGetSemaphoreCounterValue.unwrap())(
                d.handle,
                self.handle,
                &mut value,
            ))?;
        }
        Ok(value)
    }
    pub(super) fn wait(&self, value: u64, timeout: u64) -> Result<(), Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        let info = vk::VkSemaphoreWaitInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_WAIT_INFO,
            semaphoreCount: 1,
            pSemaphores: &self.handle,
            pValues: &value,
            ..Default::default()
        };
        unsafe { d.result((d.f.vkWaitSemaphores.unwrap())(d.handle, &info, timeout)) }
    }
    pub(super) fn signal(&self, value: u64) -> Result<(), Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        let info = vk::VkSemaphoreSignalInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_SIGNAL_INFO,
            semaphore: self.handle,
            value,
            ..Default::default()
        };
        unsafe { d.result((d.f.vkSignalSemaphore.unwrap())(d.handle, &info)) }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;
    use std::cell::Cell;

    thread_local! { static POLLS: Cell<u32> = const { Cell::new(0) }; }
    unsafe extern "C" fn fail_create(
        _: vk::VkDevice,
        _: *const vk::VkSemaphoreCreateInfo,
        _: *const vk::VkAllocationCallbacks,
        out: *mut vk::VkSemaphore,
    ) -> vk::VkResult {
        // Native failure output is unspecified: never adopt/destroy this value.
        unsafe {
            out.write(ptr::dangling_mut());
        }
        vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY
    }
    unsafe extern "C" fn fail_poll(
        _: vk::VkDevice,
        _: vk::VkSemaphore,
        out: *mut u64,
    ) -> vk::VkResult {
        POLLS.set(POLLS.get() + 1);
        unsafe {
            out.write(999);
        }
        vk::VkResult_VK_ERROR_DEVICE_LOST
    }
    #[test]
    fn native_statuses_are_distinct() {
        assert_eq!(status(vk::VkResult_VK_TIMEOUT), Err(TIMEOUT));
        assert_eq!(
            status(vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY),
            Err(OUT_OF_MEMORY)
        );
        assert_eq!(status(vk::VkResult_VK_ERROR_DEVICE_LOST), Err(DEVICE_LOST));
        assert_eq!(
            status(vk::VkResult_VK_ERROR_FEATURE_NOT_PRESENT),
            Err(UNSUPPORTED)
        );
    }
    #[test]
    #[ignore = "requires modern Vulkan; setup creation-failure cleanup and synthetic sticky loss"]
    fn gpu_foundation_failures() {
        let instance = Arc::new(Instance::new().unwrap());
        let mut tested = 0;
        for physical in instance.physical_devices().unwrap() {
            let snapshot = snapshot(&instance, physical).unwrap();
            if snapshot.features.baseline_supported == 0 {
                continue;
            }
            let family = snapshot
                .queues
                .iter()
                .find(|q| q.count != 0)
                .unwrap()
                .domain;
            let adapter = Adapter {
                instance: instance.clone(),
                physical,
                snapshot,
            };
            let mut d = Device::create(
                &adapter,
                &[QueueRequest {
                    domain: family,
                    count: 1,
                    priority: 1.0,
                }],
                0,
            )
            .unwrap();
            let create = d.f.vkCreateSemaphore;
            d.f.vkCreateSemaphore = Some(fail_create);
            let mut t = ptr::dangling_mut();
            unsafe {
                assert_eq!(ogpu_next_timeline_create(&mut *d, 0, &mut t), OUT_OF_MEMORY);
            }
            assert!(t.is_null());
            assert_eq!(d.ready(), Ok(()));
            d.f.vkCreateSemaphore = create;
            d.f.vkGetSemaphoreCounterValue = Some(fail_poll);
            let device_pointer = ptr::from_ref(&*d).cast_mut();
            unsafe {
                assert_eq!(ogpu_next_timeline_create(device_pointer, 0, &mut t), OK);
            }
            POLLS.set(0);
            let mut observed = 123;
            unsafe {
                assert_eq!(ogpu_next_timeline_poll(t, &mut observed), DEVICE_LOST);
                assert_eq!(observed, 123);
                assert_eq!(ogpu_next_timeline_poll(t, &mut observed), DEVICE_LOST);
                assert_eq!(POLLS.get(), 1); // No native retry after terminal loss.
                let point = Point {
                    timeline: t,
                    value: 1,
                };
                assert_eq!(ogpu_next_timeline_signal_host(point), DEVICE_LOST);
                assert_eq!(ogpu_next_timeline_wait(point, 0), DEVICE_LOST);
                let mut other = ptr::dangling_mut();
                assert_eq!(
                    ogpu_next_timeline_create(device_pointer, 0, &mut other),
                    DEVICE_LOST
                );
                assert!(other.is_null());
                ogpu_next_timeline_destroy(t);
            }
            // The real device is idle/healthy; only the poll result was injected.
            tested += 1;
        }
        assert!(tested != 0, "No suitable device; not a passing skip");
    }
}
