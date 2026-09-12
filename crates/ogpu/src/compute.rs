//! First execution experiment: one externally synchronized queue, addressable host-visible
//! buffers, descriptor-free compute pipelines, and one-shot asynchronous submission.
use crate::{vulkan::Instance, Error, INVALID_ARGUMENT, LOADER_ERROR, OUT_OF_RANGE, UNSUPPORTED};
use ogpu_vulkan_sys as vk;
use std::{cell::Cell, ptr, rc::Rc, sync::Arc};

#[path = "batch.rs"]
mod batch;
pub(crate) use batch::{Batch, Completion};
#[path = "graphics.rs"]
mod graphics;
pub(crate) use graphics::{Raster, Target};

#[cfg(test)]
#[path = "reduction_tests.rs"]
mod reduction_tests;

macro_rules! functions {
    ($($name:ident: $ty:ident),* $(,)?) => {
        #[allow(non_snake_case)]
        struct Functions { $($name: vk::$ty,)* }
        impl Functions {
            #[allow(non_snake_case)]
            fn load(instance: &Instance) -> Result<Self, Error> {
                $(
                    // SAFETY: command spelling and its generated PFN are paired here.
                    let $name: vk::$ty = unsafe { std::mem::transmute(instance.proc(
                        std::ffi::CStr::from_bytes_with_nul(concat!(stringify!($name), "\0").as_bytes()).unwrap()
                    )) };
                    if $name.is_none() { return Err(Error::new(LOADER_ERROR, concat!("Missing ", stringify!($name)))); }
                )*
                Ok(Self { $($name,)* })
            }
        }
    };
}
functions! {
    vkGetPhysicalDeviceMemoryProperties: PFN_vkGetPhysicalDeviceMemoryProperties,
    vkGetPhysicalDeviceProperties: PFN_vkGetPhysicalDeviceProperties,
    vkGetPhysicalDeviceQueueFamilyProperties: PFN_vkGetPhysicalDeviceQueueFamilyProperties,
    vkCreateDevice: PFN_vkCreateDevice, vkDestroyDevice: PFN_vkDestroyDevice,
    vkGetDeviceQueue: PFN_vkGetDeviceQueue,
    vkCreateBuffer: PFN_vkCreateBuffer, vkDestroyBuffer: PFN_vkDestroyBuffer,
    vkGetBufferMemoryRequirements: PFN_vkGetBufferMemoryRequirements,
    vkAllocateMemory: PFN_vkAllocateMemory, vkFreeMemory: PFN_vkFreeMemory,
    vkBindBufferMemory: PFN_vkBindBufferMemory,
    vkMapMemory: PFN_vkMapMemory, vkUnmapMemory: PFN_vkUnmapMemory,
    vkFlushMappedMemoryRanges: PFN_vkFlushMappedMemoryRanges,
    vkInvalidateMappedMemoryRanges: PFN_vkInvalidateMappedMemoryRanges,
    vkGetBufferDeviceAddress: PFN_vkGetBufferDeviceAddress,
    vkCreateShaderModule: PFN_vkCreateShaderModule, vkDestroyShaderModule: PFN_vkDestroyShaderModule,
    vkCreatePipelineLayout: PFN_vkCreatePipelineLayout, vkDestroyPipelineLayout: PFN_vkDestroyPipelineLayout,
    vkCreateComputePipelines: PFN_vkCreateComputePipelines, vkDestroyPipeline: PFN_vkDestroyPipeline,
    vkCreateCommandPool: PFN_vkCreateCommandPool, vkDestroyCommandPool: PFN_vkDestroyCommandPool,
    vkAllocateCommandBuffers: PFN_vkAllocateCommandBuffers,
    vkBeginCommandBuffer: PFN_vkBeginCommandBuffer, vkEndCommandBuffer: PFN_vkEndCommandBuffer,
    vkCmdBindPipeline: PFN_vkCmdBindPipeline, vkCmdPushConstants: PFN_vkCmdPushConstants,
    vkCmdDispatch: PFN_vkCmdDispatch, vkCmdPipelineBarrier: PFN_vkCmdPipelineBarrier,
    vkQueueSubmit: PFN_vkQueueSubmit, vkQueueWaitIdle: PFN_vkQueueWaitIdle,
    vkCreateFence: PFN_vkCreateFence, vkDestroyFence: PFN_vkDestroyFence,
    vkWaitForFences: PFN_vkWaitForFences,
    vkGetPhysicalDeviceImageFormatProperties: PFN_vkGetPhysicalDeviceImageFormatProperties,
    vkCreateImage: PFN_vkCreateImage, vkDestroyImage: PFN_vkDestroyImage,
    vkGetImageMemoryRequirements: PFN_vkGetImageMemoryRequirements,
    vkBindImageMemory: PFN_vkBindImageMemory,
    vkCreateImageView: PFN_vkCreateImageView, vkDestroyImageView: PFN_vkDestroyImageView,
    vkCreateRenderPass: PFN_vkCreateRenderPass, vkDestroyRenderPass: PFN_vkDestroyRenderPass,
    vkCreateFramebuffer: PFN_vkCreateFramebuffer, vkDestroyFramebuffer: PFN_vkDestroyFramebuffer,
    vkCreateGraphicsPipelines: PFN_vkCreateGraphicsPipelines,
    vkCmdBeginRenderPass: PFN_vkCmdBeginRenderPass, vkCmdEndRenderPass: PFN_vkCmdEndRenderPass,
    vkCmdSetViewport: PFN_vkCmdSetViewport, vkCmdSetScissor: PFN_vkCmdSetScissor,
    vkCmdDrawIndirect: PFN_vkCmdDrawIndirect, vkCmdCopyImageToBuffer: PFN_vkCmdCopyImageToBuffer,
    vkCreateQueryPool: PFN_vkCreateQueryPool, vkDestroyQueryPool: PFN_vkDestroyQueryPool,
    vkCmdResetQueryPool: PFN_vkCmdResetQueryPool, vkCmdWriteTimestamp: PFN_vkCmdWriteTimestamp,
    vkGetQueryPoolResults: PFN_vkGetQueryPoolResults,
}

