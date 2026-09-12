//! Host-side one-shot recordings and timeline-owned submitted resources.
use super::*;

pub(crate) const COMPUTE_READ: u32 = 1;
pub(crate) const COMPUTE_WRITE: u32 = 2;

pub(crate) const VERTEX_READ: u32 = 4;
pub(crate) const INDIRECT_READ: u32 = 8;
pub(crate) const COLOR_WRITE: u32 = 16;
pub(crate) const TRANSFER_READ: u32 = 32;
pub(crate) const TRANSFER_WRITE: u32 = 64;
pub(crate) const FRAGMENT_READ: u32 = 128;
pub(crate) const COLOR_READ: u32 = 256;
pub(crate) const CLEAR: u32 = 0;
pub(crate) const LOAD: u32 = 1;
const GRAPHICS_ACCESS: u32 = VERTEX_READ | INDIRECT_READ | COLOR_WRITE | FRAGMENT_READ | COLOR_READ;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Access {
    stages: vk::VkPipelineStageFlags2,
    flags: vk::VkAccessFlags2,
}

fn access(mask: u32) -> Result<Access, Error> {
    if mask == 0 || mask & !511 != 0 {
        return Err(Error::new(INVALID_ARGUMENT, "Invalid access mask"));
    }
    let mut result = Access {
        stages: 0,
        flags: 0,
    };
    for (bit, stage, flags) in [
        (
            COLOR_READ,
            vk::VK_PIPELINE_STAGE_2_COLOR_ATTACHMENT_OUTPUT_BIT,
            vk::VK_ACCESS_2_COLOR_ATTACHMENT_READ_BIT,
        ),
        (
            COMPUTE_READ,
            vk::VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT,
            vk::VK_ACCESS_2_SHADER_READ_BIT,
        ),
        (
            COMPUTE_WRITE,
            vk::VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT,
            vk::VK_ACCESS_2_SHADER_WRITE_BIT,
        ),
        (
            VERTEX_READ,
            vk::VK_PIPELINE_STAGE_2_VERTEX_SHADER_BIT,
            vk::VK_ACCESS_2_SHADER_READ_BIT,
        ),
        (
            INDIRECT_READ,
            vk::VK_PIPELINE_STAGE_2_DRAW_INDIRECT_BIT,
            vk::VK_ACCESS_2_INDIRECT_COMMAND_READ_BIT,
        ),
        (
            COLOR_WRITE,
            vk::VK_PIPELINE_STAGE_2_COLOR_ATTACHMENT_OUTPUT_BIT,
            vk::VK_ACCESS_2_COLOR_ATTACHMENT_WRITE_BIT,
        ),
        (
            TRANSFER_READ,
            vk::VK_PIPELINE_STAGE_2_TRANSFER_BIT,
            vk::VK_ACCESS_2_TRANSFER_READ_BIT,
        ),
        (
            TRANSFER_WRITE,
            vk::VK_PIPELINE_STAGE_2_TRANSFER_BIT,
            vk::VK_ACCESS_2_TRANSFER_WRITE_BIT,
        ),
        (
            FRAGMENT_READ,
            vk::VK_PIPELINE_STAGE_2_FRAGMENT_SHADER_BIT,
            vk::VK_ACCESS_2_SHADER_READ_BIT,
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
    DiscardTarget(Rc<Target>),
    BindImages(Rc<ImageTable>),
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
        load: u32,
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
    timed: bool,
    retained: Vec<Rc<Buffer>>,
}

impl Batch {
    pub(crate) fn discard_target(&mut self, target: Rc<Target>) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.device, &target.device) {
            return Err(Error::new(
                INVALID_ARGUMENT,
                "Target belongs to another device",
            ));
        }
        self.recording()?.push(Step::DiscardTarget(target));
        Ok(())
    }

    pub(crate) fn bind_images(&mut self, table: Rc<ImageTable>) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.device, &table.device) {
            return Err(Error::new(
                INVALID_ARGUMENT,
                "Image table belongs to another device",
            ));
        }
        self.recording()?.push(Step::BindImages(table));
        Ok(())
    }

    pub(crate) fn new(device: Rc<Device>) -> Result<Self, Error> {
        device.ready()?;
        Ok(Self {
            device,
            steps: Some(Vec::new()),
            timed: false,
            retained: Vec::new(),
        })
    }

    fn recording(&mut self) -> Result<&mut Vec<Step>, Error> {
        self.device.ready()?;
        self.steps
            .as_mut()
            .ok_or_else(|| Error::new(INVALID_ARGUMENT, "Batch already submitted"))
    }

    pub(crate) fn enable_timing(&mut self) -> Result<(), Error> {
        self.recording()?;
        self.device.timing_info()?;
        self.timed = true;
        Ok(())
    }

    pub(crate) fn retain_buffer(&mut self, buffer: Rc<Buffer>) -> Result<(), Error> {
        self.recording()?;
        if !Rc::ptr_eq(&self.device, &buffer.device) {
            return Err(Error::new(
                INVALID_ARGUMENT,
                "Buffer belongs to another device",
            ));
        }
        if !self.retained.iter().any(|b| Rc::ptr_eq(b, &buffer)) {
            self.retained.push(buffer);
        }
        Ok(())
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
        load: u32,
    ) -> Result<(), Error> {
        if !matches!(load, CLEAR | LOAD) {
            return Err(Error::new(
                INVALID_ARGUMENT,
                "Invalid attachment load operation",
            ));
        }
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
            load,
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
        let retained = std::mem::take(&mut self.retained);
        self.device.ready()?;
        let mut completion = Completion {
            device: self.device.clone(),
            steps,
            pool: ptr::null_mut(),
            timeline_value: 0,
            pending: false,
            outcome: None,
            timed: self.timed,
            queries: ptr::null_mut(),
            elapsed: None,
            _retained: retained,
        };
        // SAFETY: the caller guarantees shader semantics/lifetimes. Preparation retains
        // every kernel, and the completion owns partial construction immediately.
        unsafe {
            let command = completion.prepare()?;
            let d = &completion.device;
            let command_info = vk::VkCommandBufferSubmitInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_BUFFER_SUBMIT_INFO,
                commandBuffer: command,
                ..Default::default()
            };
            let timeline_value = d.reserve_timeline()?;
            let signal = vk::VkSemaphoreSubmitInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_SUBMIT_INFO,
                semaphore: d.timeline,
                value: timeline_value,
                stageMask: vk::VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
                ..Default::default()
            };
            let submit = vk::VkSubmitInfo2 {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SUBMIT_INFO_2,
                commandBufferInfoCount: 1,
                pCommandBufferInfos: &command_info,
                signalSemaphoreInfoCount: 1,
                pSignalSemaphoreInfos: &signal,
                ..Default::default()
            };
            completion.timeline_value = timeline_value;
            let status = (d.f.vkQueueSubmit2.unwrap())(d.queue, 1, &submit, ptr::null_mut());
            if status == vk::VkResult_VK_SUCCESS {
                // No fallible operation between accepted submission and recording ownership.
                completion.pending = true;
            } else {
                // Vulkan guarantees unchanged submission state for OOM, and cleanup on
                // device loss. An unexpected error has no such guarantee: drain the queue,
                // not the timeline value (which might never have been signaled).
                if !submission_is_unaccepted(status) {
                    let drained = drain(|| (d.f.vkQueueWaitIdle.unwrap())(d.queue));
                    if drained == vk::VkResult_VK_ERROR_DEVICE_LOST {
                        d.result("vkQueueWaitIdle after submit error", drained)?;
                    }
                }
                d.result("vkQueueSubmit2", status)?;
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
    timeline_value: u64,
    pending: bool,
    outcome: Option<vk::VkResult>,
    timed: bool,
    queries: vk::VkQueryPool,
    elapsed: Option<f64>,
    // Explicit ownership assistance, independent of shader access declarations.
    _retained: Vec<Rc<Buffer>>,
}

