//! Prepared native compute state. Borrowed device, explicit artifact ABI, no
//! resource retention, command-time compilation, argument upload or implicit cache.
use super::*;
use std::ffi::CStr;
#[path = "graphics_pipeline.rs"]
mod graphics_pipeline;
#[path = "subgroups.rs"]
mod subgroups;

pub struct Executable {
    pub(super) device: *const Device,
    pub(super) pipeline: vk::VkPipeline,
    pub(super) byte_size: u32,
    pub(super) roots: Vec<RootSlot>,
    pub(super) stages: u64,
}
impl Drop for Executable {
    fn drop(&mut self) {
        if !self.pipeline.is_null() {
            let d = unsafe { &*self.device };
            unsafe { (d.f.vkDestroyPipeline.unwrap())(d.handle, self.pipeline, ptr::null()) };
        }
    }
}
pub(in crate::foundation) unsafe fn record<'a, T>(
    p: *const Record,
    kind: u32,
) -> Result<&'a T, Status> {
    if p.is_null() {
        return Err(INVALID);
    }
    unsafe { p.read() }.validate::<T>(kind)?;
    Ok(unsafe { &*p.cast() })
}
unsafe fn array<'a, T>(p: *const T, count: u32) -> Result<&'a [T], Status> {
    if count == 0 {
        return Ok(&[]);
    }
    if p.is_null() || count as u64 > isize::MAX as u64 / size_of::<T>() as u64 {
        return Err(INVALID);
    }
    Ok(unsafe { std::slice::from_raw_parts(p, count as usize) })
}
fn interface(
    limits: &ExecutionLimits,
    input: &ArgumentInterface,
    roots: &[RootSlot],
) -> Result<(), Status> {
    if input.byte_size % 4 != 0 {
        return Err(INVALID);
    }
    if u64::from(input.byte_size) > limits.max_inline_size {
        return Err(UNSUPPORTED);
    }
    for (i, root) in roots.iter().enumerate() {
        if root.stages == 0 || root.stages & !(4 | 8 | 16) != 0 {
            return Err(UNSUPPORTED);
        }
        if root.offset % 8 != 0
            || root
                .offset
                .checked_add(8)
                .is_none_or(|end| end > input.byte_size)
            || !root.alignment.is_power_of_two()
        {
            return Err(INVALID);
        }
        // Cold preparation only. No overlap scan or hash map in set_root.
        if roots[..i].iter().any(|r| r.offset == root.offset) {
            return Err(INVALID);
        }
    }
    Ok(())
}
fn requirements(d: &Device, req: &ShaderRequirements) -> Result<(), Status> {
    if req.features & !d.snapshot.features.enabled != 0 {
        return Err(UNSUPPORTED);
    }
    let limits = &d.snapshot.execution_limits;
    if req.local_size.contains(&0) {
        return Err(INVALID);
    }
    let count = req
        .local_size
        .iter()
        .try_fold(1u64, |n, v| n.checked_mul(u64::from(*v)))
        .ok_or(UNSUPPORTED)?;
    if req
        .local_size
        .iter()
        .zip(limits.max_local_size)
        .any(|(v, max)| *v > max)
        || count > u64::from(limits.max_local_invocations)
        || req.shared_memory > limits.max_shared_memory
    {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
impl Executable {
    pub(in crate::foundation) unsafe fn create(
        d: &Device,
        desc: &ExecutableDesc,
    ) -> Result<Box<Self>, Status> {
        d.ready()?;
        desc.header.validate::<ExecutableDesc>(EXECUTABLE_DESC)?;
        if desc.kind == 2 {
            return unsafe { graphics_pipeline::prepare(d, desc) };
        }
        if desc.kind != 1
            || desc.shader_count != 1
            || !desc.static_state.is_null()
            || desc.dynamic_state != 0
        {
            return Err(UNSUPPORTED);
        }
        let cache = unsafe { ExecutableCache::optional(d, desc.cache)? };
        let shader = unsafe { array(desc.shaders, 1)? }.first().unwrap();
        if shader.stage != 4 || shader.format != 0 {
            return Err(UNSUPPORTED);
        }
        if shader.code.data.is_null()
            || shader.code.data as usize % 4 != 0
            || shader.code.size < 20
            || shader.code.size % 4 != 0
            || shader.code.size > isize::MAX as usize
            || shader.entry.is_null()
        {
            return Err(INVALID);
        }
        let entry = unsafe { CStr::from_ptr(shader.entry) };
        if entry.to_bytes().is_empty() || entry.to_str().is_err() {
            return Err(INVALID);
        }
        if unsafe { shader.code.data.cast::<u32>().read() } != 0x07230203 {
            return Err(INVALID);
        }
        let abi =
            unsafe { record::<ArgumentInterface>(shader.interface_metadata, ARGUMENT_INTERFACE)? };
        let roots = unsafe { array(abi.roots, abi.root_count)? };
        interface(&d.snapshot.execution_limits, abi, roots)?;
        if roots.iter().any(|r| r.stages != 4) {
            return Err(UNSUPPORTED);
        }
        let req = unsafe { record::<ShaderRequirements>(desc.requirements, SHADER_REQUIREMENTS)? };
        requirements(d, req)?;
        let (subgroup_flags, subgroup) = unsafe { subgroups::prepare(d, shader, req.local_size)? };
        let mut entries = Vec::new();
        let mut spec = vk::VkSpecializationInfo::default();
        if !shader.specialization.is_null() {
            let s = unsafe { record::<Specialization>(shader.specialization, SPECIALIZATION)? };
            if s.reserved != 0 {
                return Err(UNSUPPORTED);
            }
            if s.data.size > isize::MAX as usize || (s.data.size != 0 && s.data.data.is_null()) {
                return Err(INVALID);
            }
            let values = unsafe { array(s.entries, s.count)? };
            entries
                .try_reserve_exact(values.len())
                .map_err(|_| OUT_OF_MEMORY)?;
            for (i, v) in values.iter().enumerate() {
                if v.size == 0
                    || v.size > s.data.size as u64
                    || u64::from(v.offset) > s.data.size as u64 - v.size
                    || values[..i].iter().any(|p| p.id == v.id)
                {
                    return Err(INVALID);
                }
                entries.push(vk::VkSpecializationMapEntry {
                    constantID: v.id,
                    offset: v.offset,
                    size: v.size as usize,
                });
            }
            spec = vk::VkSpecializationInfo {
                mapEntryCount: s.count,
                pMapEntries: entries.as_ptr(),
                dataSize: s.data.size,
                pData: s.data.data,
            };
        }
        let mut owned_roots = Vec::new();
        owned_roots
            .try_reserve_exact(roots.len())
            .map_err(|_| OUT_OF_MEMORY)?;
        owned_roots.extend_from_slice(roots);
        let mut result = Box::new(Self {
            device: d,
            pipeline: ptr::null_mut(),
            byte_size: abi.byte_size,
            roots: owned_roots,
            stages: 4,
        });
        let info = vk::VkShaderModuleCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO,
            codeSize: shader.code.size,
            pCode: shader.code.data.cast(),
            ..Default::default()
        };
        let mut module = ptr::null_mut();
        unsafe {
            d.result((d.f.vkCreateShaderModule.unwrap())(
                d.handle,
                &info,
                ptr::null(),
                &mut module,
            ))?;
        }
        let flags = vk::VkPipelineCreateFlags2CreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_CREATE_FLAGS_2_CREATE_INFO,
            flags: vk::VK_PIPELINE_CREATE_2_DESCRIPTOR_HEAP_BIT_EXT,
            ..Default::default()
        };
        let info = vk::VkComputePipelineCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMPUTE_PIPELINE_CREATE_INFO,
            pNext: ptr::from_ref(&flags).cast(),
            stage: vk::VkPipelineShaderStageCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
                flags: subgroup_flags,
                pNext: if subgroup.requiredSubgroupSize == 0 {
                    ptr::null()
                } else {
                    ptr::from_ref(&subgroup).cast()
                },
                stage: vk::VkShaderStageFlagBits_VK_SHADER_STAGE_COMPUTE_BIT,
                module,
                pName: shader.entry,
                pSpecializationInfo: &spec,
            },
            basePipelineIndex: -1,
            ..Default::default()
        };
        unsafe {
            // Pipeline creation guarantees each output is a valid pipeline or NULL,
            // including failure. Adopt for cleanup; unlike unspecified module output.
            let status = (d.f.vkCreateComputePipelines.unwrap())(
                d.handle,
                cache,
                1,
                &info,
                ptr::null(),
                &mut result.pipeline,
            );
            (d.f.vkDestroyShaderModule.unwrap())(d.handle, module, ptr::null());
            d.result(status)?;
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    #[ignore = "requires modern Vulkan; independent scalar feature enabling and artifact requirement rejection"]
    fn gpu_foundation_numerical_profiles() {
        check_numerical_profile(
            include_bytes!("../../../../examples/shaders/foundation-numerics.comp.spv"),
            c"numericMain",
            FLOAT16 | INT8 | INT16 | INT64 | FLOAT64 | STORAGE8,
            0,
        );
    }
    #[test]
    #[ignore = "requires modern Vulkan; independent buffer/shared atomic enabling and missing-domain rejection"]
    fn gpu_foundation_atomic_profiles() {
        check_numerical_profile(
            include_bytes!("../../../../examples/shaders/foundation-atomics.comp.spv"),
            c"atomicMain",
            INT64 | BUFFER_ATOMIC64 | SHARED_ATOMIC64,
            8,
        );
    }
    fn check_numerical_profile(bytes: &[u8], entry: &CStr, all: u64, shared_memory: u32) {
        let instance = Arc::new(Instance::new().unwrap());
        let code: Vec<u32> = bytes
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        let root = RootSlot {
            stages: 4,
            offset: 0,
            alignment: 8,
        };
        let abi = ArgumentInterface {
            header: Record::new::<ArgumentInterface>(ARGUMENT_INTERFACE),
            byte_size: 8,
            root_count: 1,
            roots: &root,
        };
        let requirements = ShaderRequirements {
            header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
            features: all,
            local_size: [64, 1, 1],
            shared_memory,
        };
        let shader = Shader {
            stage: 4,
            format: 0,
            code: Bytes {
                data: code.as_ptr().cast(),
                size: size_of_val(code.as_slice()),
            },
            entry: entry.as_ptr(),
            interface_metadata: &abi.header,
            specialization: ptr::null(),
            subgroup: ptr::null(),
        };
        let desc = ExecutableDesc {
            header: Record::new::<ExecutableDesc>(EXECUTABLE_DESC),
            kind: 1,
            shader_count: 1,
            shaders: &shader,
            static_state: ptr::null(),
            dynamic_state: 0,
            requirements: &requirements.header,
            cache: ptr::null_mut(),
        };
        let mut tested = 0;
        for physical in instance.physical_devices().unwrap() {
            let snapshot = snapshot(&instance, physical).unwrap();
            if snapshot.features.baseline_supported == 0 {
                continue;
            }
            let Some(q) = snapshot
                .queues
                .iter()
                .find(|q| q.count > 0 && q.flags & COMPUTE != 0)
            else {
                continue;
            };
            let request = [QueueRequest {
                domain: q.domain,
                count: 1,
                priority: 0.5,
            }];
            let available = snapshot.features.available;
            let adapter = Adapter {
                instance: instance.clone(),
                physical,
                snapshot,
            };
            // Exercise each independent bit and each otherwise-complete profile
            // missing one required bit, not merely a device with everything disabled.
            let profiles = [0, all].into_iter().chain(
                (0..64)
                    .map(|n| 1u64 << n)
                    .filter(|bit| all & bit != 0)
                    .flat_map(|bit| [bit, all & !bit]),
            );
            for enabled in profiles {
                if enabled & !available != 0 {
                    continue;
                }
                let device = Device::create(&adapter, &request, enabled).unwrap();
                assert_eq!(device.snapshot.features.enabled, enabled);
                let executable = unsafe { Executable::create(&device, &desc) };
                if enabled == all {
                    assert!(executable.is_ok());
                } else {
                    assert!(matches!(executable, Err(UNSUPPORTED)));
                }
            }
            tested += 1;
        }
        assert!(tested > 0, "No suitable device; not a passing skip");
    }

    #[test]
    fn argument_abi_limits_and_root_layout() {
        let limits = ExecutionLimits {
            max_inline_size: 64,
            ..Default::default()
        };
        let mut input = ArgumentInterface {
            header: Record::new::<ArgumentInterface>(ARGUMENT_INTERFACE),
            byte_size: 24,
            root_count: 2,
            roots: ptr::null(),
        };
        let mut roots = [
            RootSlot {
                stages: 4,
                offset: 0,
                alignment: 8,
            },
            RootSlot {
                stages: 4,
                offset: 8,
                alignment: 4,
            },
        ];
        assert_eq!(interface(&limits, &input, &roots), Ok(()));
        roots[1].offset = 0;
        assert_eq!(interface(&limits, &input, &roots), Err(INVALID));
        roots[1].offset = 20;
        assert_eq!(interface(&limits, &input, &roots), Err(INVALID));
        roots[1].offset = 8;
        roots[1].alignment = 3;
        assert_eq!(interface(&limits, &input, &roots), Err(INVALID));
        roots[1].alignment = 4;
        roots[1].stages = 32;
        assert_eq!(interface(&limits, &input, &roots), Err(UNSUPPORTED));
        input.byte_size = 68;
        assert_eq!(interface(&limits, &input, &[]), Err(UNSUPPORTED));
        input.byte_size = 3;
        assert_eq!(interface(&limits, &input, &[]), Err(INVALID));
        input.byte_size = 0;
        assert_eq!(interface(&limits, &input, &[]), Ok(()));
    }
    thread_local! {
        static FAILURE: Cell<u32> = const { Cell::new(0) };
        static CREATE_MODULE: Cell<vk::PFN_vkCreateShaderModule> = const { Cell::new(None) };
        static CREATE_PIPELINE: Cell<vk::PFN_vkCreateComputePipelines> = const { Cell::new(None) };
        static CREATE_GRAPHICS: Cell<vk::PFN_vkCreateGraphicsPipelines> = const { Cell::new(None) };
        static MODULE_CALLS: Cell<u32> = const { Cell::new(0) };
        static DESTROY_MODULE: Cell<vk::PFN_vkDestroyShaderModule> = const { Cell::new(None) };
        static DESTROY_PIPELINE: Cell<vk::PFN_vkDestroyPipeline> = const { Cell::new(None) };
        static MODULE_DROPS: Cell<u32> = const { Cell::new(0) };
        static PIPELINE_DROPS: Cell<u32> = const { Cell::new(0) };
    }
    unsafe extern "C" fn module(
        d: vk::VkDevice,
        info: *const vk::VkShaderModuleCreateInfo,
        a: *const vk::VkAllocationCallbacks,
        out: *mut vk::VkShaderModule,
    ) -> vk::VkResult {
        MODULE_CALLS.set(MODULE_CALLS.get() + 1);
        if FAILURE.get() == 1 || (FAILURE.get() == 4 && MODULE_CALLS.get() == 2) {
            unsafe {
                out.write(ptr::dangling_mut());
            }
            return vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY;
        }
        unsafe { CREATE_MODULE.get().unwrap()(d, info, a, out) }
    }
    unsafe extern "C" fn pipeline(
        d: vk::VkDevice,
        cache: vk::VkPipelineCache,
        count: u32,
        info: *const vk::VkComputePipelineCreateInfo,
        a: *const vk::VkAllocationCallbacks,
        out: *mut vk::VkPipeline,
    ) -> vk::VkResult {
        if FAILURE.get() == 2 {
            unsafe {
                out.write(ptr::null_mut());
            }
            return vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY;
        }
        let status = unsafe { CREATE_PIPELINE.get().unwrap()(d, cache, count, info, a, out) };
        if FAILURE.get() == 3 && status == vk::VkResult_VK_SUCCESS {
            // Vulkan permits successfully created outputs on a failed pipeline call.
            return vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY;
        }
        status
    }
    unsafe extern "C" fn destroy_module(
        d: vk::VkDevice,
        m: vk::VkShaderModule,
        a: *const vk::VkAllocationCallbacks,
    ) {
        MODULE_DROPS.set(MODULE_DROPS.get() + 1);
        unsafe {
            DESTROY_MODULE.get().unwrap()(d, m, a);
        }
    }
    unsafe extern "C" fn graphics_pipeline(
        d: vk::VkDevice,
        cache: vk::VkPipelineCache,
        count: u32,
        info: *const vk::VkGraphicsPipelineCreateInfo,
        a: *const vk::VkAllocationCallbacks,
        out: *mut vk::VkPipeline,
    ) -> vk::VkResult {
        if FAILURE.get() == 2 {
            unsafe {
                out.write(ptr::null_mut());
            }
            return vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY;
        }
        let status = unsafe { CREATE_GRAPHICS.get().unwrap()(d, cache, count, info, a, out) };
        if FAILURE.get() == 3 && status == vk::VkResult_VK_SUCCESS {
            return vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY;
        }
        status
    }
    #[test]
    #[ignore = "requires modern Vulkan raster; graphics module and partial pipeline failure cleanup"]
    fn gpu_foundation_graphics_preparation() {
        use crate::foundation::{ogpu_next_executable_create, ogpu_next_executable_destroy};
        let instance = Arc::new(Instance::new().unwrap());
        let mut tested = 0;
        for physical in instance.physical_devices().unwrap() {
            let snapshot = snapshot(&instance, physical).unwrap();
            if snapshot.features.baseline_supported == 0
                || snapshot.features.available & RASTER == 0
            {
                continue;
            }
            let domain = snapshot
                .queues
                .iter()
                .find(|q| q.count != 0 && q.flags & GRAPHICS != 0)
                .unwrap()
                .domain;
            let adapter = Adapter {
                instance: instance.clone(),
                physical,
                snapshot,
            };
            let mut device = Device::create(
                &adapter,
                &[QueueRequest {
                    domain,
                    count: 1,
                    priority: 0.5,
                }],
                RASTER,
            )
            .unwrap();
            // Install instrumentation before creating any borrowed children.
            CREATE_MODULE.set(device.f.vkCreateShaderModule);
            device.f.vkCreateShaderModule = Some(module);
            CREATE_GRAPHICS.set(device.f.vkCreateGraphicsPipelines);
            device.f.vkCreateGraphicsPipelines = Some(graphics_pipeline);
            DESTROY_MODULE.set(device.f.vkDestroyShaderModule);
            device.f.vkDestroyShaderModule = Some(destroy_module);
            DESTROY_PIPELINE.set(device.f.vkDestroyPipeline);
            device.f.vkDestroyPipeline = Some(destroy_pipeline);
            let d = ptr::from_ref(&*device).cast_mut();
            let bytes: [&[u8]; 2] = [
                include_bytes!("../../../../examples/shaders/foundation-graphics.vert.spv"),
                include_bytes!("../../../../examples/shaders/foundation-graphics.frag.spv"),
            ];
            let code: Vec<Vec<u32>> = bytes
                .iter()
                .map(|b| {
                    b.chunks_exact(4)
                        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                        .collect()
                })
                .collect();
            let root = RootSlot {
                stages: 24,
                offset: 0,
                alignment: 16,
            };
            let abi = ArgumentInterface {
                header: Record::new::<ArgumentInterface>(ARGUMENT_INTERFACE),
                byte_size: 8,
                root_count: 1,
                roots: &root,
            };
            let shaders: Vec<Shader> = (0..2)
                .map(|i| Shader {
                    stage: if i == 0 { 8 } else { 16 },
                    format: 0,
                    code: Bytes {
                        data: code[i].as_ptr().cast(),
                        size: code[i].len() * 4,
                    },
                    entry: if i == 0 {
                        c"vertexMain".as_ptr()
                    } else {
                        c"fragmentMain".as_ptr()
                    },
                    interface_metadata: &abi.header,
                    specialization: ptr::null(),
                    subgroup: ptr::null(),
                })
                .collect();
            let color = ColorState {
                format: 3,
                write_mask: 15,
                blend: 0,
                src_color: 1,
                dst_color: 0,
                color_op: 0,
                src_alpha: 1,
                dst_alpha: 0,
                alpha_op: 0,
            };
            let state = GraphicsState {
                header: Record::new::<GraphicsState>(GRAPHICS_STATE),
                topology: 3,
                cull: 0,
                front_face: 0,
                samples: 1,
                color_count: 1,
                colors: &color,
                depth_format: 0,
                depth_test: 0,
                depth_write: 0,
                depth_compare: 0,
                blend_constants: [0.0; 4],
                stencil_test: 0,
                stencil_front: StencilState::default(),
                stencil_back: StencilState::default(),
                vertex_input: ptr::null(),
            };
            let req = ShaderRequirements {
                header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
                features: RASTER,
                local_size: [0; 3],
                shared_memory: 0,
            };
            let desc = ExecutableDesc {
                header: Record::new::<ExecutableDesc>(EXECUTABLE_DESC),
                kind: 2,
                shader_count: 2,
                shaders: shaders.as_ptr(),
                static_state: &state.header,
                dynamic_state: 1,
                requirements: &req.header,
                cache: ptr::null_mut(),
            };
            for (mode, modules, pipelines) in
                [(1, 0, 0), (4, 1, 0), (2, 2, 0), (3, 2, 1), (0, 2, 1)]
            {
                FAILURE.set(mode);
                MODULE_CALLS.set(0);
                MODULE_DROPS.set(0);
                PIPELINE_DROPS.set(0);
                let mut out = ptr::dangling_mut();
                let status = unsafe { ogpu_next_executable_create(d, &desc, &mut out) };
                assert_eq!(status, if mode == 0 { OK } else { OUT_OF_MEMORY });
                assert_eq!(out.is_null(), mode != 0);
                unsafe {
                    ogpu_next_executable_destroy(out);
                }
                assert_eq!(MODULE_DROPS.get(), modules);
                assert_eq!(PIPELINE_DROPS.get(), pipelines);
                assert_eq!(device.ready(), Ok(()));
            }
            tested += 1;
        }
        assert!(tested > 0, "No suitable raster device; not a passing skip");
    }
    unsafe extern "C" fn destroy_pipeline(
        d: vk::VkDevice,
        p: vk::VkPipeline,
        a: *const vk::VkAllocationCallbacks,
    ) {
        PIPELINE_DROPS.set(PIPELINE_DROPS.get() + 1);
        unsafe {
            DESTROY_PIPELINE.get().unwrap()(d, p, a);
        }
    }
    #[test]
    #[ignore = "requires modern Vulkan; executable artifact ownership and partial native failure cleanup"]
    fn gpu_foundation_executables() {
        use crate::foundation::{ogpu_next_executable_create, ogpu_next_executable_destroy};
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
                .find(|q| q.count != 0 && q.flags & COMPUTE != 0)
                .unwrap()
                .domain;
            let adapter = Adapter {
                instance: instance.clone(),
                physical,
                snapshot,
            };
            let mut device = Device::create(
                &adapter,
                &[QueueRequest {
                    domain,
                    count: 1,
                    priority: 0.5,
                }],
                0,
            )
            .unwrap();
            CREATE_MODULE.set(device.f.vkCreateShaderModule);
            device.f.vkCreateShaderModule = Some(module);
            CREATE_PIPELINE.set(device.f.vkCreateComputePipelines);
            device.f.vkCreateComputePipelines = Some(pipeline);
            DESTROY_MODULE.set(device.f.vkDestroyShaderModule);
            device.f.vkDestroyShaderModule = Some(destroy_module);
            DESTROY_PIPELINE.set(device.f.vkDestroyPipeline);
            device.f.vkDestroyPipeline = Some(destroy_pipeline);
            let d = ptr::from_ref(&*device).cast_mut();
            let bytes = include_bytes!("../../../../examples/shaders/foundation-compute.comp.spv");
            let code: Vec<u32> = bytes
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .collect();
            let root = RootSlot {
                stages: 4,
                offset: 0,
                alignment: 8,
            };
            let abi = ArgumentInterface {
                header: Record::new::<ArgumentInterface>(ARGUMENT_INTERFACE),
                byte_size: 24,
                root_count: 1,
                roots: &root,
            };
            let req = ShaderRequirements {
                header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
                features: 0,
                local_size: [64, 1, 1],
                shared_memory: 0,
            };
            let shader = Shader {
                stage: 4,
                format: 0,
                code: Bytes {
                    data: code.as_ptr().cast(),
                    size: size_of_val(code.as_slice()),
                },
                entry: c"transform".as_ptr(),
                interface_metadata: &abi.header,
                specialization: ptr::null(),
                subgroup: ptr::null(),
            };
            let desc = ExecutableDesc {
                header: Record::new::<ExecutableDesc>(EXECUTABLE_DESC),
                kind: 1,
                shader_count: 1,
                shaders: &shader,
                static_state: ptr::null(),
                dynamic_state: 0,
                requirements: &req.header,
                cache: ptr::null_mut(),
            };
            for (mode, modules, pipelines) in [(1, 0, 0), (2, 1, 0), (3, 1, 1), (0, 1, 1)] {
                FAILURE.set(mode);
                MODULE_DROPS.set(0);
                PIPELINE_DROPS.set(0);
                let mut out = ptr::dangling_mut();
                let status = unsafe { ogpu_next_executable_create(d, &desc, &mut out) };
                assert_eq!(status, if mode == 0 { OK } else { OUT_OF_MEMORY });
                if mode != 0 {
                    assert!(out.is_null());
                }
                unsafe {
                    ogpu_next_executable_destroy(out);
                }
                assert_eq!(MODULE_DROPS.get(), modules);
                assert_eq!(PIPELINE_DROPS.get(), pipelines);
                assert_eq!(device.ready(), Ok(()));
            }
            tested += 1;
        }
        assert!(tested > 0, "No suitable device; not a passing skip");
    }
}
