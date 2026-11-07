//! Explicit native command storage. No pending registry, resource retention,
//! hot-path allocator, implicit signal, or host wait.
use super::*;
#[path = "generated_commands.rs"]
mod generated_commands;
use std::{alloc::Layout, cell::Cell, ffi::c_void};
#[path = "dependencies.rs"]
pub(super) mod dependencies;
#[path = "secondary.rs"]
mod secondary;
#[path = "splits.rs"]
mod splits;
#[path = "submissions.rs"]
pub(super) mod submissions;
pub(in crate::foundation) use dependencies::{
    barrier_requirements as barrier_scratch, split_requirements as split_scratch,
};
pub(in crate::foundation) use secondary::{execute_scratch, inheritance_scratch};
pub use splits::Split;
pub(in crate::foundation) use submissions::requirements as submit_scratch;

pub(in crate::foundation) fn vertex_scratch(count: u32) -> Result<HostRequirements, Status> {
    if count == 0 {
        return Ok(HostRequirements {
            size: 0,
            alignment: 1,
        });
    }
    let layout =
        Layout::array::<vk::VkBindVertexBuffer3InfoKHR>(count as usize).map_err(|_| INVALID)?;
    Ok(HostRequirements {
        size: layout.size() as u64,
        alignment: layout.align() as u64,
    })
}

pub struct Arena {
    device: *const Device,
    pool: vk::VkCommandPool,
    domain: u32,
    level: u32,
    // Box addresses remain stable when the explicitly reserved slot array grows.
    #[allow(clippy::vec_box)]
    slots: Vec<Box<List>>,
    used: usize,
    usable: bool,
}
#[derive(Clone, Copy)]
struct ActiveQuery {
    pool: *const QueryPool,
    index: u32,
    inside: bool,
    flags: u32,
}
pub struct List {
    device: *const Device,
    command: vk::VkCommandBuffer,
    domain: u32,
    level: u32,
    continuation: bool,
    inherited_occlusion: u32,
    inherited_query_flags: u32,
    inherited_statistics: u64,
    inherited_heaps: u32,
    secondary_contents: Cell<bool>,
    mode: u32,
    state: Cell<u32>, // 0 invalid, 1 recording, 2 executable, 3 consumed one-shot
    error: Cell<Status>,
    executable: Cell<*const Executable>,
    graphics: Cell<*const Executable>,
    rendering: Cell<bool>,
    view_mask: Cell<u32>,
    render_colors: Cell<u32>,
    index_count: Cell<Option<u64>>,
    active_queries: [Cell<Option<ActiveQuery>>; 2],
}

impl Arena {
    pub(in crate::foundation) fn create(d: &Device, desc: &ArenaDesc) -> Result<Box<Self>, Status> {
        d.ready()?;
        desc.header.validate::<ArenaDesc>(ARENA_DESC)?;
        if desc.level > 1 || desc.reserved != 0 {
            return Err(UNSUPPORTED);
        }
        let queue = d
            .snapshot
            .queues
            .iter()
            .find(|q| q.domain == desc.domain && q.count > 0)
            .ok_or(INVALID)?;
        if queue.flags & (GRAPHICS | COMPUTE | TRANSFER) == 0 {
            return Err(UNSUPPORTED);
        }
        let mut result = Box::new(Self {
            device: d,
            pool: ptr::null_mut(),
            domain: desc.domain,
            level: desc.level,
            slots: Vec::new(),
            used: 0,
            usable: true,
        });
        let info = vk::VkCommandPoolCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO,
            queueFamilyIndex: desc.domain,
            ..Default::default()
        };
        let mut pool = ptr::null_mut();
        // SAFETY: validated domain of the live device, synchronous output adoption.
        unsafe {
            d.result((d.f.vkCreateCommandPool.unwrap())(
                d.handle,
                &info,
                ptr::null(),
                &mut pool,
            ))?;
        }
        result.pool = pool;
        result.reserve(desc.list_capacity)?;
        Ok(result)
    }
    pub(in crate::foundation) fn reserve(&mut self, count: u32) -> Result<(), Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        let count = count as usize;
        if count <= self.slots.len() {
            return Ok(());
        }
        let extra = count - self.slots.len();
        self.slots
            .try_reserve_exact(extra)
            .map_err(|_| OUT_OF_MEMORY)?;
        let mut slots = Vec::new();
        slots.try_reserve_exact(extra).map_err(|_| OUT_OF_MEMORY)?;
        let mut commands = Vec::new();
        commands
            .try_reserve_exact(extra)
            .map_err(|_| OUT_OF_MEMORY)?;
        commands.resize(extra, ptr::null_mut());
        for _ in 0..extra {
            slots.push(Box::new(List {
                device: self.device,
                command: ptr::null_mut(),
                domain: self.domain,
                level: self.level,
                continuation: false,
                inherited_occlusion: 0,
                inherited_query_flags: 0,
                inherited_statistics: 0,
                inherited_heaps: 0,
                secondary_contents: Cell::new(false),
                mode: 0,
                state: Cell::new(0),
                error: Cell::new(OK),
                executable: Cell::new(ptr::null()),
                graphics: Cell::new(ptr::null()),
                rendering: Cell::new(false),
                view_mask: Cell::new(0),
                render_colors: Cell::new(0),
                index_count: Cell::new(None),
                active_queries: [const { Cell::new(None) }; 2],
            }));
        }
        let info = vk::VkCommandBufferAllocateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
            commandPool: self.pool,
            level: self.level,
            commandBufferCount: extra as u32,
            ..Default::default()
        };
        // SAFETY: native allocation is all-or-nothing. Failure output isn't adopted.
        unsafe {
            d.result((d.f.vkAllocateCommandBuffers.unwrap())(
                d.handle,
                &info,
                commands.as_mut_ptr(),
            ))?;
        }
        for (slot, command) in slots.iter_mut().zip(commands) {
            slot.command = command;
        }
        self.slots.extend(slots);
        Ok(())
    }
    pub(in crate::foundation) fn reset(&mut self, release: bool) -> Result<(), Status> {
        // Invalidate even when reset fails: no potentially stale executable list.
        self.usable = false;
        for slot in &self.slots {
            slot.state.set(0);
        }
        let d = unsafe { &*self.device };
        d.ready()?;
        let flags = if release {
            vk::VkCommandPoolResetFlagBits_VK_COMMAND_POOL_RESET_RELEASE_RESOURCES_BIT
        } else {
            0
        };
        // SAFETY: the caller proves no pending use and excludes all pool access.
        unsafe {
            d.result((d.f.vkResetCommandPool.unwrap())(
                d.handle, self.pool, flags,
            ))?;
        }
        self.used = 0;
        self.usable = true;
        Ok(())
    }
    pub(in crate::foundation) fn trim(&mut self, retained: u32) -> Result<(), Status> {
        if retained as usize > self.slots.len() {
            return Err(INVALID);
        }
        self.reset(true)?;
        let d = unsafe { &*self.device };
        while self.slots.len() > retained as usize {
            let slot = self.slots.pop().unwrap();
            unsafe {
                (d.f.vkFreeCommandBuffers.unwrap())(d.handle, self.pool, 1, &slot.command);
            }
        }
        self.slots.shrink_to_fit();
        unsafe {
            (d.f.vkTrimCommandPool.unwrap())(d.handle, self.pool, 0);
        }
        Ok(())
    }
    pub(in crate::foundation) fn begin(
        &mut self,
        desc: &RecordingDesc,
    ) -> Result<*mut List, Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        desc.header.validate::<RecordingDesc>(RECORDING_DESC)?;
        if desc.level != self.level {
            return Err(INVALID);
        }
        let mut prepared = unsafe { secondary::inheritance(d, self.domain, desc)? };
        let continuation = prepared.rendering.is_some();
        let inherited_heaps = prepared.heap_mask();
        let view_mask = prepared.rendering.as_ref().map_or(0, |r| r.viewMask);
        if continuation {
            stages(d, self.domain, 24, false)?;
        }
        let render_colors = prepared
            .rendering
            .as_ref()
            .map_or(0, |r| r.colorAttachmentCount);
        let inheritance = prepared.link();
        let mut flags = match desc.replay_mode {
            1 => vk::VkCommandBufferUsageFlagBits_VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT,
            2 => 0,
            3 => vk::VkCommandBufferUsageFlagBits_VK_COMMAND_BUFFER_USAGE_SIMULTANEOUS_USE_BIT,
            _ => return Err(INVALID),
        };
        if continuation {
            flags |=
                vk::VkCommandBufferUsageFlagBits_VK_COMMAND_BUFFER_USAGE_RENDER_PASS_CONTINUE_BIT;
        }
        if !self.usable {
            return Err(INVALID);
        }
        let slot = self.slots.get_mut(self.used).ok_or(CAPACITY)?;
        self.used += 1;
        slot.state.set(0);
        slot.error.set(OK);
        slot.executable.set(ptr::null());
        slot.graphics.set(ptr::null());
        slot.continuation = continuation;
        slot.rendering.set(slot.continuation);
        slot.view_mask.set(view_mask);
        slot.render_colors.set(render_colors);
        slot.secondary_contents.set(false);
        slot.inherited_occlusion = inheritance.occlusionQueryEnable;
        slot.inherited_query_flags = inheritance.queryFlags;
        slot.inherited_statistics = u64::from(inheritance.pipelineStatistics);
        slot.inherited_heaps = inherited_heaps;
        slot.index_count.set(None);
        for query in &slot.active_queries {
            query.set(None);
        }
        slot.mode = desc.replay_mode;
        let info = vk::VkCommandBufferBeginInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,
            flags,
            pInheritanceInfo: if self.level == 1 {
                inheritance
            } else {
                ptr::null()
            },
            ..Default::default()
        };
        unsafe {
            d.result((d.f.vkBeginCommandBuffer.unwrap())(slot.command, &info))?;
        }
        slot.state.set(1);
        Ok(ptr::from_mut(&mut **slot))
    }
}
impl Drop for Arena {
    fn drop(&mut self) {
        if !self.pool.is_null() {
            let d = unsafe { &*self.device };
            // SAFETY: caller excludes pending/host use. Deliberately no idle wait.
            unsafe {
                (d.f.vkDestroyCommandPool.unwrap())(d.handle, self.pool, ptr::null());
            }
        }
    }
}