impl Completion {
    // SUCCESS means this submission's accesses have finished. A transient query
    // error gives no release permission and leaves the completion pending.
    pub(crate) fn poll(&mut self) -> Result<bool, Error> {
        if self.pending {
            let d = &self.device;
            let status = if d.lost.get() {
                vk::VkResult_VK_ERROR_DEVICE_LOST
            } else {
                unsafe { self.timeline_wait(0) }
            };
            match status {
                vk::VkResult_VK_TIMEOUT => return Ok(false),
                vk::VkResult_VK_SUCCESS | vk::VkResult_VK_ERROR_DEVICE_LOST => {
                    self.pending = false;
                    self.outcome = Some(status);
                }
                _ => return d.result("vkWaitSemaphores (poll)", status).map(|()| false),
            }
        }
        self.device.result(
            "completion poll",
            self.outcome.unwrap_or(vk::VkResult_VK_SUCCESS),
        )?;
        Ok(true)
    }

    unsafe fn timeline_wait(&self, timeout: u64) -> vk::VkResult {
        let wait = vk::VkSemaphoreWaitInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_WAIT_INFO,
            semaphoreCount: 1,
            pSemaphores: &self.device.timeline,
            pValues: &self.timeline_value,
            ..Default::default()
        };
        // SAFETY: accepted submission, live semaphore, serialized host access.
        unsafe { (self.device.f.vkWaitSemaphores.unwrap())(self.device.handle, &wait, timeout) }
    }

    unsafe fn prepare(&mut self) -> Result<vk::VkCommandBuffer, Error> {
        let d = &self.device;
        // SAFETY: Vulkan pointers refer to initialized local structures. This object
        // owns each successful allocation before another fallible call can occur.
        unsafe {
            if self.timed {
                let info = vk::VkQueryPoolCreateInfo {
                    sType: vk::VkStructureType_VK_STRUCTURE_TYPE_QUERY_POOL_CREATE_INFO,
                    queryType: vk::VkQueryType_VK_QUERY_TYPE_TIMESTAMP,
                    queryCount: 2,
                    ..Default::default()
                };
                d.result(
                    "vkCreateQueryPool",
                    (d.f.vkCreateQueryPool.unwrap())(
                        d.handle,
                        &info,
                        ptr::null(),
                        &mut self.queries,
                    ),
                )?;
            }
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
            if self.timed {
                (d.f.vkCmdResetQueryPool.unwrap())(command, self.queries, 0, 2);
                (d.f.vkCmdWriteTimestamp2.unwrap())(
                    command,
                    vk::VK_PIPELINE_STAGE_2_TOP_OF_PIPE_BIT,
                    self.queries,
                    0,
                );
            }
            barrier(
                d,
                command,
                vk::VK_PIPELINE_STAGE_2_HOST_BIT,
                vk::VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
                vk::VK_ACCESS_2_HOST_WRITE_BIT,
                vk::VK_ACCESS_2_MEMORY_READ_BIT | vk::VK_ACCESS_2_MEMORY_WRITE_BIT,
            );
            for step in &self.steps {
                match step {
                    Step::DiscardTarget(target) => target.discard(command),
                    Step::BindImages(table) => table.bind(command),
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
                        push_data(d, command, root);
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
                        load,
                    } => target.draw(command, raster, indirect, *offset, root, *load),
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
                vk::VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
                vk::VK_PIPELINE_STAGE_2_HOST_BIT,
                vk::VK_ACCESS_2_MEMORY_WRITE_BIT,
                vk::VK_ACCESS_2_HOST_READ_BIT,
            );
            if self.timed {
                (d.f.vkCmdWriteTimestamp2.unwrap())(
                    command,
                    vk::VK_PIPELINE_STAGE_2_BOTTOM_OF_PIPE_BIT,
                    self.queries,
                    1,
                );
            }
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
                // SAFETY: this completion owns an accepted submission's timeline value and all
                // resources. Wait errors retain them until draining establishes safety.
                drain(|| unsafe { self.timeline_wait(u64::MAX) })
            };
            self.pending = false;
            self.outcome = Some(status);
        }
        self.device.result(
            "vkWaitSemaphores",
            self.outcome.unwrap_or(vk::VkResult_VK_SUCCESS),
        )
    }

    pub(crate) fn elapsed_ns(&mut self) -> Result<f64, Error> {
        if !self.timed {
            return Err(Error::new(INVALID_ARGUMENT, "Completion was not timed"));
        }
        let outcome = self.outcome.ok_or_else(|| {
            Error::new(
                INVALID_ARGUMENT,
                "Timing requires a successful completion wait or poll",
            )
        })?;
        self.device
            .result("Timing requires confirmed successful completion", outcome)?;
        let (period, bits) = self.device.timing_info()?;
        if let Some(elapsed) = self.elapsed {
            return Ok(elapsed);
        }
        let mut ticks = [0u64; 2];
        // SAFETY: the successful timeline wait establishes completion of reset and both
        // writes. This completion uniquely owns the pool and output is two aligned u64s.
        let status = unsafe {
            (self.device.f.vkGetQueryPoolResults.unwrap())(
                self.device.handle,
                self.queries,
                0,
                2,
                std::mem::size_of_val(&ticks),
                ticks.as_mut_ptr().cast(),
                8,
                vk::VkQueryResultFlagBits_VK_QUERY_RESULT_64_BIT,
            )
        };
        self.device.result("vkGetQueryPoolResults", status)?;
        let elapsed = timestamp_delta_ns(ticks[0], ticks[1], bits, period);
        self.elapsed = Some(elapsed);
        Ok(elapsed)
    }
}