fn check(operation: &str, result: vk::VkResult) -> Result<(), Error> {
    if result == vk::VkResult_VK_SUCCESS {
        Ok(())
    } else {
        Err(Error::vulkan(operation, result))
    }
}

pub(crate) struct Device {
    handle: vk::VkDevice,
    queue: vk::VkQueue,
    family: u32,
    physical: vk::VkPhysicalDevice,
    graphics: bool,
    timestamp_bits: u32,
    memory: vk::VkPhysicalDeviceMemoryProperties,
    limits: vk::VkPhysicalDeviceLimits,
    lost: Cell<bool>,
    f: Functions,
    _instance: Arc<Instance>,
}

impl Drop for Device {
    fn drop(&mut self) {
        // SAFETY: all children retain this device; completions drain before releasing it.
        // The instance/library still live.
        unsafe {
            (self.f.vkDestroyDevice.unwrap())(self.handle, ptr::null());
        }
    }
}

impl Device {
    pub(crate) fn new(
        instance: Arc<Instance>,
        physical: vk::VkPhysicalDevice,
    ) -> Result<Rc<Self>, Error> {
        Self::create(instance, physical, false)
    }

    pub(crate) fn new_graphics(
        instance: Arc<Instance>,
        physical: vk::VkPhysicalDevice,
    ) -> Result<Rc<Self>, Error> {
        Self::create(instance, physical, true)
    }