const COPY_SCOPE: u64 = 512;
const INDEX_SCOPE: u64 = 1024;
const FETCH_SCOPE: u64 = 2048;
const EARLY_SCOPE: u64 = 4096;
const LATE_SCOPE: u64 = 8192;
const CLEAR_SCOPE: u64 = 16384;
const RESOLVE_SCOPE: u64 = 32768;
const BLIT_SCOPE: u64 = 65536;
const INPUT_SCOPE: u64 = 131072;
const GRAPHICS_SCOPE: u64 = 262144;
const PRE_RASTER_SCOPE: u64 = 524288;
const KNOWN_SCOPES: u64 = (1 << 27) - 1;
const RASTER_SCOPES: u64 = 8
    | 16
    | 64
    | 128
    | INDEX_SCOPE
    | FETCH_SCOPE
    | EARLY_SCOPE
    | LATE_SCOPE
    | INPUT_SCOPE
    | GRAPHICS_SCOPE
    | PRE_RASTER_SCOPE
    | MESH_STAGE
    | TASK_STAGE
    | SHADING_RATE_STAGE
    | TESS_CONTROL_STAGE
    | TESS_EVALUATION_STAGE
    | GEOMETRY_STAGE;

fn stage_mask(flags: u32, raster: bool, mask: u64, semaphore: bool) -> Result<u64, Status> {
    if mask & !KNOWN_SCOPES != 0 {
        return Err(UNSUPPORTED);
    }
    if semaphore && mask & 256 != 0 {
        return Err(INVALID);
    }
    if mask & RASTER_SCOPES != 0 && (flags & GRAPHICS == 0 || !raster)
        || mask & (RESOLVE_SCOPE | BLIT_SCOPE) != 0 && flags & GRAPHICS == 0
        || mask & 4 != 0 && flags & COMPUTE == 0
        || mask & (32 | PREPROCESS_STAGE) != 0 && flags & (COMPUTE | GRAPHICS) == 0
        || mask & (2 | COPY_SCOPE | CLEAR_SCOPE) != 0
            && flags & (COMPUTE | GRAPHICS | TRANSFER) == 0
    {
        return Err(UNSUPPORTED);
    }
    let bits = [
        vk::VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
        vk::VK_PIPELINE_STAGE_2_ALL_TRANSFER_BIT,
        vk::VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT,
        vk::VK_PIPELINE_STAGE_2_VERTEX_SHADER_BIT,
        vk::VK_PIPELINE_STAGE_2_FRAGMENT_SHADER_BIT,
        vk::VK_PIPELINE_STAGE_2_DRAW_INDIRECT_BIT,
        vk::VK_PIPELINE_STAGE_2_COLOR_ATTACHMENT_OUTPUT_BIT,
        vk::VK_PIPELINE_STAGE_2_EARLY_FRAGMENT_TESTS_BIT
            | vk::VK_PIPELINE_STAGE_2_LATE_FRAGMENT_TESTS_BIT,
        vk::VK_PIPELINE_STAGE_2_HOST_BIT,
        vk::VK_PIPELINE_STAGE_2_COPY_BIT,
        vk::VK_PIPELINE_STAGE_2_INDEX_INPUT_BIT,
        vk::VK_PIPELINE_STAGE_2_VERTEX_ATTRIBUTE_INPUT_BIT,
        vk::VK_PIPELINE_STAGE_2_EARLY_FRAGMENT_TESTS_BIT,
        vk::VK_PIPELINE_STAGE_2_LATE_FRAGMENT_TESTS_BIT,
        vk::VK_PIPELINE_STAGE_2_CLEAR_BIT,
        vk::VK_PIPELINE_STAGE_2_RESOLVE_BIT,
        vk::VK_PIPELINE_STAGE_2_BLIT_BIT,
        vk::VK_PIPELINE_STAGE_2_VERTEX_INPUT_BIT,
        vk::VK_PIPELINE_STAGE_2_ALL_GRAPHICS_BIT,
        vk::VK_PIPELINE_STAGE_2_PRE_RASTERIZATION_SHADERS_BIT,
        vk::VK_PIPELINE_STAGE_2_MESH_SHADER_BIT_EXT,
        vk::VK_PIPELINE_STAGE_2_TASK_SHADER_BIT_EXT,
        vk::VK_PIPELINE_STAGE_2_COMMAND_PREPROCESS_BIT_EXT,
        vk::VK_PIPELINE_STAGE_2_FRAGMENT_SHADING_RATE_ATTACHMENT_BIT_KHR,
        vk::VK_PIPELINE_STAGE_2_TESSELLATION_CONTROL_SHADER_BIT,
        vk::VK_PIPELINE_STAGE_2_TESSELLATION_EVALUATION_SHADER_BIT,
        vk::VK_PIPELINE_STAGE_2_GEOMETRY_SHADER_BIT,
    ];
    Ok(bits.iter().enumerate().fold(0, |v, (i, bit)| {
        v | if mask & (1 << i) != 0 { *bit } else { 0 }
    }))
}
fn stage_features(enabled: FeatureBits, mask: u64) -> Result<(), Status> {
    if mask & (TESS_CONTROL_STAGE | TESS_EVALUATION_STAGE) != 0 && enabled & TESSELLATION == 0
        || mask & GEOMETRY_STAGE != 0 && enabled & GEOMETRY == 0
        || mask & MESH_STAGE != 0 && enabled & MESH_SHADER == 0
        || mask & TASK_STAGE != 0 && enabled & TASK_SHADER == 0
        || mask & PREPROCESS_STAGE != 0 && enabled & GENERATED_COMMANDS == 0
        || mask & SHADING_RATE_STAGE != 0 && enabled & ATTACHMENT_SHADING_RATE == 0
    {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
fn stages(d: &Device, domain: u32, mask: u64, semaphore: bool) -> Result<u64, Status> {
    stage_features(d.snapshot.features.enabled, mask)?;
    let flags = d
        .snapshot
        .queues
        .iter()
        .find(|q| q.domain == domain)
        .ok_or(INVALID)?
        .flags;
    stage_mask(
        flags,
        d.snapshot.features.enabled & RASTER != 0,
        mask,
        semaphore,
    )
}
fn access(mask: u64, stages: u64) -> Result<u64, Status> {
    if mask & !((1 << 25) - 1) != 0 {
        return Err(UNSUPPORTED);
    }
    // ALL_COMMANDS covers GPU, not host accesses. Generic read/write require a scope.
    let shaders = 1
        | 4
        | 8
        | 16
        | GRAPHICS_SCOPE
        | PRE_RASTER_SCOPE
        | MESH_STAGE
        | TASK_STAGE
        | TESS_CONTROL_STAGE
        | TESS_EVALUATION_STAGE
        | GEOMETRY_STAGE;
    let transfer_read = 1 | 2 | COPY_SCOPE | RESOLVE_SCOPE | BLIT_SCOPE;
    let depth = 1 | 128 | EARLY_SCOPE | LATE_SCOPE | GRAPHICS_SCOPE;
    let allowed = [
        KNOWN_SCOPES,
        KNOWN_SCOPES,
        transfer_read,
        transfer_read | CLEAR_SCOPE,
        shaders,
        shaders,
        256,
        256,
        1 | 32 | GRAPHICS_SCOPE,
        1 | INDEX_SCOPE | INPUT_SCOPE | GRAPHICS_SCOPE,
        1 | FETCH_SCOPE | INPUT_SCOPE | GRAPHICS_SCOPE,
        shaders,
        1 | 64 | GRAPHICS_SCOPE,
        1 | 64 | GRAPHICS_SCOPE,
        depth,
        depth,
        shaders,
        shaders,
        shaders,
        shaders,
        shaders,
        1 | PREPROCESS_STAGE,
        1 | PREPROCESS_STAGE,
        1 | GRAPHICS_SCOPE | SHADING_RATE_STAGE,
        1 | GRAPHICS_SCOPE | 16,
    ];
    let bits = [
        vk::VK_ACCESS_2_MEMORY_READ_BIT,
        vk::VK_ACCESS_2_MEMORY_WRITE_BIT,
        vk::VK_ACCESS_2_TRANSFER_READ_BIT,
        vk::VK_ACCESS_2_TRANSFER_WRITE_BIT,
        vk::VK_ACCESS_2_SHADER_READ_BIT,
        vk::VK_ACCESS_2_SHADER_WRITE_BIT,
        vk::VK_ACCESS_2_HOST_READ_BIT,
        vk::VK_ACCESS_2_HOST_WRITE_BIT,
        vk::VK_ACCESS_2_INDIRECT_COMMAND_READ_BIT,
        vk::VK_ACCESS_2_INDEX_READ_BIT,
        vk::VK_ACCESS_2_VERTEX_ATTRIBUTE_READ_BIT,
        vk::VK_ACCESS_2_UNIFORM_READ_BIT,
        vk::VK_ACCESS_2_COLOR_ATTACHMENT_READ_BIT,
        vk::VK_ACCESS_2_COLOR_ATTACHMENT_WRITE_BIT,
        vk::VK_ACCESS_2_DEPTH_STENCIL_ATTACHMENT_READ_BIT,
        vk::VK_ACCESS_2_DEPTH_STENCIL_ATTACHMENT_WRITE_BIT,
        vk::VK_ACCESS_2_RESOURCE_HEAP_READ_BIT_EXT,
        vk::VK_ACCESS_2_SAMPLER_HEAP_READ_BIT_EXT,
        vk::VK_ACCESS_2_SHADER_SAMPLED_READ_BIT,
        vk::VK_ACCESS_2_SHADER_STORAGE_READ_BIT,
        vk::VK_ACCESS_2_SHADER_STORAGE_WRITE_BIT,
        vk::VK_ACCESS_2_COMMAND_PREPROCESS_READ_BIT_EXT,
        vk::VK_ACCESS_2_COMMAND_PREPROCESS_WRITE_BIT_EXT,
        vk::VK_ACCESS_2_FRAGMENT_SHADING_RATE_ATTACHMENT_READ_BIT_KHR,
        vk::VK_ACCESS_2_INPUT_ATTACHMENT_READ_BIT,
    ];
    let mut value = 0;
    for (i, bit) in bits.iter().enumerate() {
        if mask & (1 << i) != 0 {
            if stages & allowed[i] == 0 {
                return Err(INVALID);
            }
            value |= bit;
        }
    }
    Ok(value)
}

fn scoped_access(d: &Device, domain: u32, mask: u64, mut scope: u64) -> Result<u64, Status> {
    if mask & SHADING_RATE_READ != 0 && d.snapshot.features.enabled & ATTACHMENT_SHADING_RATE == 0 {
        return Err(UNSUPPORTED);
    }
    if scope & 1 != 0 {
        // ALL_COMMANDS expands only to operations supported by this family; it
        // doesn't make shader accesses valid on a transfer-only queue.
        let flags = d
            .snapshot
            .queues
            .iter()
            .find(|q| q.domain == domain)
            .ok_or(INVALID)?
            .flags;
        scope &= !1;
        if flags & (TRANSFER | COMPUTE | GRAPHICS) != 0 {
            scope |= 2;
        }
        if flags & COMPUTE != 0 {
            scope |= 4 | 32;
        }
        if flags & GRAPHICS != 0 {
            scope |= 8 | 16 | 32 | 64 | 128 | INPUT_SCOPE;
            if d.snapshot.features.enabled & ATTACHMENT_SHADING_RATE != 0 {
                scope |= SHADING_RATE_STAGE;
            }
        }
    }
    access(mask, scope)
}

fn array_layout<T>(count: u32) -> Result<Layout, Status> {
    if count == 0 {
        return Ok(Layout::from_size_align(0, 1).unwrap());
    }
    Layout::array::<T>(count as usize).map_err(|_| INVALID)
}
fn submit_layout(lists: u32, waits: u32, signals: u32) -> Result<(Layout, usize, usize), Status> {
    let commands = array_layout::<vk::VkCommandBufferSubmitInfo>(lists)?;
    let (layout, wait_offset) = commands
        .extend(array_layout::<vk::VkSemaphoreSubmitInfo>(waits)?)
        .map_err(|_| INVALID)?;
    let (layout, signal_offset) = layout
        .extend(array_layout::<vk::VkSemaphoreSubmitInfo>(signals)?)
        .map_err(|_| INVALID)?;
    Ok((layout.pad_to_align(), wait_offset, signal_offset))
}
pub(super) fn scratch(
    pointer: *mut c_void,
    size: u64,
    requirements: HostRequirements,
) -> Result<*mut u8, Status> {
    if size < requirements.size {
        return Err(CAPACITY);
    }
    if requirements.size != 0
        && (pointer.is_null() || pointer as usize % requirements.alignment as usize != 0)
    {
        return Err(INVALID);
    }
    Ok(pointer.cast())
}
pub(in crate::foundation) unsafe fn slice<'a, T>(
    pointer: *const T,
    count: u32,
) -> Result<&'a [T], Status> {
    array_layout::<T>(count)?;
    if count == 0 {
        return Ok(&[]);
    }
    if pointer.is_null() {
        return Err(INVALID);
    }
    // SAFETY: trusted C array extent and alignment; size arithmetic checked above.
    Ok(unsafe { std::slice::from_raw_parts(pointer, count as usize) })
}

