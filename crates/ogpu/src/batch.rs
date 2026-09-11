//! Host-side one-shot recordings and fence-owned submitted resources.
use super::*;

pub(crate) const COMPUTE_READ: u32 = 1;
pub(crate) const COMPUTE_WRITE: u32 = 2;

pub(crate) const VERTEX_READ: u32 = 4;
pub(crate) const INDIRECT_READ: u32 = 8;
pub(crate) const COLOR_WRITE: u32 = 16;
pub(crate) const TRANSFER_READ: u32 = 32;
pub(crate) const TRANSFER_WRITE: u32 = 64;
pub(crate) const FRAGMENT_READ: u32 = 128;
const GRAPHICS_ACCESS: u32 = VERTEX_READ | INDIRECT_READ | COLOR_WRITE | FRAGMENT_READ;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Access {
    stages: vk::VkPipelineStageFlags,
    flags: vk::VkAccessFlags,
}

fn access(mask: u32) -> Result<Access, Error> {
    if mask == 0 || mask & !255 != 0 {
        return Err(Error::new(INVALID_ARGUMENT, "Invalid access mask"));
    }
    let mut result = Access {
        stages: 0,
        flags: 0,
    };
    for (bit, stage, flags) in [
        (
            COMPUTE_READ,
            vk::VkPipelineStageFlagBits_VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT,
            vk::VkAccessFlagBits_VK_ACCESS_SHADER_READ_BIT,
        ),
        (
            COMPUTE_WRITE,
            vk::VkPipelineStageFlagBits_VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT,
            vk::VkAccessFlagBits_VK_ACCESS_SHADER_WRITE_BIT,
        ),
        (
            VERTEX_READ,
            vk::VkPipelineStageFlagBits_VK_PIPELINE_STAGE_VERTEX_SHADER_BIT,
            vk::VkAccessFlagBits_VK_ACCESS_SHADER_READ_BIT,
        ),
        (
            INDIRECT_READ,
            vk::VkPipelineStageFlagBits_VK_PIPELINE_STAGE_DRAW_INDIRECT_BIT,
            vk::VkAccessFlagBits_VK_ACCESS_INDIRECT_COMMAND_READ_BIT,
        ),
        (
            COLOR_WRITE,
            vk::VkPipelineStageFlagBits_VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT,
            vk::VkAccessFlagBits_VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT,
        ),
        (
            TRANSFER_READ,
            vk::VkPipelineStageFlagBits_VK_PIPELINE_STAGE_TRANSFER_BIT,
            vk::VkAccessFlagBits_VK_ACCESS_TRANSFER_READ_BIT,
        ),
        (
            TRANSFER_WRITE,
            vk::VkPipelineStageFlagBits_VK_PIPELINE_STAGE_TRANSFER_BIT,
            vk::VkAccessFlagBits_VK_ACCESS_TRANSFER_WRITE_BIT,
        ),
        (
            FRAGMENT_READ,
            vk::VkPipelineStageFlagBits_VK_PIPELINE_STAGE_FRAGMENT_SHADER_BIT,
            vk::VkAccessFlagBits_VK_ACCESS_SHADER_READ_BIT,
        ),
    ] {
        if mask & bit != 0 {
            result.stages |= stage;
            result.flags |= flags;
        }
    }
    Ok(result)
}

enum Step {
    Dispatch {
        kernel: Rc<Kernel>,
        groups: u32,
        root: Vec<u8>,
    },
    Barrier {
        source: Access,
        destination: Access,
    },
    Draw {
        raster: Rc<Raster>,
        target: Rc<Target>,
        indirect: Rc<Buffer>,
        offset: u64,
        root: Vec<u8>,
    },
    CopyTarget {
        target: Rc<Target>,
        destination: Rc<Buffer>,
        offset: u64,
    },
}

pub(crate) struct Batch {
    device: Rc<Device>,
    // None means a submission was attempted; even failed attempts are terminal.
    steps: Option<Vec<Step>>,
}

impl Batch {
    pub(crate) fn new(device: Rc<Device>) -> Result<Self, Error> {
        device.ready()?;
        Ok(Self {
            device,
            steps: Some(Vec::new()),
        })
    }