    fn create(
        instance: Arc<Instance>,
        physical: vk::VkPhysicalDevice,
        require_graphics: bool,
    ) -> Result<Rc<Self>, Error> {
        let info = instance.device_info(physical)?;
        if info.vulkan_api_major < 1
            || (info.vulkan_api_major == 1 && info.vulkan_api_minor < 2)
            || info.capabilities.buffer_device_address == 0
            || info.capabilities.compute_queue == 0
        {
            return Err(Error::new(
                UNSUPPORTED,
                "Execution requires Vulkan 1.2, buffer device addresses, and a compute queue",
            ));
        }
        let f = Functions::load(&instance)?;
        // SAFETY: the physical handle belongs to the retained instance. Each query has
        // initialized, appropriately sized outputs; creation enables only the BDA feature.
        unsafe {
            let mut count = 0;
            (f.vkGetPhysicalDeviceQueueFamilyProperties.unwrap())(
                physical,
                &mut count,
                ptr::null_mut(),
            );
            let mut families = vec![vk::VkQueueFamilyProperties::default(); count as usize];
            if count != 0 {
                (f.vkGetPhysicalDeviceQueueFamilyProperties.unwrap())(
                    physical,
                    &mut count,
                    families.as_mut_ptr(),
                );
            }
            let family =
                queue_family(&families[..count as usize], require_graphics).ok_or_else(|| {
                    Error::new(
                        UNSUPPORTED,
                        "No queue supporting the requested execution profile",
                    )
                })?;
            let graphics = families[family as usize].queueFlags
                & vk::VkQueueFlagBits_VK_QUEUE_GRAPHICS_BIT
                != 0;
            let mut properties = vk::VkPhysicalDeviceProperties::default();
            (f.vkGetPhysicalDeviceProperties.unwrap())(physical, &mut properties);
            let mut memory = vk::VkPhysicalDeviceMemoryProperties::default();
            (f.vkGetPhysicalDeviceMemoryProperties.unwrap())(physical, &mut memory);
            let priority = 1.0;
            let queue_info = vk::VkDeviceQueueCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
                queueFamilyIndex: family,
                queueCount: 1,
                pQueuePriorities: &priority,
                ..Default::default()
            };
            let bda = vk::VkPhysicalDeviceBufferDeviceAddressFeatures { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_BUFFER_DEVICE_ADDRESS_FEATURES,
                bufferDeviceAddress: vk::VK_TRUE, ..Default::default() };
            let create = vk::VkDeviceCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,
                pNext: (&bda as *const vk::VkPhysicalDeviceBufferDeviceAddressFeatures).cast(),
                queueCreateInfoCount: 1,
                pQueueCreateInfos: &queue_info,
                ..Default::default()
            };
            let mut handle = ptr::null_mut();
            check(
                "vkCreateDevice",
                (f.vkCreateDevice.unwrap())(physical, &create, ptr::null(), &mut handle),
            )?;
            // No fallible operation between successful creation and wrapping ownership.
            let mut queue = ptr::null_mut();
            (f.vkGetDeviceQueue.unwrap())(handle, family, 0, &mut queue);
            Ok(Rc::new(Self {
                handle,
                queue,
                family,
                physical,
                graphics,
                timestamp_bits: families[family as usize].timestampValidBits,
                memory,
                limits: properties.limits,
                lost: Cell::new(false),
                f,
                _instance: instance,
            }))
        }
    }

    pub(crate) fn timing_info(&self) -> Result<(f64, u32), Error> {
        self.ready()?;
        timestamp_info(self.timestamp_bits, self.limits.timestampPeriod)
    }

    fn ready(&self) -> Result<(), Error> {
        if self.lost.get() {
            Err(Error::vulkan(
                "device is lost",
                vk::VkResult_VK_ERROR_DEVICE_LOST,
            ))
        } else {
            Ok(())
        }
    }

    fn result(&self, op: &str, status: vk::VkResult) -> Result<(), Error> {
        if status == vk::VkResult_VK_ERROR_DEVICE_LOST {
            self.lost.set(true);
        }
        check(op, status)
    }
}

fn timestamp_info(bits: u32, period: f32) -> Result<(f64, u32), Error> {
    if !(36..=64).contains(&bits) || !period.is_finite() || period <= 0.0 {
        Err(Error::new(
            UNSUPPORTED,
            "Selected queue has no supported timestamp clock",
        ))
    } else {
        Ok((f64::from(period), bits))
    }
}

fn queue_family(families: &[vk::VkQueueFamilyProperties], graphics: bool) -> Option<u32> {
    let required = vk::VkQueueFlagBits_VK_QUEUE_COMPUTE_BIT
        | if graphics {
            vk::VkQueueFlagBits_VK_QUEUE_GRAPHICS_BIT
        } else {
            0
        };
    families
        .iter()
        .position(|f| f.queueCount != 0 && f.queueFlags & required == required)
        .map(|i| i as u32)
}

pub(crate) struct Buffer {
    device: Rc<Device>,
    buffer: vk::VkBuffer,
    memory: vk::VkDeviceMemory,
    mapped: *mut u8,
    size: usize,
    address: u64,
    coherent: bool,
}

impl Drop for Buffer {
    fn drop(&mut self) {
        // SAFETY: the caller must complete all GPU access before destroying this allocation.
        // Destroy the bound buffer before freeing its memory; partial construction is valid.
        unsafe {
            let d = &self.device;
            if !self.mapped.is_null() {
                (d.f.vkUnmapMemory.unwrap())(d.handle, self.memory);
            }
            if !self.buffer.is_null() {
                (d.f.vkDestroyBuffer.unwrap())(d.handle, self.buffer, ptr::null());
            }
            if !self.memory.is_null() {
                (d.f.vkFreeMemory.unwrap())(d.handle, self.memory, ptr::null());
            }
        }
    }
}