impl List {
    pub(in crate::foundation) fn bind_executable(
        &self,
        executable: &Executable,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, executable.stages, false)?;
        if !ptr::eq(executable.device, d) {
            return Err(INVALID);
        }
        unsafe {
            (d.f.vkCmdBindPipeline.unwrap())(
                self.command,
                if executable.stages == 4 {
                    vk::VkPipelineBindPoint_VK_PIPELINE_BIND_POINT_COMPUTE
                } else {
                    vk::VkPipelineBindPoint_VK_PIPELINE_BIND_POINT_GRAPHICS
                },
                executable.pipeline,
            );
        }
        if executable.stages == 4 {
            self.executable.set(executable);
        } else {
            self.graphics.set(executable);
        }
        Ok(())
    }
    fn executable(&self, scope: u64) -> Result<&Executable, Status> {
        let d = self.recording()?;
        // Binding already established queue compatibility; only optional stage
        // enabling is needed here, not a queue-domain search on every byte update.
        stage_features(d.snapshot.features.enabled, scope)?;
        let p = match scope {
            4 => self.executable.get(),
            value
                if value != 0
                    && value
                        & !(8
                            | 16
                            | MESH_STAGE
                            | TASK_STAGE
                            | TESS_CONTROL_STAGE
                            | TESS_EVALUATION_STAGE
                            | GEOMETRY_STAGE)
                        == 0 =>
            {
                self.graphics.get()
            }
            _ => return Err(UNSUPPORTED),
        };
        unsafe { p.as_ref() }.ok_or(INVALID)
    }
    pub(in crate::foundation) unsafe fn set_arguments(
        &self,
        layout: &ArgumentLayout,
        offset: u32,
        size: u32,
        data: *const c_void,
    ) -> Result<(), Status> {
        self.recording()?;
        let bound = if layout.execution_class == ARGUMENT_COMPUTE {
            self.executable.get()
        } else {
            self.graphics.get()
        };
        let executable = unsafe { bound.as_ref() }.ok_or(INVALID)?;
        if !ptr::eq(layout.device, self.device) || !ptr::eq(executable.arguments, layout) {
            return Err(INVALID);
        }
        unsafe {
            layout.validate_update(offset, size, data)?;
        }
        if size == 0 {
            return Ok(());
        }
        let d = unsafe { &*self.device };
        let info = vk::VkPushDataInfoEXT {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PUSH_DATA_INFO_EXT,
            offset,
            data: vk::VkHostAddressRangeConstEXT {
                address: data,
                size: size as usize,
            },
            ..Default::default()
        };
        unsafe {
            (d.f.vkCmdPushDataEXT.unwrap())(self.command, &info);
        }
        Ok(())
    }
    pub(in crate::foundation) fn dispatch(&self, launch: &Launch) -> Result<(), Status> {
        self.executable(4)?;
        self.outside_render()?;
        if launch.dynamic_shared_bytes != 0 || !launch.extensions.is_null() {
            return Err(UNSUPPORTED);
        }
        let d = unsafe { &*self.device };
        let groups = [launch.groups.x, launch.groups.y, launch.groups.z];
        if groups
            .iter()
            .zip(d.snapshot.execution_limits.max_groups)
            .any(|(n, max)| *n > max)
        {
            return Err(UNSUPPORTED);
        }
        unsafe {
            (d.f.vkCmdDispatch.unwrap())(self.command, groups[0], groups[1], groups[2]);
        }
        Ok(())
    }
    pub(in crate::foundation) unsafe fn dispatch_indirect(
        &self,
        args: Span,
        dynamic_shared_bytes: u32,
    ) -> Result<(), Status> {
        self.executable(4)?;
        self.outside_render()?;
        if dynamic_shared_bytes != 0 {
            return Err(UNSUPPORTED);
        }
        let d = unsafe { &*self.device };
        let memory = unsafe { args.memory.as_ref() }.ok_or(INVALID)?;
        let (buffer, offset, _) =
            memory.command_range(d, self.domain, args.offset, args.size, 4)?;
        if offset % 4 != 0 || args.size < 12 {
            return Err(INVALID);
        }
        unsafe {
            (d.f.vkCmdDispatchIndirect.unwrap())(self.command, buffer, offset);
        }
        Ok(())
    }
    pub(in crate::foundation) unsafe fn bind_heap(
        &self,
        heap: &descriptors::storage::DescriptorHeap,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        if self.inherited_heaps & (1 << (heap.kind() - 1)) != 0 {
            return Err(INVALID);
        }
        unsafe { heap.bind(d, self.command, self.domain) }
    }
    fn recording(&self) -> Result<&Device, Status> {
        if self.state.get() != 1 {
            return Err(INVALID);
        }
        if self.error.get() != OK {
            return Err(self.error.get());
        }
        let d = unsafe { &*self.device };
        d.ready()?;
        Ok(d)
    }
    pub(in crate::foundation) fn poison(&self, status: Status) {
        if status != OK && self.error.get() == OK {
            self.error.set(status);
        }
    }
    pub(in crate::foundation) fn end(&self) -> Result<(), Status> {
        let result = self.recording().and_then(|d| {
            if self.rendering.get() != self.continuation
                || self.active_queries.iter().any(|q| q.get().is_some())
            {
                return Err(INVALID);
            }
            unsafe { d.result((d.f.vkEndCommandBuffer.unwrap())(self.command)) }
        });
        self.state.set(if result.is_ok() { 2 } else { 0 });
        result
    }
    pub(in crate::foundation) fn cancel(&self) {
        self.state.set(0);
    }
    pub(in crate::foundation) unsafe fn copy(&self, dst: Span, src: Span) -> Result<(), Status> {
        let d = self.outside_render()?;
        stages(d, self.domain, 2, false)?;
        if src.size != dst.size {
            return Err(INVALID);
        }
        let source = unsafe { src.memory.as_ref() }.ok_or(INVALID)?;
        let target = unsafe { dst.memory.as_ref() }.ok_or(INVALID)?;
        let (_, _, src_address) =
            source.command_range(self.device, self.domain, src.offset, src.size, 1)?;
        let (_, _, dst_address) =
            target.command_range(self.device, self.domain, dst.offset, dst.size, 2)?;
        if src.size == 0 {
            return Ok(());
        }
        if src_address.abs_diff(dst_address) < src.size {
            return Err(INVALID);
        }
        let region = vk::VkDeviceMemoryCopyKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEVICE_MEMORY_COPY_KHR,
            srcRange: vk::VkDeviceAddressRangeKHR {
                address: src_address,
                size: src.size,
            },
            dstRange: vk::VkDeviceAddressRangeKHR {
                address: dst_address,
                size: dst.size,
            },
            srcFlags: source.address_flags(),
            dstFlags: target.address_flags(),
            ..Default::default()
        };
        let info = vk::VkCopyDeviceMemoryInfoKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COPY_DEVICE_MEMORY_INFO_KHR,
            regionCount: 1,
            pRegions: &region,
            ..Default::default()
        };
        unsafe {
            (d.f.vkCmdCopyMemoryKHR.unwrap())(self.command, &info);
        }
        Ok(())
    }
    pub(in crate::foundation) unsafe fn fill(&self, dst: Span, pattern: u32) -> Result<(), Status> {
        let d = self.outside_render()?;
        stages(d, self.domain, 2, false)?;
        let target = unsafe { dst.memory.as_ref() }.ok_or(INVALID)?;
        let (buffer, offset, _) =
            target.command_range(self.device, self.domain, dst.offset, dst.size, 2)?;
        if offset % 4 != 0 || dst.size % 4 != 0 {
            return Err(INVALID);
        }
        if dst.size != 0 {
            unsafe {
                (d.f.vkCmdFillBuffer.unwrap())(self.command, buffer, offset, dst.size, pattern);
            }
        }
        Ok(())
    }
    pub(in crate::foundation) fn clear_image(
        &self,
        image: &Image,
        state: u32,
        range: Subresources,
        value: ClearValue,
    ) -> Result<(), Status> {
        self.outside_render()?;
        image.record_clear(self.device, self.domain, self.command, state, range, value)
    }
    pub(in crate::foundation) unsafe fn copy_image_memory(
        &self,
        image: &Image,
        span: Span,
        copy: &ImageCopy,
        to_image: bool,
    ) -> Result<(), Status> {
        let d = self.outside_render()?;
        stages(d, self.domain, 2, false)?;
        unsafe { image.record_copy(self.device, self.domain, self.command, span, copy, to_image) }
    }
    fn outside_render(&self) -> Result<&Device, Status> {
        let d = self.recording()?;
        if self.rendering.get() {
            return Err(INVALID);
        }
        Ok(d)
    }
    pub(in crate::foundation) fn transfer_image(
        &self,
        dst: &Image,
        src: &Image,
        transfer: &ImageTransfer,
        mode: u32,
    ) -> Result<(), Status> {
        let d = self.outside_render()?;
        stages(d, self.domain, 2, false)?;
        dst.record_transfer(d, self.domain, self.command, src, transfer, mode)
    }
    pub(in crate::foundation) fn blit_image(
        &self,
        dst: &Image,
        src: &Image,
        blit: &ImageBlit,
    ) -> Result<(), Status> {
        let d = self.outside_render()?;
        stages(d, self.domain, BLIT_SCOPE, false)?;
        dst.record_blit(d, self.domain, self.command, src, blit)
    }
    pub(in crate::foundation) unsafe fn render_begin(
        &self,
        desc: &RenderDesc,
    ) -> Result<(), Status> {
        let d = self.outside_render()?;
        stages(d, self.domain, 24, false)?;
        if self.level == 1 && desc.flags & 1 != 0 {
            return Err(UNSUPPORTED);
        }
        // Native multiview queries must begin and end inside the render instance.
        if desc.view_mask != 0 && self.active_queries.iter().any(|q| q.get().is_some()) {
            return Err(INVALID);
        }
        unsafe {
            d.begin_render(self.command, self.domain, desc)?;
        }
        self.view_mask.set(desc.view_mask);
        self.render_colors.set(desc.color_count);
        self.rendering.set(true);
        self.secondary_contents.set(desc.flags & 1 != 0);
        Ok(())
    }
    pub(in crate::foundation) fn render_end(&self) -> Result<(), Status> {
        let d = self.recording()?;
        if self.continuation
            || !self.rendering.get()
            || self
                .active_queries
                .iter()
                .any(|q| q.get().is_some_and(|active| active.inside))
        {
            return Err(INVALID);
        }
        unsafe {
            (d.f.vkCmdEndRendering.unwrap())(self.command);
        }
        self.rendering.set(false);
        self.view_mask.set(0);
        self.secondary_contents.set(false);
        Ok(())
    }
    pub(in crate::foundation) fn blend_constants(&self, values: &[f32; 4]) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        // Values are not clamped: native floating-point attachment behavior remains available.
        unsafe { (d.f.vkCmdSetBlendConstants.unwrap())(self.command, values.as_ptr()) };
        Ok(())
    }
    pub(in crate::foundation) fn stencil_value(
        &self,
        field: u32,
        faces: u32,
        value: u32,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        if faces & !3 != 0 {
            return Err(INVALID);
        }
        let command = match field {
            4 => d.f.vkCmdSetStencilReference,
            8 => d.f.vkCmdSetStencilCompareMask,
            16 => d.f.vkCmdSetStencilWriteMask,
            _ => return Err(INVALID),
        }
        .unwrap();
        unsafe { command(self.command, faces, value) };
        Ok(())
    }
    pub(in crate::foundation) unsafe fn render_locations(
        &self,
        state: &RenderLocations,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        if !self.rendering.get() || self.secondary_contents.get() {
            return Err(INVALID);
        }
        let info = unsafe { local_read::locations(d, state, self.render_colors.get())? };
        unsafe {
            d.f.vkCmdSetRenderingAttachmentLocations
                .ok_or(UNSUPPORTED)?(self.command, &info);
        }
        Ok(())
    }
    pub(in crate::foundation) unsafe fn render_inputs(
        &self,
        state: &RenderInputs,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        if !self.rendering.get() || self.secondary_contents.get() {
            return Err(INVALID);
        }
        let info = unsafe { local_read::inputs(d, state, self.render_colors.get())? };
        unsafe {
            d.f.vkCmdSetRenderingInputAttachmentIndices
                .ok_or(UNSUPPORTED)?(self.command, &info);
        }
        Ok(())
    }
    pub(in crate::foundation) fn shading_rate(
        &self,
        state: &ShadingRateState,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        shading_rate::state(
            d.snapshot.features.enabled,
            &d.snapshot.shading_rate_limits,
            state,
        )?;
        let size = vk::VkExtent2D {
            width: state.fragment_size[0],
            height: state.fragment_size[1],
        };
        let combiners = state.combiners;
        unsafe {
            (d.shading_rate_function.ok_or(UNSUPPORTED)?)(self.command, &size, combiners.as_ptr());
        }
        Ok(())
    }
    pub(in crate::foundation) fn viewport(&self, state: &ViewportState) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        d.viewport(self.command, state)?;
        Ok(())
    }

    pub(in crate::foundation) unsafe fn viewports(
        &self,
        first: u32,
        count: u32,
        values: *const Viewport,
        flags: u32,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        viewports::range(
            first,
            count,
            flags,
            d.snapshot.features.enabled,
            &d.snapshot.viewport_limits,
        )?;
        let values = unsafe { slice(values, count)? };
        for v in values {
            viewports::viewport(v, &d.snapshot.graphics_limits)?;
        }
        if count != 0 {
            unsafe {
                if flags & SET_COUNT != 0 {
                    (d.f.vkCmdSetViewportWithCount.unwrap())(
                        self.command,
                        count,
                        values.as_ptr().cast(),
                    );
                } else {
                    (d.f.vkCmdSetViewport.unwrap())(
                        self.command,
                        first,
                        count,
                        values.as_ptr().cast(),
                    );
                }
            }
        }
        Ok(())
    }
    pub(in crate::foundation) unsafe fn scissors(
        &self,
        first: u32,
        count: u32,
        values: *const Scissor,
        flags: u32,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        viewports::range(
            first,
            count,
            flags,
            d.snapshot.features.enabled,
            &d.snapshot.viewport_limits,
        )?;
        let values = unsafe { slice(values, count)? };
        for v in values {
            viewports::scissor(v)?;
        }
        if count != 0 {
            unsafe {
                if flags & SET_COUNT != 0 {
                    (d.f.vkCmdSetScissorWithCount.unwrap())(
                        self.command,
                        count,
                        values.as_ptr().cast(),
                    );
                } else {
                    (d.f.vkCmdSetScissor.unwrap())(
                        self.command,
                        first,
                        count,
                        values.as_ptr().cast(),
                    );
                }
            }
        }
        Ok(())
    }
    pub(in crate::foundation) unsafe fn vertex_input(
        &self,
        input: &VertexInput,
        scratch: HostSpan,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        let command = d.vertex_input_command.ok_or(UNSUPPORTED)?;
        let (bindings, attributes) = unsafe {
            vertex_input::pack(
                input,
                d.snapshot.features.enabled,
                &d.snapshot.graphics_limits,
                &d.snapshot.raster_control_limits,
                &d.snapshot.formats,
                scratch,
            )?
        };
        unsafe {
            command(
                self.command,
                input.binding_count,
                bindings,
                input.attribute_count,
                attributes,
            );
        }
        Ok(())
    }
    pub(in crate::foundation) unsafe fn sample_locations(
        &self,
        p: &SampleLocations,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        let native = unsafe {
            sample_locations::pattern(
                p,
                d.snapshot.features.enabled,
                &d.snapshot.sample_location_limits,
            )?
        };
        unsafe {
            d.sample_commands.positions.ok_or(UNSUPPORTED)?(self.command, &native);
        }
        Ok(())
    }
    pub(in crate::foundation) fn sample_locations_enable(&self, value: u32) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        if value > 1 {
            return Err(INVALID);
        }
        unsafe {
            d.sample_commands.enable.ok_or(UNSUPPORTED)?(self.command, value);
        }
        Ok(())
    }
    pub(in crate::foundation) fn raster_mechanism_value(
        &self,
        field: u64,
        value: u32,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        if raster_mechanisms::dynamic_mask(d.snapshot.features.enabled) & field == 0 {
            return Err(UNSUPPORTED);
        }
        let command = match field {
            DYNAMIC_CONSERVATIVE_MODE_STATE => {
                raster_mechanisms::conservative(
                    value,
                    d.snapshot.features.enabled,
                    &d.snapshot.raster_mechanism_limits,
                )?;
                d.raster_commands.conservative_mode
            }
            DYNAMIC_PROVOKING_VERTEX_STATE => {
                if value > 1 {
                    return Err(INVALID);
                }
                d.raster_commands.provoking_vertex
            }
            DYNAMIC_DEPTH_CLIP_STATE => {
                if value > 1 {
                    return Err(INVALID);
                }
                d.raster_commands.depth_clip
            }
            DYNAMIC_DEPTH_CONVENTION_STATE => {
                if value > 1 {
                    return Err(INVALID);
                }
                d.raster_commands.depth_convention
            }
            DYNAMIC_LINE_MODE_STATE => {
                raster_mechanisms::line_mode(value, d.snapshot.features.enabled)?;
                d.raster_commands.line_mode
            }
            DYNAMIC_LINE_STIPPLE_ENABLE_STATE => {
                if value > 1 {
                    return Err(INVALID);
                }
                if value != 0 && d.snapshot.features.enabled & raster_mechanisms::STIPPLED == 0 {
                    return Err(UNSUPPORTED);
                }
                d.raster_commands.line_stipple_enable
            }
            _ => return Err(INVALID),
        };
        unsafe {
            command.ok_or(UNSUPPORTED)?(self.command, value);
        }
        Ok(())
    }
    pub(in crate::foundation) fn extra_overestimation(&self, value: f32) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        raster_mechanisms::extra(value, &d.snapshot.raster_mechanism_limits)?;
        unsafe {
            d.raster_commands.extra_overestimation.ok_or(UNSUPPORTED)?(self.command, value);
        }
        Ok(())
    }
    pub(in crate::foundation) fn line_stipple(
        &self,
        factor: u32,
        pattern: u32,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        raster_mechanisms::stipple(factor, pattern)?;
        unsafe {
            d.raster_commands.line_stipple.ok_or(UNSUPPORTED)?(
                self.command,
                factor,
                pattern as u16,
            );
        }
        Ok(())
    }
    pub(in crate::foundation) fn dynamic_raster_value(
        &self,
        field: u64,
        value: u32,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        let (max, command) = match field {
            32 => (1, d.f.vkCmdSetDepthTestEnable),
            64 => (1, d.f.vkCmdSetDepthWriteEnable),
            128 => (7, d.f.vkCmdSetDepthCompareOp),
            256 => (1, d.f.vkCmdSetStencilTestEnable),
            1024 => (3, d.f.vkCmdSetCullMode),
            2048 => (1, d.f.vkCmdSetFrontFace),
            DYNAMIC_TOPOLOGY_STATE => {
                core_raster::topology(value, d.snapshot.features.enabled)?;
                (PATCHES, d.f.vkCmdSetPrimitiveTopology)
            }
            DYNAMIC_PRIMITIVE_RESTART_STATE => (1, d.f.vkCmdSetPrimitiveRestartEnable),
            DYNAMIC_RASTERIZER_DISCARD_STATE => (1, d.f.vkCmdSetRasterizerDiscardEnable),
            DYNAMIC_DEPTH_BIAS_ENABLE_STATE => (1, d.f.vkCmdSetDepthBiasEnable),
            DYNAMIC_DEPTH_BOUNDS_ENABLE_STATE => {
                if value == 1 && d.snapshot.features.enabled & DEPTH_BOUNDS == 0 {
                    return Err(UNSUPPORTED);
                }
                (1, d.f.vkCmdSetDepthBoundsTestEnable)
            }
            _ => return Err(INVALID),
        };
        if value > max {
            return Err(INVALID);
        }
        unsafe { command.unwrap()(self.command, value) };
        Ok(())
    }
    pub(in crate::foundation) fn line_width(&self, value: f32) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        core_raster::line_width(value, d.snapshot.features.enabled)?;
        unsafe {
            (d.f.vkCmdSetLineWidth.unwrap())(self.command, value);
        }
        Ok(())
    }
    pub(in crate::foundation) fn depth_bias(
        &self,
        constant: f32,
        clamp: f32,
        slope: f32,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        core_raster::depth_bias(constant, clamp, slope, d.snapshot.features.enabled)?;
        unsafe {
            (d.f.vkCmdSetDepthBias.unwrap())(self.command, constant, clamp, slope);
        }
        Ok(())
    }
    pub(in crate::foundation) fn depth_bounds(&self, min: f32, max: f32) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        core_raster::depth_bounds(min, max)?;
        unsafe {
            (d.f.vkCmdSetDepthBounds.unwrap())(self.command, min, max);
        }
        Ok(())
    }
    pub(in crate::foundation) fn stencil_ops(
        &self,
        faces: u32,
        fail: u32,
        pass: u32,
        depth_fail: u32,
        compare: u32,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        if faces & !3 != 0 || [fail, pass, depth_fail, compare].iter().any(|&v| v > 7) {
            return Err(INVALID);
        }
        unsafe {
            (d.f.vkCmdSetStencilOp.unwrap())(self.command, faces, fail, pass, depth_fail, compare)
        };
        Ok(())
    }
    pub(in crate::foundation) fn queries_reset(
        &self,
        pool: &QueryPool,
        first: u32,
        count: u32,
    ) -> Result<(), Status> {
        let d = self.outside_render()?;
        stages(d, self.domain, 32, false)?; // Native reset/copy requires graphics or compute, not transfer-only.
        pool.range(d, first, count)?;
        if self.active_queries.iter().any(|q| {
            q.get().is_some_and(|active| {
                ptr::eq(active.pool, pool) && active.index >= first && active.index - first < count
            })
        }) {
            return Err(INVALID);
        }
        unsafe {
            (d.f.vkCmdResetQueryPool.unwrap())(self.command, pool.handle, first, count);
        }
        Ok(())
    }
    pub(in crate::foundation) fn query(
        &self,
        pool: &QueryPool,
        index: u32,
        begin: bool,
        flags: u32,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 32, false)?;
        pool.range(d, index, self.view_mask.get().count_ones().max(1))?;
        if !matches!(pool.kind, 2 | 3) {
            return Err(INVALID);
        }
        let queue_flags = d
            .snapshot
            .queues
            .iter()
            .find(|q| q.domain == self.domain)
            .ok_or(INVALID)?
            .flags;
        if (pool.kind == 2 || pool.statistics & !1024 != 0) && queue_flags & GRAPHICS == 0
            || pool.statistics & 1024 != 0 && queue_flags & COMPUTE == 0
        {
            return Err(UNSUPPORTED);
        }
        let native_flags = queries::controls(pool.kind, d.snapshot.features.enabled, flags)?;
        let active = &self.active_queries[(pool.kind - 2) as usize];
        if begin {
            if active.get().is_some() {
                return Err(INVALID);
            }
            unsafe {
                (d.f.vkCmdBeginQuery.unwrap())(self.command, pool.handle, index, native_flags);
            }
            active.set(Some(ActiveQuery {
                pool,
                index,
                inside: self.rendering.get(),
                flags,
            }));
        } else {
            let previous = active.get().ok_or(INVALID)?;
            if !ptr::eq(previous.pool, pool)
                || previous.index != index
                || previous.inside != self.rendering.get()
            {
                return Err(INVALID);
            }
            unsafe {
                (d.f.vkCmdEndQuery.unwrap())(self.command, pool.handle, index);
            }
            active.set(None);
        }
        Ok(())
    }
    pub(in crate::foundation) fn timestamp(
        &self,
        pool: &QueryPool,
        index: u32,
        scope: u64,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        pool.range(d, index, self.view_mask.get().count_ones().max(1))?;
        let native = stages(d, self.domain, scope, true)?;
        if pool.kind != 1 || native.count_ones() != 1 {
            return Err(INVALID);
        }
        if d.snapshot
            .queues
            .iter()
            .find(|q| q.domain == self.domain)
            .ok_or(INVALID)?
            .timestamp_bits
            == 0
        {
            return Err(UNSUPPORTED);
        }
        unsafe {
            (d.f.vkCmdWriteTimestamp2.unwrap())(self.command, native, pool.handle, index);
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub(in crate::foundation) unsafe fn queries_resolve(
        &self,
        pool: &QueryPool,
        first: u32,
        count: u32,
        dst: Span,
        stride: u64,
        flags: u32,
    ) -> Result<(), Status> {
        let d = self.outside_render()?;
        stages(d, self.domain, 32, false)?;
        pool.range(d, first, count)?;
        if self.active_queries.iter().any(|q| {
            q.get().is_some_and(|active| {
                ptr::eq(active.pool, pool) && active.index >= first && active.index - first < count
            })
        }) {
            return Err(INVALID);
        }
        let (size, align, native) =
            queries::result_layout(pool.kind, pool.statistics, count, stride, flags)?;
        if dst.size < size {
            return Err(INVALID);
        }
        let memory = unsafe { dst.memory.as_ref() }.ok_or(INVALID)?;
        let (buffer, offset, _) = memory.command_range(d, self.domain, dst.offset, dst.size, 2)?;
        if offset % align != 0 {
            return Err(INVALID);
        }
        unsafe {
            (d.f.vkCmdCopyQueryPoolResults.unwrap())(
                self.command,
                pool.handle,
                first,
                count,
                buffer,
                offset,
                stride,
                native,
            );
        }
        Ok(())
    }
    fn drawing(&self, indexed: bool) -> Result<&Device, Status> {
        let executable = self.executable(24)?;
        if executable.view_mask != self.view_mask.get() || executable.stages & MESH_STAGE != 0 {
            return Err(INVALID);
        }
        if !self.rendering.get()
            || self.secondary_contents.get()
            || (indexed && self.index_count.get().is_none())
        {
            return Err(INVALID);
        }
        Ok(unsafe { &*self.device })
    }
    fn mesh_drawing(&self) -> Result<(&Device, &MeshWorkgroupLimits), Status> {
        let executable = self.executable(MESH_STAGE)?;
        if executable.view_mask != self.view_mask.get()
            || executable.stages & MESH_STAGE == 0
            || !self.rendering.get()
            || self.secondary_contents.get()
        {
            return Err(INVALID);
        }
        // Legacy vertex/tessellation/geometry statistics are invalid for mesh draws.
        if self.active_queries[1]
            .get()
            .is_some_and(|q| unsafe { (*q.pool).statistics & (127 | 256 | 512) != 0 })
        {
            return Err(INVALID);
        }
        let d = unsafe { &*self.device };
        let limits = if executable.stages & TASK_STAGE != 0 {
            &d.snapshot.mesh_limits.task
        } else {
            &d.snapshot.mesh_limits.mesh
        };
        Ok((d, limits))
    }
    pub(in crate::foundation) fn draw_mesh(&self, groups: Extent) -> Result<(), Status> {
        let (d, limits) = self.mesh_drawing()?;
        mesh::groups(limits, [groups.x, groups.y, groups.z])?;
        unsafe {
            (d.mesh_functions
                .as_ref()
                .ok_or(UNSUPPORTED)?
                .direct
                .unwrap())(self.command, groups.x, groups.y, groups.z)
        };
        Ok(())
    }
    pub(in crate::foundation) unsafe fn draw_mesh_indirect(
        &self,
        desc: &Indirect,
    ) -> Result<(), Status> {
        unsafe { self.record_indirect(desc, DrawKind::Mesh) }
    }
    pub(in crate::foundation) fn draw(&self, draw: &DrawDesc, indexed: bool) -> Result<(), Status> {
        let d = self.drawing(indexed)?;
        features::instance_range(
            self.view_mask.get(),
            d.snapshot.graphics_limits.max_multiview_instance_index,
            draw.first_instance,
            draw.instances,
        )?;
        if indexed {
            if u64::from(draw.first) + u64::from(draw.count) > self.index_count.get().unwrap() {
                return Err(INVALID);
            }
            unsafe {
                (d.f.vkCmdDrawIndexed.unwrap())(
                    self.command,
                    draw.count,
                    draw.instances,
                    draw.first,
                    draw.vertex_offset,
                    draw.first_instance,
                );
            }
            return Ok(());
        }
        if draw.vertex_offset != 0 {
            return Err(INVALID);
        }
        unsafe {
            (d.f.vkCmdDraw.unwrap())(
                self.command,
                draw.count,
                draw.instances,
                draw.first,
                draw.first_instance,
            );
        }
        Ok(())
    }
    pub(in crate::foundation) unsafe fn bind_indices(
        &self,
        span: Span,
        kind: u32,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        let (size, native) = match kind {
            16 => (2, vk::VkIndexType_VK_INDEX_TYPE_UINT16),
            32 => (4, vk::VkIndexType_VK_INDEX_TYPE_UINT32),
            _ => return Err(UNSUPPORTED),
        };
        let memory = unsafe { span.memory.as_ref() }.ok_or(INVALID)?;
        let (_, _, address) = memory.command_range(d, self.domain, span.offset, span.size, 8)?;
        if address % size != 0 || span.size % size != 0 {
            return Err(INVALID);
        }
        let info = vk::VkBindIndexBuffer3InfoKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BIND_INDEX_BUFFER_3_INFO_KHR,
            addressRange: vk::VkDeviceAddressRangeKHR {
                address,
                size: span.size,
            },
            addressFlags: memory.address_flags(),
            indexType: native,
            ..Default::default()
        };
        unsafe {
            (d.f.vkCmdBindIndexBuffer3KHR.unwrap())(self.command, &info);
        }
        self.index_count.set(Some(span.size / size));
        Ok(())
    }
    pub(in crate::foundation) unsafe fn bind_vertices(
        &self,
        first: u32,
        count: u32,
        spans: *const VertexBufferBinding,
        scratch: HostSpan,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        let max = d.snapshot.graphics_limits.max_vertex_bindings;
        if first > max || count > max - first {
            return Err(INVALID);
        }
        if count == 0 {
            return Ok(());
        }
        let req = vertex_scratch(count)?;
        if scratch.size < req.size {
            return Err(CAPACITY);
        }
        if spans.is_null()
            || scratch.data.is_null()
            || scratch.data as usize % req.alignment as usize != 0
        {
            return Err(INVALID);
        }
        let output = scratch.data.cast::<vk::VkBindVertexBuffer3InfoKHR>();
        for i in 0..count as usize {
            let binding = unsafe { &*spans.add(i) };
            let span = &binding.span;
            if binding.flags & !VERTEX_BIND_STRIDE != 0 {
                return Err(UNSUPPORTED);
            }
            if binding.stride > d.snapshot.graphics_limits.max_vertex_stride
                || binding.flags == 0 && binding.stride != 0
            {
                return Err(INVALID);
            }
            let memory = unsafe { span.memory.as_ref() }.ok_or(INVALID)?;
            let (_, _, address) =
                memory.command_range(d, self.domain, span.offset, span.size, 16)?;
            let native = vk::VkBindVertexBuffer3InfoKHR {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BIND_VERTEX_BUFFER_3_INFO_KHR,
                setStride: binding.flags & VERTEX_BIND_STRIDE,
                addressRange: vk::VkStridedDeviceAddressRangeKHR {
                    address,
                    size: span.size,
                    stride: u64::from(binding.stride),
                },
                addressFlags: memory.address_flags(),
                ..Default::default()
            };
            unsafe {
                output.add(i).write(native);
            }
        }
        // All local checks precede the single native bind. The caller chooses native static or dynamic stride binding.
        unsafe {
            (d.f.vkCmdBindVertexBuffers3KHR.unwrap())(self.command, first, count, output);
        }
        Ok(())
    }
    pub(in crate::foundation) unsafe fn draw_indirect(
        &self,
        desc: &Indirect,
        indexed: bool,
    ) -> Result<(), Status> {
        unsafe {
            self.record_indirect(
                desc,
                if indexed {
                    DrawKind::Indices
                } else {
                    DrawKind::Vertices
                },
            )
        }
    }
    unsafe fn record_indirect(&self, desc: &Indirect, kind: DrawKind) -> Result<(), Status> {
        let (d, record_size) = match kind {
            DrawKind::Mesh => (self.mesh_drawing()?.0, 12),
            DrawKind::Indices => (self.drawing(true)?, 20),
            DrawKind::Vertices => (self.drawing(false)?, 16),
        };
        if desc.maximum_count > d.snapshot.graphics_limits.max_indirect_count {
            return Err(INVALID);
        }
        let needed = indirect_record_size(desc.stride, desc.maximum_count, record_size)?;
        if desc.arguments.size < needed {
            return Err(INVALID);
        }
        let memory = unsafe { desc.arguments.memory.as_ref() }.ok_or(INVALID)?;
        let (_, _, address) = memory.command_range(
            d,
            self.domain,
            desc.arguments.offset,
            desc.arguments.size,
            4,
        )?;
        if address % 4 != 0 {
            return Err(INVALID);
        }
        let range = vk::VkStridedDeviceAddressRangeKHR {
            address,
            size: desc.arguments.size,
            stride: u64::from(desc.stride),
        };
        if desc.count.memory.is_null() {
            if desc.count.offset != 0 || desc.count.size != 0 {
                return Err(INVALID);
            }
            let info = vk::VkDrawIndirect2InfoKHR {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DRAW_INDIRECT_2_INFO_KHR,
                addressRange: range,
                addressFlags: memory.address_flags(),
                drawCount: desc.maximum_count,
                ..Default::default()
            };
            let command = match kind {
                DrawKind::Mesh => d.mesh_functions.as_ref().ok_or(UNSUPPORTED)?.indirect,
                DrawKind::Indices => d.f.vkCmdDrawIndexedIndirect2KHR,
                DrawKind::Vertices => d.f.vkCmdDrawIndirect2KHR,
            };
            unsafe {
                command.unwrap()(self.command, &info);
            }
        } else {
            let count = unsafe { &*desc.count.memory };
            if desc.count.size < 4 {
                return Err(INVALID);
            }
            let (_, _, count_address) =
                count.command_range(d, self.domain, desc.count.offset, desc.count.size, 4)?;
            if count_address % 4 != 0 {
                return Err(INVALID);
            }
            let info = vk::VkDrawIndirectCount2InfoKHR {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DRAW_INDIRECT_COUNT_2_INFO_KHR,
                addressRange: range,
                addressFlags: memory.address_flags(),
                countAddressRange: vk::VkDeviceAddressRangeKHR {
                    address: count_address,
                    size: desc.count.size,
                },
                countAddressFlags: count.address_flags(),
                maxDrawCount: desc.maximum_count,
                ..Default::default()
            };
            let command = match kind {
                DrawKind::Mesh => d.mesh_functions.as_ref().ok_or(UNSUPPORTED)?.count,
                DrawKind::Indices => d.f.vkCmdDrawIndexedIndirectCount2KHR,
                DrawKind::Vertices => d.f.vkCmdDrawIndirectCount2KHR,
            };
            unsafe {
                command.unwrap()(self.command, &info);
            }
        }
        Ok(())
    }
}