    fn recording(&mut self) -> Result<&mut Vec<Step>, Error> {
        self.device.ready()?;
        self.steps
            .as_mut()
            .ok_or_else(|| Error::new(INVALID_ARGUMENT, "Batch already submitted"))
    }

    pub(crate) fn dispatch(
        &mut self,
        kernel: Rc<Kernel>,
        groups: u32,
        root: &[u8],
    ) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.device, &kernel.device) {
            return Err(Error::new(
                INVALID_ARGUMENT,
                "Kernel belongs to another device",
            ));
        }
        if groups == 0
            || groups > self.device.limits.maxComputeWorkGroupCount[0]
            || root.len() != kernel.push_size as usize
        {
            return Err(Error::new(
                INVALID_ARGUMENT,
                "Invalid dispatch size or argument byte count",
            ));
        }
        self.recording()?.push(Step::Dispatch {
            kernel,
            groups,
            root: root.to_vec(),
        });
        Ok(())
    }

    pub(crate) fn barrier(&mut self, source: u32, destination: u32) -> Result<(), Error> {
        let graphics_access = (source | destination) & GRAPHICS_ACCESS != 0;
        let source = access(source)?;
        let destination = access(destination)?;
        if !self.device.graphics && graphics_access {
            return Err(Error::new(
                UNSUPPORTED,
                "Graphics access on a compute-only queue",
            ));
        }
        self.recording()?.push(Step::Barrier {
            source,
            destination,
        });
        Ok(())
    }

    pub(crate) fn draw(
        &mut self,
        raster: Rc<Raster>,
        target: Rc<Target>,
        indirect: Rc<Buffer>,
        offset: usize,
        root: &[u8],
    ) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.device, &raster.device)
            || !Rc::ptr_eq(&self.device, &target.device)
            || !Rc::ptr_eq(&self.device, &indirect.device)
        {
            return Err(Error::new(
                INVALID_ARGUMENT,
                "Draw objects belong to different devices",
            ));
        }
        if offset % 4 != 0 || root.len() != raster.push_size as usize {
            return Err(Error::new(
                INVALID_ARGUMENT,
                "Invalid indirect offset or raster argument size",
            ));
        }
        indirect.range(offset, 16)?;
        self.recording()?.push(Step::Draw {
            raster,
            target,
            indirect,
            offset: offset as u64,
            root: root.to_vec(),
        });
        Ok(())
    }

    pub(crate) fn copy_target(
        &mut self,
        target: Rc<Target>,
        destination: Rc<Buffer>,
        offset: usize,
    ) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.device, &target.device)
            || !Rc::ptr_eq(&self.device, &destination.device)
        {
            return Err(Error::new(
                INVALID_ARGUMENT,
                "Copy objects belong to different devices",
            ));
        }
        if offset % 4 != 0 {
            return Err(Error::new(INVALID_ARGUMENT, "Unaligned copy destination"));
        }
        destination.range(offset, target.size)?;
        let steps = self.recording()?;
        if !steps.iter().any(
            |step| matches!(step, Step::Draw { target: drawn, .. } if Rc::ptr_eq(drawn, &target)),
        ) {
            return Err(Error::new(
                INVALID_ARGUMENT,
                "Target copy requires an earlier draw in this batch",
            ));
        }
        steps.push(Step::CopyTarget {
            target,
            destination,
            offset: offset as u64,
        });
        Ok(())
    }

    /// # Safety
    /// All addresses recorded in this batch must reference live, aligned, in-bounds
    /// memory on its device; dependencies must make shader accesses race-free. Keep
    /// allocations live and do not perform host accesses until all GPU uses complete.
    /// Calls on this device and its children must be externally serialized.
    pub(crate) unsafe fn submit(&mut self) -> Result<Completion, Error> {
        let steps = self
            .steps
            .take()
            .ok_or_else(|| Error::new(INVALID_ARGUMENT, "Batch already submitted"))?;
        self.device.ready()?;
        let mut completion = Completion {
            device: self.device.clone(),
            steps,
            pool: ptr::null_mut(),
            fence: ptr::null_mut(),
            pending: false,
            outcome: None,
        };
        // SAFETY: the caller guarantees shader semantics/lifetimes. Preparation retains
        // every kernel, and the completion owns partial construction immediately.
        unsafe {
            let command = completion.prepare()?;
            let d = &completion.device;
            let submit = vk::VkSubmitInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SUBMIT_INFO,
                commandBufferCount: 1,
                pCommandBuffers: &command,
                ..Default::default()
            };
            let status = (d.f.vkQueueSubmit.unwrap())(d.queue, 1, &submit, completion.fence);
            if status == vk::VkResult_VK_SUCCESS {
                // No fallible operation between accepted submission and recording ownership.
                completion.pending = true;
            } else {
                // Vulkan guarantees unchanged submission state for OOM, and cleanup on
                // device loss. An unexpected error has no such guarantee: drain the queue,
                // not the fence (which might never have been submitted).
                if !submission_is_unaccepted(status) {
                    let drained = drain(|| (d.f.vkQueueWaitIdle.unwrap())(d.queue));
                    if drained == vk::VkResult_VK_ERROR_DEVICE_LOST {
                        d.result("vkQueueWaitIdle after submit error", drained)?;
                    }
                }
                d.result("vkQueueSubmit", status)?;
            }
        }
        Ok(completion)
    }
}