fn memory_type(memory: &vk::VkPhysicalDeviceMemoryProperties, mask: u32) -> Option<(u32, bool)> {
    (0..memory.memoryTypeCount)
        .filter_map(|i| {
            let flags = memory.memoryTypes[i as usize].propertyFlags;
            if mask & (1 << i) == 0
                || flags & vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT == 0
                || flags
                    & (vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_DEVICE_COHERENT_BIT_AMD
                        | vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_PROTECTED_BIT)
                    != 0
            {
                return None;
            }
            Some((
                i,
                flags & vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_HOST_COHERENT_BIT != 0,
            ))
        })
        .max_by_key(|&(_, coherent)| coherent)
}

impl Buffer {
    pub(crate) fn new(device: Rc<Device>, size: usize) -> Result<Self, Error> {
        device.ready()?;
        if size == 0 || size > isize::MAX as usize {
            return Err(Error::new(
                INVALID_ARGUMENT,
                "Buffer size must be in 1..=isize::MAX",
            ));
        }
        let mut result = Self {
            device,
            buffer: ptr::null_mut(),
            memory: ptr::null_mut(),
            mapped: ptr::null_mut(),
            size,
            address: 0,
            coherent: false,
        };
        let d = &result.device;
        // SAFETY: initialized Vulkan structures. The result owns each handle immediately,
        // ensuring cleanup on every later failure. Offset zero satisfies memory alignment.
        unsafe {
            let create = vk::VkBufferCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO,
                size: size as u64,
                usage: vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_STORAGE_BUFFER_BIT
                    | vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_SHADER_DEVICE_ADDRESS_BIT
                    | vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_INDIRECT_BUFFER_BIT
                    | vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_TRANSFER_DST_BIT,
                sharingMode: vk::VkSharingMode_VK_SHARING_MODE_EXCLUSIVE,
                ..Default::default()
            };
            d.result(
                "vkCreateBuffer",
                (d.f.vkCreateBuffer.unwrap())(d.handle, &create, ptr::null(), &mut result.buffer),
            )?;
            let mut requirements = vk::VkMemoryRequirements::default();
            (d.f.vkGetBufferMemoryRequirements.unwrap())(
                d.handle,
                result.buffer,
                &mut requirements,
            );
            let (index, coherent) = memory_type(&d.memory, requirements.memoryTypeBits)
                .ok_or_else(|| Error::new(UNSUPPORTED, "No compatible host-visible memory type"))?;
            result.coherent = coherent;
            // Declare the one-buffer allocation as dedicated as well as owning it that
            // way, satisfying implementations that require dedicated buffer allocations.
            let dedicated = vk::VkMemoryDedicatedAllocateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_DEDICATED_ALLOCATE_INFO,
                buffer: result.buffer,
                ..Default::default()
            };
            let flags = vk::VkMemoryAllocateFlagsInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_FLAGS_INFO,
                pNext: (&dedicated as *const vk::VkMemoryDedicatedAllocateInfo).cast(),
                flags: vk::VkMemoryAllocateFlagBits_VK_MEMORY_ALLOCATE_DEVICE_ADDRESS_BIT,
                ..Default::default()
            };
            let allocate = vk::VkMemoryAllocateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
                pNext: (&flags as *const vk::VkMemoryAllocateFlagsInfo).cast(),
                allocationSize: requirements.size,
                memoryTypeIndex: index,
            };
            d.result(
                "vkAllocateMemory",
                (d.f.vkAllocateMemory.unwrap())(
                    d.handle,
                    &allocate,
                    ptr::null(),
                    &mut result.memory,
                ),
            )?;
            d.result(
                "vkBindBufferMemory",
                (d.f.vkBindBufferMemory.unwrap())(d.handle, result.buffer, result.memory, 0),
            )?;
            let mut mapping = ptr::null_mut();
            d.result(
                "vkMapMemory",
                (d.f.vkMapMemory.unwrap())(
                    d.handle,
                    result.memory,
                    0,
                    vk::VK_WHOLE_SIZE as vk::VkDeviceSize,
                    0,
                    &mut mapping,
                ),
            )?;
            result.mapped = mapping.cast();
            let address_info = vk::VkBufferDeviceAddressInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BUFFER_DEVICE_ADDRESS_INFO,
                buffer: result.buffer,
                ..Default::default()
            };
            result.address = (d.f.vkGetBufferDeviceAddress.unwrap())(d.handle, &address_info);
        }
        Ok(result)
    }

    pub(crate) fn address(&self) -> Result<u64, Error> {
        self.device.ready()?;
        Ok(self.address)
    }

    pub(crate) fn range(&self, offset: usize, length: usize) -> Result<(), Error> {
        if offset > self.size || length > self.size - offset {
            Err(Error::new(
                OUT_OF_RANGE,
                "Buffer transfer exceeds allocation",
            ))
        } else {
            Ok(())
        }
    }

    fn cache(&self, flush: bool) -> Result<(), Error> {
        if self.coherent {
            return Ok(());
        }
        let d = &self.device;
        // Map and maintain the entire dedicated allocation: offset zero / WHOLE_SIZE also
        // satisfy nonCoherentAtomSize requirements when the logical buffer is unaligned.
        let range = vk::VkMappedMemoryRange {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MAPPED_MEMORY_RANGE,
            memory: self.memory,
            offset: 0,
            size: vk::VK_WHOLE_SIZE as vk::VkDeviceSize,
            ..Default::default()
        };
        unsafe {
            if flush {
                d.result(
                    "vkFlushMappedMemoryRanges",
                    (d.f.vkFlushMappedMemoryRanges.unwrap())(d.handle, 1, &range),
                )
            } else {
                d.result(
                    "vkInvalidateMappedMemoryRanges",
                    (d.f.vkInvalidateMappedMemoryRanges.unwrap())(d.handle, 1, &range),
                )
            }
        }
    }

    pub(crate) fn write(&self, offset: usize, bytes: &[u8]) -> Result<(), Error> {
        self.device.ready()?;
        self.range(offset, bytes.len())?;
        if bytes.is_empty() {
            return Ok(());
        }
        // Preserve GPU-written bytes outside a partial CPU update before whole-range flush.
        self.cache(false)?;
        // SAFETY: checked range in a live mapping, with an independent caller-owned slice.
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), self.mapped.add(offset), bytes.len());
        }
        self.cache(true)
    }

    /// # Safety
    /// destination must be writable for length bytes, independent of this allocation.
    /// Unlike a Rust byte slice, a C destination may initially be uninitialized.
    pub(crate) unsafe fn read(
        &self,
        offset: usize,
        destination: *mut u8,
        length: usize,
    ) -> Result<(), Error> {
        self.device.ready()?;
        self.range(offset, length)?;
        if length == 0 {
            return Ok(());
        }
        self.cache(false)?;
        // SAFETY: checked range and no pending work; destination is an independent slice.
        unsafe {
            ptr::copy_nonoverlapping(self.mapped.add(offset), destination, length);
        }
        Ok(())
    }
}