fn timestamp_delta_ns(start: u64, end: u64, bits: u32, period: f64) -> f64 {
    // bits/period were checked by timing_info. Avoid shifting by 64; wrapping_sub
    // also handles the full-width counter. Additional whole wraps are unknowable.
    let mask = u64::MAX >> (64 - bits);
    (end.wrapping_sub(start) & mask) as f64 * period
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
            if !self.queries.is_null() {
                (d.f.vkDestroyQueryPool.unwrap())(d.handle, self.queries, ptr::null());
            }
        }
    }
}

pub(super) unsafe fn barrier(
    d: &Device,
    command: vk::VkCommandBuffer,
    source_stage: vk::VkPipelineStageFlags2,
    destination_stage: vk::VkPipelineStageFlags2,
    source: vk::VkAccessFlags2,
    destination: vk::VkAccessFlags2,
) {
    let memory = vk::VkMemoryBarrier2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_BARRIER_2,
        srcStageMask: source_stage,
        srcAccessMask: source,
        dstStageMask: destination_stage,
        dstAccessMask: destination,
        ..Default::default()
    };
    let dependency = vk::VkDependencyInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEPENDENCY_INFO,
        memoryBarrierCount: 1,
        pMemoryBarriers: &memory,
        ..Default::default()
    };
    // SAFETY: command is recording; stages/accesses match enabled queue capabilities.
    unsafe {
        (d.f.vkCmdPipelineBarrier2.unwrap())(command, &dependency);
    }
}

pub(super) unsafe fn push_data(d: &Device, command: vk::VkCommandBuffer, root: &[u8]) {
    if root.is_empty() {
        return;
    }
    let info = vk::VkPushDataInfoEXT {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PUSH_DATA_INFO_EXT,
        data: vk::VkHostAddressRangeConstEXT {
            address: root.as_ptr().cast(),
            size: root.len(),
        },
        ..Default::default()
    };
    // SAFETY: root size/alignment was validated; Vulkan copies bytes during recording.
    unsafe {
        (d.f.vkCmdPushDataEXT.unwrap())(command, &info);
    }
}

#[cfg(test)]
#[path = "batch_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "timing_tests.rs"]
mod timing_tests;

#[cfg(test)]
#[path = "retirement_tests.rs"]
mod retirement_tests;