fn submission_is_unaccepted(status: vk::VkResult) -> bool {
    matches!(
        status,
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
            | vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY
            | vk::VkResult_VK_ERROR_DEVICE_LOST
    )
}

pub(crate) struct Completion {
    device: Rc<Device>,
    // Retained until command pool destruction, even if public kernel handles are gone.
    steps: Vec<Step>,
    pool: vk::VkCommandPool,
    fence: vk::VkFence,
    pending: bool,
    outcome: Option<vk::VkResult>,
}

impl Completion {
    unsafe fn prepare(&mut self) -> Result<vk::VkCommandBuffer, Error> {
        let d = &self.device;
        // SAFETY: Vulkan pointers refer to initialized local structures. This object
        // owns each successful allocation before another fallible call can occur.
        unsafe {
            let fence = vk::VkFenceCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_FENCE_CREATE_INFO,
                ..Default::default()
            };
            d.result(
                "vkCreateFence",
                (d.f.vkCreateFence.unwrap())(d.handle, &fence, ptr::null(), &mut self.fence),
            )?;
            let pool = vk::VkCommandPoolCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO,
                queueFamilyIndex: d.family,
                ..Default::default()
            };
            d.result(
                "vkCreateCommandPool",
                (d.f.vkCreateCommandPool.unwrap())(d.handle, &pool, ptr::null(), &mut self.pool),
            )?;
            let allocate = vk::VkCommandBufferAllocateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
                commandPool: self.pool,
                level: vk::VkCommandBufferLevel_VK_COMMAND_BUFFER_LEVEL_PRIMARY,
                commandBufferCount: 1,
                ..Default::default()
            };
            let mut command = ptr::null_mut();
            d.result(
                "vkAllocateCommandBuffers",
                (d.f.vkAllocateCommandBuffers.unwrap())(d.handle, &allocate, &mut command),
            )?;
            let begin = vk::VkCommandBufferBeginInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,
                ..Default::default()
            };
            d.result(
                "vkBeginCommandBuffer",
                (d.f.vkBeginCommandBuffer.unwrap())(command, &begin),
            )?;
            barrier(
                d,
                command,
                vk::VkPipelineStageFlagBits_VK_PIPELINE_STAGE_HOST_BIT,
                vk::VkPipelineStageFlagBits_VK_PIPELINE_STAGE_ALL_COMMANDS_BIT,
                vk::VkAccessFlagBits_VK_ACCESS_HOST_WRITE_BIT,
                vk::VkAccessFlagBits_VK_ACCESS_MEMORY_READ_BIT
                    | vk::VkAccessFlagBits_VK_ACCESS_MEMORY_WRITE_BIT,
            );
            for step in &self.steps {
                match step {
                    Step::Dispatch {
                        kernel,
                        groups,
                        root,
                    } => {
                        (d.f.vkCmdBindPipeline.unwrap())(
                            command,
                            vk::VkPipelineBindPoint_VK_PIPELINE_BIND_POINT_COMPUTE,
                            kernel.pipeline,
                        );
                        if kernel.push_size != 0 {
                            (d.f.vkCmdPushConstants.unwrap())(
                                command,
                                kernel.layout,
                                vk::VkShaderStageFlagBits_VK_SHADER_STAGE_COMPUTE_BIT,
                                0,
                                kernel.push_size,
                                root.as_ptr().cast(),
                            );
                        }
                        (d.f.vkCmdDispatch.unwrap())(command, *groups, 1, 1);
                    }
                    Step::Barrier {
                        source,
                        destination,
                    } => barrier(
                        d,
                        command,
                        source.stages,
                        destination.stages,
                        source.flags,
                        destination.flags,
                    ),
                    Step::Draw {
                        raster,
                        target,
                        indirect,
                        offset,
                        root,
                    } => target.draw(command, raster, indirect, *offset, root),
                    Step::CopyTarget {
                        target,
                        destination,
                        offset,
                    } => target.copy_to(command, destination, *offset),
                }
            }
            barrier(
                d,
                command,
                vk::VkPipelineStageFlagBits_VK_PIPELINE_STAGE_ALL_COMMANDS_BIT,
                vk::VkPipelineStageFlagBits_VK_PIPELINE_STAGE_HOST_BIT,
                vk::VkAccessFlagBits_VK_ACCESS_MEMORY_WRITE_BIT,
                vk::VkAccessFlagBits_VK_ACCESS_HOST_READ_BIT,
            );
            d.result(
                "vkEndCommandBuffer",
                (d.f.vkEndCommandBuffer.unwrap())(command),
            )?;
            Ok(command)
        }
    }

    pub(crate) fn wait(&mut self) -> Result<(), Error> {
        if self.pending {
            let d = &self.device;
            let status = if d.lost.get() {
                vk::VkResult_VK_ERROR_DEVICE_LOST
            } else {
                // SAFETY: this completion owns an accepted submission's fence and all
                // resources. Wait errors retain them until draining establishes safety.
                drain(|| unsafe {
                    (d.f.vkWaitForFences.unwrap())(d.handle, 1, &self.fence, vk::VK_TRUE, u64::MAX)
                })
            };
            self.pending = false;
            self.outcome = Some(status);
        }
        self.device.result(
            "vkWaitForFences",
            self.outcome.unwrap_or(vk::VkResult_VK_SUCCESS),
        )
    }
}