pub(crate) struct Kernel {
    device: Rc<Device>,
    module: vk::VkShaderModule,
    layout: vk::VkPipelineLayout,
    pipeline: vk::VkPipeline,
    push_size: u32,
}

impl Drop for Kernel {
    fn drop(&mut self) {
        // SAFETY: all submitted work has drained; handles are either owned or NULL.
        unsafe {
            let d = &self.device;
            if !self.pipeline.is_null() {
                (d.f.vkDestroyPipeline.unwrap())(d.handle, self.pipeline, ptr::null());
            }
            if !self.layout.is_null() {
                (d.f.vkDestroyPipelineLayout.unwrap())(d.handle, self.layout, ptr::null());
            }
            if !self.module.is_null() {
                (d.f.vkDestroyShaderModule.unwrap())(d.handle, self.module, ptr::null());
            }
        }
    }
}

impl Kernel {
    /// # Safety
    /// SPIR-V must be valid for this device with only BDA enabled, a compute entry named
    /// main, no descriptors, and no push-constant accesses outside push_size bytes.
    pub(crate) unsafe fn new(
        device: Rc<Device>,
        words: &[u32],
        push_size: u32,
    ) -> Result<Self, Error> {
        device.ready()?;
        if words.len() < 5
            || words[0] != 0x07230203
            || push_size % 4 != 0
            || push_size > device.limits.maxPushConstantsSize
        {
            return Err(Error::new(
                INVALID_ARGUMENT,
                "Invalid SPIR-V header or push-constant size",
            ));
        }
        let mut result = Self {
            device,
            module: ptr::null_mut(),
            layout: ptr::null_mut(),
            pipeline: ptr::null_mut(),
            push_size,
        };
        let d = &result.device;
        // SAFETY: shader semantics are the caller's contract; the remaining Vulkan objects
        // and pointers are initialized locally, with cleanup for partial pipeline creation.
        unsafe {
            let module = vk::VkShaderModuleCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO,
                codeSize: std::mem::size_of_val(words),
                pCode: words.as_ptr(),
                ..Default::default()
            };
            d.result(
                "vkCreateShaderModule",
                (d.f.vkCreateShaderModule.unwrap())(
                    d.handle,
                    &module,
                    ptr::null(),
                    &mut result.module,
                ),
            )?;
            let range = vk::VkPushConstantRange {
                stageFlags: vk::VkShaderStageFlagBits_VK_SHADER_STAGE_COMPUTE_BIT,
                offset: 0,
                size: push_size,
            };
            let layout = vk::VkPipelineLayoutCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO,
                pushConstantRangeCount: u32::from(push_size != 0),
                pPushConstantRanges: if push_size == 0 { ptr::null() } else { &range },
                ..Default::default()
            };
            d.result(
                "vkCreatePipelineLayout",
                (d.f.vkCreatePipelineLayout.unwrap())(
                    d.handle,
                    &layout,
                    ptr::null(),
                    &mut result.layout,
                ),
            )?;
            let stage = vk::VkPipelineShaderStageCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
                stage: vk::VkShaderStageFlagBits_VK_SHADER_STAGE_COMPUTE_BIT,
                module: result.module,
                pName: c"main".as_ptr(),
                ..Default::default()
            };
            let pipeline = vk::VkComputePipelineCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMPUTE_PIPELINE_CREATE_INFO,
                stage,
                layout: result.layout,
                basePipelineIndex: -1,
                ..Default::default()
            };
            d.result(
                "vkCreateComputePipelines",
                (d.f.vkCreateComputePipelines.unwrap())(
                    d.handle,
                    ptr::null_mut(),
                    1,
                    &pipeline,
                    ptr::null(),
                    &mut result.pipeline,
                ),
            )?;
        }
        Ok(result)
    }

    /// # Safety
    /// All device addresses reachable by the kernel must refer to live allocations on
    /// this device. Accesses must be in bounds, correctly aligned, and race-free. No
    /// other operation on the device or its children may run concurrently.
    pub(crate) unsafe fn dispatch_wait(
        self: &Rc<Self>,
        groups: u32,
        root: &[u8],
    ) -> Result<(), Error> {
        let mut batch = Batch::new(self.device.clone())?;
        // Preserve the ordered convenience API's dependency on prior compute work.
        batch.barrier(
            batch::COMPUTE_READ | batch::COMPUTE_WRITE,
            batch::COMPUTE_READ | batch::COMPUTE_WRITE,
        )?;
        batch.dispatch(self.clone(), groups, root)?;
        // SAFETY: the caller supplies valid addresses and keeps their allocations live
        // through this synchronous call. The completion also drains on unwinding.
        unsafe { batch.submit()?.wait() }
    }
}