enum DrawKind {
    Vertices,
    Indices,
    Mesh,
}

#[cfg(test)]
fn indirect_size(stride: u32, count: u32, indexed: bool) -> Result<u64, Status> {
    indirect_record_size(stride, count, if indexed { 20 } else { 16 })
}
fn indirect_record_size(stride: u32, count: u32, record: u32) -> Result<u64, Status> {
    if (stride != 0 && (stride < record || stride % 4 != 0)) || (stride == 0 && count > 1) {
        return Err(INVALID);
    }
    Ok(if count == 0 {
        0
    } else {
        u64::from(count - 1) * u64::from(stride) + u64::from(record)
    })
}

impl List {
    pub(in crate::foundation) fn extended_raster_value(
        &self,
        field: u64,
        value: u32,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        extended_raster::scalar(field, value, d.snapshot.features.enabled)?;
        let command = match field {
            DYNAMIC_DEPTH_CLAMP_STATE => d.extended_commands.depth_clamp,
            DYNAMIC_POLYGON_MODE_STATE => d.extended_commands.polygon_mode,
            DYNAMIC_RASTERIZATION_SAMPLES_STATE => d.extended_commands.rasterization_samples,
            DYNAMIC_ALPHA_TO_COVERAGE_STATE => d.extended_commands.alpha_to_coverage,
            DYNAMIC_ALPHA_TO_ONE_STATE => d.extended_commands.alpha_to_one,
            DYNAMIC_LOGIC_OP_ENABLE_STATE => d.extended_commands.logic_op_enable,
            DYNAMIC_LOGIC_OP_STATE => d.extended_commands.logic_op,
            _ => return Err(INVALID),
        };
        unsafe {
            command.ok_or(UNSUPPORTED)?(self.command, value);
        }
        Ok(())
    }
    pub(in crate::foundation) fn sample_mask(
        &self,
        samples_count: u32,
        mask: u64,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        let command = d.extended_commands.sample_mask.ok_or(UNSUPPORTED)?;
        samples(samples_count, 127)?;
        let words = [mask as u32, (mask >> 32) as u32];
        unsafe {
            command(self.command, samples_count, words.as_ptr());
        }
        Ok(())
    }
    pub(in crate::foundation) unsafe fn color_values(
        &self,
        field: u64,
        first: u32,
        count: u32,
        values: *const u32,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        let (command, max) = match field {
            DYNAMIC_COLOR_BLEND_ENABLE_STATE => (d.extended_commands.color_blend_enable, 1),
            DYNAMIC_COLOR_WRITE_MASK_STATE => (d.extended_commands.color_write_mask, 15),
            _ => return Err(INVALID),
        };
        let command = command.ok_or(UNSUPPORTED)?;
        extended_raster::attachment_range(first, count, d.snapshot.graphics_limits.max_colors)?;
        let values = unsafe { slice(values, count)? };
        if values.iter().any(|&v| v > max) {
            return Err(INVALID);
        }
        if count != 0 {
            unsafe {
                command(self.command, first, count, values.as_ptr());
            }
        }
        Ok(())
    }
    pub(in crate::foundation) unsafe fn color_equations(
        &self,
        first: u32,
        count: u32,
        values: *const BlendEquation,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 24, false)?;
        let command = d
            .extended_commands
            .color_blend_equation
            .ok_or(UNSUPPORTED)?;
        extended_raster::attachment_range(first, count, d.snapshot.graphics_limits.max_colors)?;
        let values = unsafe { slice(values, count)? };
        for v in values {
            extended_raster::equation(v, d.snapshot.features.enabled)?;
        }
        if count != 0 {
            unsafe {
                command(self.command, first, count, values.as_ptr().cast());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_raster_stage_scopes_preserve_shader_access_and_enables() {
        for (stage, native, feature) in [
            (
                TESS_CONTROL_STAGE,
                vk::VK_PIPELINE_STAGE_2_TESSELLATION_CONTROL_SHADER_BIT,
                TESSELLATION,
            ),
            (
                TESS_EVALUATION_STAGE,
                vk::VK_PIPELINE_STAGE_2_TESSELLATION_EVALUATION_SHADER_BIT,
                TESSELLATION,
            ),
            (
                GEOMETRY_STAGE,
                vk::VK_PIPELINE_STAGE_2_GEOMETRY_SHADER_BIT,
                GEOMETRY,
            ),
        ] {
            assert_eq!(stage_features(0, stage), Err(UNSUPPORTED));
            assert_eq!(stage_features(feature, stage), Ok(()));
            assert_eq!(stage_mask(GRAPHICS, true, stage, false), Ok(native));
            assert_eq!(stage_mask(COMPUTE, false, stage, false), Err(UNSUPPORTED));
            assert_eq!(
                access(1 << 19, stage),
                Ok(vk::VK_ACCESS_2_SHADER_STORAGE_READ_BIT)
            );
        }
    }
    #[test]
    fn indirect_ranges_preserve_native_stride_rules() {
        use super::*;
        assert_eq!(indirect_record_size(0, 1, 12), Ok(12));
        assert_eq!(indirect_record_size(16, 3, 12), Ok(44));
        assert_eq!(indirect_record_size(8, 1, 12), Err(INVALID));
        assert_eq!(indirect_record_size(0, 2, 12), Err(INVALID));
        assert_eq!(indirect_size(0, 0, false), Ok(0));
        assert_eq!(indirect_size(0, 1, false), Ok(16));
        assert_eq!(indirect_size(0, 1, true), Ok(20));
        assert_eq!(indirect_size(0, 2, false), Err(INVALID));
        assert_eq!(indirect_size(4, 1, false), Err(INVALID));
        assert_eq!(indirect_size(18, 1, false), Err(INVALID));
        assert_eq!(indirect_size(16, 2, true), Err(INVALID));
        assert_eq!(indirect_size(32, 3, true), Ok(84));
        assert_eq!(
            indirect_size(u32::MAX - 3, u32::MAX, true),
            Ok(u64::from(u32::MAX - 1) * u64::from(u32::MAX - 3) + 20)
        );
    }
    use super::*;
    thread_local! {
        static FAILURE: Cell<u32> = const { Cell::new(0) };
        static SUBMITS: Cell<u32> = const { Cell::new(0) };
        static BATCH_COUNT: Cell<u32> = const { Cell::new(0) };
        static BEGIN: Cell<vk::PFN_vkBeginCommandBuffer> = const { Cell::new(None) };
        static END: Cell<vk::PFN_vkEndCommandBuffer> = const { Cell::new(None) };
        static RESET: Cell<vk::PFN_vkResetCommandPool> = const { Cell::new(None) };
        static ALLOCATE: Cell<vk::PFN_vkAllocateCommandBuffers> = const { Cell::new(None) };
        static SUBMIT: Cell<vk::PFN_vkQueueSubmit2> = const { Cell::new(None) };
    }
    unsafe extern "C" fn begin(
        command: vk::VkCommandBuffer,
        info: *const vk::VkCommandBufferBeginInfo,
    ) -> vk::VkResult {
        if FAILURE.get() == 1 {
            return vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY;
        }
        unsafe { BEGIN.get().unwrap()(command, info) }
    }
    unsafe extern "C" fn end(command: vk::VkCommandBuffer) -> vk::VkResult {
        if FAILURE.get() == 2 {
            return vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY;
        }
        unsafe { END.get().unwrap()(command) }
    }
    unsafe extern "C" fn reset(
        device: vk::VkDevice,
        pool: vk::VkCommandPool,
        flags: vk::VkCommandPoolResetFlags,
    ) -> vk::VkResult {
        if FAILURE.get() == 6 {
            return vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY;
        }
        unsafe { RESET.get().unwrap()(device, pool, flags) }
    }
    unsafe extern "C" fn allocate(
        device: vk::VkDevice,
        info: *const vk::VkCommandBufferAllocateInfo,
        out: *mut vk::VkCommandBuffer,
    ) -> vk::VkResult {
        if FAILURE.get() == 7 {
            // Native failure output must never be adopted, submitted or freed.
            unsafe {
                out.write(ptr::dangling_mut());
            }
            return vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY;
        }
        unsafe { ALLOCATE.get().unwrap()(device, info, out) }
    }
    unsafe extern "C" fn submit(
        queue: vk::VkQueue,
        count: u32,
        infos: *const vk::VkSubmitInfo2,
        fence: vk::VkFence,
    ) -> vk::VkResult {
        SUBMITS.set(SUBMITS.get() + 1);
        BATCH_COUNT.set(count);
        match FAILURE.get() {
            3 => vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY,
            4 => vk::VkResult_VK_ERROR_DEVICE_LOST,
            5 => vk::VkResult_VK_ERROR_UNKNOWN,
            _ => unsafe { SUBMIT.get().unwrap()(queue, count, infos, fence) },
        }
    }

    #[test]
    #[ignore = "requires modern Vulkan; command failure adoption, retry, reset poisoning and terminal submit loss"]
    fn gpu_foundation_commands() {
        let instance = Arc::new(Instance::new().unwrap());
        let mut tested = 0;
        for physical in instance.physical_devices().unwrap() {
            let snapshot = snapshot(&instance, physical).unwrap();
            if snapshot.features.baseline_supported == 0 {
                continue;
            }
            let domain = snapshot
                .queues
                .iter()
                .find(|q| q.count > 0 && q.flags & (GRAPHICS | COMPUTE | TRANSFER) != 0)
                .unwrap()
                .domain;
            let adapter = Adapter {
                instance: instance.clone(),
                physical,
                snapshot,
            };
            let requests = [QueueRequest {
                domain,
                count: 1,
                priority: 0.5,
            }];
            let ad = ArenaDesc {
                header: Record::new::<ArenaDesc>(ARENA_DESC),
                domain,
                list_capacity: 1,
                level: 0,
                reserved: 0,
            };
            let rd = RecordingDesc {
                header: Record::new::<RecordingDesc>(RECORDING_DESC),
                replay_mode: 1,
                level: 0,
                inheritance: ptr::null(),
            };
            for terminal in [4, 5] {
                let mut d = Device::create(&adapter, &requests, 0).unwrap();
                // Install mocks before borrowing the device into any child.
                BEGIN.set(d.f.vkBeginCommandBuffer);
                d.f.vkBeginCommandBuffer = Some(begin);
                END.set(d.f.vkEndCommandBuffer);
                d.f.vkEndCommandBuffer = Some(end);
                RESET.set(d.f.vkResetCommandPool);
                d.f.vkResetCommandPool = Some(reset);
                ALLOCATE.set(d.f.vkAllocateCommandBuffers);
                d.f.vkAllocateCommandBuffers = Some(allocate);
                SUBMIT.set(d.f.vkQueueSubmit2);
                d.f.vkQueueSubmit2 = Some(submit);
                FAILURE.set(7);
                assert!(matches!(Arena::create(&d, &ad), Err(OUT_OF_MEMORY)));
                FAILURE.set(0);
                let mut arena = Arena::create(&d, &ad).unwrap();
                FAILURE.set(7);
                assert_eq!(arena.reserve(2), Err(OUT_OF_MEMORY));
                assert_eq!(arena.slots.len(), 1);
                FAILURE.set(1);
                assert_eq!(arena.begin(&rd), Err(OUT_OF_MEMORY));
                assert_eq!(arena.used, 1);
                FAILURE.set(0);
                arena.reset(false).unwrap();
                let encoder = arena.begin(&rd).unwrap();
                FAILURE.set(2);
                assert_eq!(unsafe { &*encoder }.end(), Err(OUT_OF_MEMORY));
                assert_eq!(unsafe { &*encoder }.state.get(), 0);
                FAILURE.set(0);
                arena.reset(false).unwrap();
                let list = arena.begin(&rd).unwrap();
                unsafe { &*list }.end().unwrap();
                let timeline = Timeline::create(&d, 0).unwrap();
                let point = SyncPoint {
                    point: Point {
                        timeline: ptr::from_ref(&*timeline).cast_mut(),
                        value: 1,
                    },
                    stages: 1,
                };
                let mut desc = SubmitDesc {
                    list_count: 1,
                    signal_count: 1,
                    lists: &list,
                    signals: &point,
                    ..Default::default()
                };
                let mut batches = [desc, SubmitDesc::default()];
                let req = submit_scratch(&d, &batches).unwrap();
                let mut data = vec![0u64; req.size.div_ceil(8) as usize];
                let storage = HostSpan {
                    data: data.as_mut_ptr().cast(),
                    size: req.size,
                };
                let mut outcomes = [SubmitOutcome::rejected(INVALID); 2];
                let queue = d.queue(domain, 0).unwrap();
                SUBMITS.set(0);
                unsafe {
                    queue.submit_descriptions(
                        &[],
                        &mut [],
                        HostSpan {
                            data: ptr::null_mut(),
                            size: 0,
                        },
                    )
                }
                .unwrap();
                batches[1].flags = 1;
                assert_eq!(
                    unsafe { queue.submit_descriptions(&batches, &mut outcomes, storage) },
                    Err(UNSUPPORTED)
                );
                assert_eq!(SUBMITS.get(), 0);
                assert_eq!(unsafe { &*list }.state.get(), 2);
                batches[1].flags = 0;
                FAILURE.set(3);
                assert_eq!(
                    unsafe { queue.submit_descriptions(&batches, &mut outcomes, storage) },
                    Err(OUT_OF_MEMORY)
                );
                assert_eq!(SUBMITS.get(), 1);
                assert_eq!(BATCH_COUNT.get(), 2);
                assert_eq!(unsafe { &*list }.state.get(), 2);
                assert_eq!(d.ready(), Ok(()));
                SUBMITS.set(0);
                FAILURE.set(3);
                assert_eq!(
                    unsafe { queue.submit_descriptions(&[desc], &mut outcomes[..1], storage) },
                    Err(OUT_OF_MEMORY)
                );
                assert_eq!(unsafe { &*list }.state.get(), 2);
                assert_eq!(d.ready(), Ok(()));
                assert_eq!(
                    unsafe {
                        queue.submit_descriptions(
                            &[desc],
                            &mut outcomes[..1],
                            HostSpan { size: 0, ..storage },
                        )
                    },
                    Err(CAPACITY)
                );
                assert_eq!(SUBMITS.get(), 1);
                FAILURE.set(0);
                unsafe { queue.submit_descriptions(&batches, &mut outcomes, storage) }.unwrap();
                assert_eq!(BATCH_COUNT.get(), 2);
                timeline.wait(1, 10_000_000_000).unwrap();
                assert_eq!(
                    unsafe { queue.submit_descriptions(&[desc], &mut outcomes[..1], storage) },
                    Err(INVALID)
                );
                assert_eq!(SUBMITS.get(), 2);
                FAILURE.set(6);
                assert_eq!(arena.reset(false), Err(OUT_OF_MEMORY));
                assert_eq!(arena.begin(&rd), Err(INVALID));
                // Internal state check only: public handles were invalidated by reset.
                assert_eq!(arena.slots[0].state.get(), 0);
                FAILURE.set(0);
                arena.reset(false).unwrap();
                let list = arena.begin(&rd).unwrap();
                unsafe { &*list }.end().unwrap();
                desc.lists = &list;
                desc.signal_count = 0;
                FAILURE.set(terminal);
                assert_eq!(
                    unsafe { queue.submit_descriptions(&[desc], &mut outcomes[..1], storage) },
                    Err(if terminal == 4 {
                        DEVICE_LOST
                    } else {
                        BACKEND_ERROR
                    })
                );
                assert_eq!(
                    unsafe { queue.submit_descriptions(&[desc], &mut outcomes[..1], storage) },
                    Err(DEVICE_LOST)
                );
                assert_eq!(
                    unsafe { queue.submit_descriptions(&[], &mut [], storage) },
                    Err(DEVICE_LOST)
                );
                assert_eq!(SUBMITS.get(), 3);
                assert_eq!(arena.begin(&rd), Err(DEVICE_LOST));
                // The injected loss did not submit native work; real device is idle.
                drop(arena);
                drop(timeline);
                drop(d);
            }
            // Separate arenas permit independent simultaneous host recording.
            let d = Device::create(&adapter, &requests, 0).unwrap();
            std::thread::scope(|scope| {
                for _ in 0..4 {
                    let d = &d;
                    scope.spawn(move || {
                        let ad = ArenaDesc {
                            header: Record::new::<ArenaDesc>(ARENA_DESC),
                            domain,
                            list_capacity: 1,
                            level: 0,
                            reserved: 0,
                        };
                        let rd = RecordingDesc {
                            header: Record::new::<RecordingDesc>(RECORDING_DESC),
                            replay_mode: 2,
                            level: 0,
                            inheritance: ptr::null(),
                        };
                        let mut arena = Arena::create(d, &ad).unwrap();
                        for _ in 0..16 {
                            let list = arena.begin(&rd).unwrap();
                            unsafe { &*list }.end().unwrap();
                            arena.reset(false).unwrap();
                        }
                    });
                }
            });
            tested += 1;
        }
        assert_ne!(tested, 0, "No suitable device, not a passing skip");
    }
    #[test]
    fn synchronization_scopes_remain_narrow_and_queue_specific() {
        assert_eq!(stage_features(0, PREPROCESS_STAGE), Err(UNSUPPORTED));
        assert_eq!(stage_features(GENERATED_COMMANDS, PREPROCESS_STAGE), Ok(()));
        assert_eq!(
            stage_mask(COMPUTE, false, PREPROCESS_STAGE, false),
            Ok(vk::VK_PIPELINE_STAGE_2_COMMAND_PREPROCESS_BIT_EXT)
        );
        assert_eq!(
            stage_mask(TRANSFER, false, PREPROCESS_STAGE, false),
            Err(UNSUPPORTED)
        );
        assert_eq!(
            access(PREPROCESS_READ, PREPROCESS_STAGE),
            Ok(vk::VK_ACCESS_2_COMMAND_PREPROCESS_READ_BIT_EXT)
        );
        assert_eq!(
            access(PREPROCESS_WRITE, PREPROCESS_STAGE),
            Ok(vk::VK_ACCESS_2_COMMAND_PREPROCESS_WRITE_BIT_EXT)
        );
        assert_eq!(access(PREPROCESS_READ, 4), Err(INVALID));
        assert_eq!(stage_features(0, MESH_STAGE), Err(UNSUPPORTED));
        assert_eq!(stage_features(MESH_SHADER, TASK_STAGE), Err(UNSUPPORTED));
        assert_eq!(stage_features(MESH_SHADER, MESH_STAGE), Ok(()));
        assert_eq!(
            stage_features(MESH_SHADER | TASK_SHADER, MESH_STAGE | TASK_STAGE),
            Ok(())
        );
        let exact = [
            (MESH_STAGE, vk::VK_PIPELINE_STAGE_2_MESH_SHADER_BIT_EXT),
            (TASK_STAGE, vk::VK_PIPELINE_STAGE_2_TASK_SHADER_BIT_EXT),
            (8, vk::VK_PIPELINE_STAGE_2_VERTEX_SHADER_BIT),
            (COPY_SCOPE, vk::VK_PIPELINE_STAGE_2_COPY_BIT),
            (INDEX_SCOPE, vk::VK_PIPELINE_STAGE_2_INDEX_INPUT_BIT),
            (
                FETCH_SCOPE,
                vk::VK_PIPELINE_STAGE_2_VERTEX_ATTRIBUTE_INPUT_BIT,
            ),
            (
                EARLY_SCOPE,
                vk::VK_PIPELINE_STAGE_2_EARLY_FRAGMENT_TESTS_BIT,
            ),
            (LATE_SCOPE, vk::VK_PIPELINE_STAGE_2_LATE_FRAGMENT_TESTS_BIT),
            (CLEAR_SCOPE, vk::VK_PIPELINE_STAGE_2_CLEAR_BIT),
            (RESOLVE_SCOPE, vk::VK_PIPELINE_STAGE_2_RESOLVE_BIT),
            (BLIT_SCOPE, vk::VK_PIPELINE_STAGE_2_BLIT_BIT),
            (INPUT_SCOPE, vk::VK_PIPELINE_STAGE_2_VERTEX_INPUT_BIT),
            (GRAPHICS_SCOPE, vk::VK_PIPELINE_STAGE_2_ALL_GRAPHICS_BIT),
            (
                PRE_RASTER_SCOPE,
                vk::VK_PIPELINE_STAGE_2_PRE_RASTERIZATION_SHADERS_BIT,
            ),
        ];
        for (scope, native) in exact {
            assert_eq!(stage_mask(GRAPHICS, true, scope, false), Ok(native));
            assert_eq!(stage_mask(GRAPHICS, true, scope, true), Ok(native));
            if scope & RASTER_SCOPES != 0 {
                assert_eq!(stage_mask(GRAPHICS, false, scope, false), Err(UNSUPPORTED));
            }
            if scope != COPY_SCOPE && scope != CLEAR_SCOPE {
                assert_eq!(stage_mask(TRANSFER, false, scope, false), Err(UNSUPPORTED));
                assert_eq!(stage_mask(COMPUTE, false, scope, false), Err(UNSUPPORTED));
            }
        }
        assert_eq!(
            stage_mask(TRANSFER, false, COPY_SCOPE | CLEAR_SCOPE, false),
            Ok(vk::VK_PIPELINE_STAGE_2_COPY_BIT | vk::VK_PIPELINE_STAGE_2_CLEAR_BIT)
        );
        assert_eq!(stage_mask(GRAPHICS, true, 1 << 27, false), Err(UNSUPPORTED));
        assert_eq!(stage_mask(COMPUTE, false, 256, true), Err(INVALID));
        assert_eq!(access(512, 8), Err(INVALID));
        assert_eq!(access(1024, 8), Err(INVALID));
        assert_eq!(access(512, INDEX_SCOPE), Ok(vk::VK_ACCESS_2_INDEX_READ_BIT));
        assert_eq!(
            access(1024, FETCH_SCOPE),
            Ok(vk::VK_ACCESS_2_VERTEX_ATTRIBUTE_READ_BIT)
        );
        assert_eq!(access(512, FETCH_SCOPE), Err(INVALID));
        assert_eq!(access(1024, INDEX_SCOPE), Err(INVALID));
        assert_eq!(access(16, FETCH_SCOPE), Err(INVALID));
        for scope in [MESH_STAGE, TASK_STAGE, MESH_STAGE | TASK_STAGE] {
            assert_eq!(access(16, scope), Ok(vk::VK_ACCESS_2_SHADER_READ_BIT));
            assert_eq!(access(32, scope), Ok(vk::VK_ACCESS_2_SHADER_WRITE_BIT));
        }
        assert_eq!(access(4, CLEAR_SCOPE), Err(INVALID));
        assert_eq!(
            access(8, CLEAR_SCOPE),
            Ok(vk::VK_ACCESS_2_TRANSFER_WRITE_BIT)
        );
        for (mask, native) in [
            (1 << 18, vk::VK_ACCESS_2_SHADER_SAMPLED_READ_BIT),
            (1 << 19, vk::VK_ACCESS_2_SHADER_STORAGE_READ_BIT),
            (1 << 20, vk::VK_ACCESS_2_SHADER_STORAGE_WRITE_BIT),
        ] {
            for scope in [4, 8, 16, PRE_RASTER_SCOPE, GRAPHICS_SCOPE] {
                assert_eq!(access(mask, scope), Ok(native));
            }
            assert_eq!(access(mask, COPY_SCOPE), Err(INVALID));
        }
    }
    #[test]
    fn scratch_layout_and_scope_contracts() {
        for (l, w, s) in [(1, 0, 0), (0, 1, 1), (3, 5, 7)] {
            let (layout, wo, so) = submit_layout(l, w, s).unwrap();
            assert!(wo >= l as usize * size_of::<vk::VkCommandBufferSubmitInfo>());
            assert!(so >= wo + w as usize * size_of::<vk::VkSemaphoreSubmitInfo>());
            assert!(layout.size() >= so + s as usize * size_of::<vk::VkSemaphoreSubmitInfo>());
            assert_eq!(wo % align_of::<vk::VkSemaphoreSubmitInfo>(), 0);
            assert_eq!(so % align_of::<vk::VkSemaphoreSubmitInfo>(), 0);
        }
        assert_eq!(access(64, 1), Err(INVALID));
        assert_eq!(access(64, 256), Ok(vk::VK_ACCESS_2_HOST_READ_BIT));
        assert_eq!(access(8, 0), Err(INVALID));
        assert_eq!(access(16, 2), Err(INVALID));
        assert_eq!(access(1 << 25, 1), Err(UNSUPPORTED));
        assert_eq!(
            access(1 << 16, 4),
            Ok(vk::VK_ACCESS_2_RESOURCE_HEAP_READ_BIT_EXT)
        );
        assert_eq!(
            access(1 << 17, 16),
            Ok(vk::VK_ACCESS_2_SAMPLER_HEAP_READ_BIT_EXT)
        );
        assert_eq!(access(1 << 16, 2), Err(INVALID));
        assert_eq!(access(1 << 17, 256), Err(INVALID));
        let layout = submit_layout(1, 0, 0).unwrap().0;
        let req = HostRequirements {
            size: layout.size() as u64,
            alignment: layout.align() as u64,
        };
        assert_eq!(scratch(ptr::null_mut(), 0, req), Err(CAPACITY));
        assert_eq!(scratch(ptr::null_mut(), req.size, req), Err(INVALID));
    }
}