impl Drop for Completion {
    fn drop(&mut self) {
        // No cancellation: command resources and retained kernels must outlive GPU use.
        let _ = self.wait();
        unsafe {
            let d = &self.device;
            if !self.pool.is_null() {
                (d.f.vkDestroyCommandPool.unwrap())(d.handle, self.pool, ptr::null());
            }
            if !self.fence.is_null() {
                (d.f.vkDestroyFence.unwrap())(d.handle, self.fence, ptr::null());
            }
        }
    }
}

unsafe fn barrier(
    d: &Device,
    command: vk::VkCommandBuffer,
    source_stage: vk::VkPipelineStageFlags,
    destination_stage: vk::VkPipelineStageFlags,
    source: vk::VkAccessFlags,
    destination: vk::VkAccessFlags,
) {
    let memory = vk::VkMemoryBarrier {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_BARRIER,
        srcAccessMask: source,
        dstAccessMask: destination,
        ..Default::default()
    };
    // SAFETY: command is recording; stages/accesses are supported on the compute queue.
    unsafe {
        (d.f.vkCmdPipelineBarrier.unwrap())(
            command,
            source_stage,
            destination_stage,
            0,
            1,
            &memory,
            0,
            ptr::null(),
            0,
            ptr::null(),
        );
    }
}

#[cfg(test)]
#[path = "batch_tests.rs"]
mod tests;