/// A wait allocation error does not establish completion. Preserve resources and retry
/// until completion or device loss, then report the first error. This API has no timeout.
fn drain(mut wait: impl FnMut() -> vk::VkResult) -> vk::VkResult {
    let mut first_error = vk::VkResult_VK_SUCCESS;
    loop {
        let status = wait();
        if status == vk::VkResult_VK_ERROR_DEVICE_LOST {
            return status;
        }
        if status == vk::VkResult_VK_SUCCESS {
            return first_error;
        }
        if status != vk::VkResult_VK_TIMEOUT && first_error == vk::VkResult_VK_SUCCESS {
            first_error = status;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wait_error_does_not_release_pending_resources() {
        let mut calls = 0;
        assert_eq!(
            drain(|| {
                calls += 1;
                if calls == 1 {
                    vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
                } else {
                    vk::VkResult_VK_SUCCESS
                }
            }),
            vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
        );
        assert_eq!(calls, 2);
        assert_eq!(
            drain(|| vk::VkResult_VK_ERROR_DEVICE_LOST),
            vk::VkResult_VK_ERROR_DEVICE_LOST
        );
        let mut responses = [vk::VkResult_VK_TIMEOUT, vk::VkResult_VK_SUCCESS].into_iter();
        assert_eq!(drain(|| responses.next().unwrap()), vk::VkResult_VK_SUCCESS);
        let mut responses = [
            vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY,
            vk::VkResult_VK_TIMEOUT,
            vk::VkResult_VK_SUCCESS,
        ]
        .into_iter();
        assert_eq!(
            drain(|| responses.next().unwrap()),
            vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY
        );
    }
    #[test]
    fn memory_selection_prefers_coherent_but_accepts_noncoherent() {
        let mut memory = vk::VkPhysicalDeviceMemoryProperties {
            memoryTypeCount: 2,
            ..Default::default()
        };
        memory.memoryTypes[0].propertyFlags =
            vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT;
        memory.memoryTypes[1].propertyFlags = memory.memoryTypes[0].propertyFlags
            | vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_HOST_COHERENT_BIT;
        assert_eq!(memory_type(&memory, 3), Some((1, true)));
        assert_eq!(memory_type(&memory, 1), Some((0, false)));
        assert_eq!(memory_type(&memory, 0), None);
        memory.memoryTypes[1].propertyFlags |=
            vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_DEVICE_COHERENT_BIT_AMD;
        assert_eq!(memory_type(&memory, 3), Some((0, false)));
    }
    #[test]
    #[ignore = "requires a real Vulkan loader/device; run with --ignored --nocapture"]
    fn gpu_roundtrip() {
        let instance = Arc::new(Instance::new().unwrap());
        let words: Vec<_> = include_bytes!("../../../examples/shaders/roundtrip.spv")
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        let mut tested = 0;
        for physical in instance.physical_devices().unwrap() {
            let info = instance.device_info(physical).unwrap();
            let device = match Device::new(instance.clone(), physical) {
                Ok(device) => device,
                Err(e) if e.status == UNSUPPORTED => continue,
                Err(e) => panic!("{e:?}"),
            };
            let count = 4099u32;
            let mut buffer = Buffer::new(device.clone(), count as usize * 4).unwrap();
            // Exercise the explicit maintenance calls even on coherent memory (legal in
            // Vulkan). This tests their ranges/lifetimes, not non-coherent hardware effects.
            buffer.coherent = false;
            let mut expected: Vec<u32> = (0..count).collect();
            let input: Vec<u8> = expected.iter().flat_map(|v| v.to_ne_bytes()).collect();
            buffer.write(0, &input).unwrap();
            let mut root = [0u8; 16];
            root[..8].copy_from_slice(&buffer.address().unwrap().to_ne_bytes());
            root[8..12].copy_from_slice(&count.to_ne_bytes());
            let kernel = Rc::new(unsafe { Kernel::new(device, &words, 16).unwrap() });
            for _ in 0..2 {
                unsafe {
                    kernel.dispatch_wait(count.div_ceil(64), &root).unwrap();
                }
                expected
                    .iter_mut()
                    .for_each(|v| *v = v.wrapping_mul(3).wrapping_add(7));
            }
            buffer.write(4, &123u32.to_ne_bytes()).unwrap();
            expected[1] = 123;
            let mut output = vec![0u8; input.len()];
            unsafe {
                buffer.read(0, output.as_mut_ptr(), output.len()).unwrap();
            }
            for (bytes, value) in output.chunks_exact(4).zip(expected) {
                assert_eq!(u32::from_ne_bytes(bytes.try_into().unwrap()), value);
            }
            assert_eq!(
                buffer.write(input.len(), &[1]).unwrap_err().status,
                OUT_OF_RANGE
            );
            assert_eq!(
                unsafe { kernel.dispatch_wait(0, &root) }
                    .unwrap_err()
                    .status,
                INVALID_ARGUMENT
            );
            assert_eq!(
                unsafe { kernel.dispatch_wait(1, &[]) }.unwrap_err().status,
                INVALID_ARGUMENT
            );
            buffer.write(input.len(), &[]).unwrap();
            println!(
                "Verified two dispatches and partial update on {}",
                unsafe { std::ffi::CStr::from_ptr(info.name.as_ptr()) }.to_string_lossy()
            );
            tested += 1;
        }
        assert!(tested > 0, "No execution-capable Vulkan device found");
    }
}
