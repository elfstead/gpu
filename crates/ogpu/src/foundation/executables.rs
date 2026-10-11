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
fn compile_flags(enabled: u64, flags: u32, reserved: u32) -> Result<u64, Status> {
    if flags & !1 != 0 || reserved != 0 || (flags != 0 && enabled & CACHE_CONTROL == 0) {
        return Err(UNSUPPORTED);
    }
    Ok(vk::VK_PIPELINE_CREATE_2_DESCRIPTOR_HEAP_BIT_EXT
        | if flags & 1 != 0 {
            vk::VK_PIPELINE_CREATE_2_FAIL_ON_PIPELINE_COMPILE_REQUIRED_BIT
        } else {
            0
        })
}
impl Executable {
    pub(in crate::foundation) unsafe fn create(
        d: &Device,
        desc: &ExecutableDesc,
    ) -> Result<Box<Self>, Status> {
        d.ready()?;
        desc.header.validate::<ExecutableDesc>(EXECUTABLE_DESC)?;
        let pipeline_flags = compile_flags(
            d.snapshot.features.enabled,
            desc.compile_flags,
            desc.reserved,
        )?;
        if desc.kind == 2 {
            return unsafe { graphics_pipeline::prepare(d, desc, pipeline_flags) };
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
            flags: pipeline_flags,
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
    #[test]
    fn compilation_policy_is_explicit() {
        let base = vk::VK_PIPELINE_CREATE_2_DESCRIPTOR_HEAP_BIT_EXT;
        assert_eq!(compile_flags(0, 0, 0), Ok(base));
        assert_eq!(compile_flags(0, 1, 0), Err(UNSUPPORTED));
        assert_eq!(compile_flags(CACHE_CONTROL, 2, 0), Err(UNSUPPORTED));
        assert_eq!(compile_flags(CACHE_CONTROL, 0, 1), Err(UNSUPPORTED));
        assert_eq!(
            compile_flags(CACHE_CONTROL, 1, 0),
            Ok(base | vk::VK_PIPELINE_CREATE_2_FAIL_ON_PIPELINE_COMPILE_REQUIRED_BIT)
        );
    }
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
    #[test]
    #[ignore = "requires modern Vulkan; independently enabled native float atomic operation/domain profiles"]
    fn gpu_foundation_float_atomic_profiles() {
        for (bytes, entry, required, shared) in [
            (include_bytes!("../../../../examples/shaders/foundation-float16-atomics.buffer-add.comp.spv").as_slice(), c"bufferAdd", FLOAT16 | BUFFER_FLOAT16_ATOMIC_ADD, 0),
            (include_bytes!("../../../../examples/shaders/foundation-float16-atomics.shared-add.comp.spv").as_slice(), c"sharedAdd", FLOAT16 | SHARED_FLOAT16_ATOMIC_ADD, 2),
            (include_bytes!("../../../../examples/shaders/foundation-float16-atomics.buffer-minmax.comp.spv").as_slice(), c"bufferMinMax", FLOAT16 | BUFFER_FLOAT16_ATOMIC_MIN_MAX, 0),
            (include_bytes!("../../../../examples/shaders/foundation-float16-atomics.shared-minmax.comp.spv").as_slice(), c"sharedMinMax", FLOAT16 | SHARED_FLOAT16_ATOMIC_MIN_MAX, 4),
            (include_bytes!("../../../../examples/shaders/foundation-float16-atomics.buffer-exchange.comp.spv").as_slice(), c"bufferExchange", FLOAT16 | BUFFER_FLOAT16_ATOMICS, 0),
            (include_bytes!("../../../../examples/shaders/foundation-float16-atomics.shared-exchange.comp.spv").as_slice(), c"sharedExchange", FLOAT16 | SHARED_FLOAT16_ATOMICS, 2),
            (include_bytes!("../../../../examples/shaders/foundation-float-atomics.buffer-add.comp.spv").as_slice(), c"bufferAdd", BUFFER_FLOAT32_ATOMIC_ADD, 0),
            (include_bytes!("../../../../examples/shaders/foundation-float-atomics.shared-add.comp.spv").as_slice(), c"sharedAdd", SHARED_FLOAT32_ATOMIC_ADD, 4),
            (include_bytes!("../../../../examples/shaders/foundation-float-atomics.buffer-minmax.comp.spv").as_slice(), c"bufferMinMax", BUFFER_FLOAT32_ATOMIC_MIN_MAX, 0),
            (include_bytes!("../../../../examples/shaders/foundation-float-atomics.shared-minmax.comp.spv").as_slice(), c"sharedMinMax", SHARED_FLOAT32_ATOMIC_MIN_MAX, 8),
            (include_bytes!("../../../../examples/shaders/foundation-float-atomics.buffer-exchange.comp.spv").as_slice(), c"bufferExchange", BUFFER_FLOAT32_ATOMICS, 0),
            (include_bytes!("../../../../examples/shaders/foundation-float-atomics.shared-exchange.comp.spv").as_slice(), c"sharedExchange", SHARED_FLOAT32_ATOMICS, 4),
            (include_bytes!("../../../../examples/shaders/foundation-float64-atomics.buffer-add.comp.spv").as_slice(), c"bufferAdd", FLOAT64 | BUFFER_FLOAT64_ATOMIC_ADD, 0),
            (include_bytes!("../../../../examples/shaders/foundation-float64-atomics.shared-add.comp.spv").as_slice(), c"sharedAdd", FLOAT64 | SHARED_FLOAT64_ATOMIC_ADD, 8),
            (include_bytes!("../../../../examples/shaders/foundation-float64-atomics.buffer-minmax.comp.spv").as_slice(), c"bufferMinMax", FLOAT64 | BUFFER_FLOAT64_ATOMIC_MIN_MAX, 0),
            (include_bytes!("../../../../examples/shaders/foundation-float64-atomics.shared-minmax.comp.spv").as_slice(), c"sharedMinMax", FLOAT64 | SHARED_FLOAT64_ATOMIC_MIN_MAX, 16),
            (include_bytes!("../../../../examples/shaders/foundation-float64-atomics.buffer-exchange.comp.spv").as_slice(), c"bufferExchange", FLOAT64 | BUFFER_FLOAT64_ATOMICS, 0),
            (include_bytes!("../../../../examples/shaders/foundation-float64-atomics.shared-exchange.comp.spv").as_slice(), c"sharedExchange", FLOAT64 | SHARED_FLOAT64_ATOMICS, 8),
        ] { check_numerical_profile(bytes, entry, required, shared); }
    }
    #[test]
    #[ignore = "requires modern Vulkan; independent packed integer-dot enabling and native preparation"]
    fn gpu_foundation_integer_dot_profile() {
        check_numerical_profile(
            include_bytes!("../../../../examples/shaders/foundation-integer-dot.comp.spv"),
            c"dotMain",
            INTEGER_DOT,
            0,
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
            compile_flags: 0,
            reserved: 0,
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
            // Float operations are independent even within the same width/domain.
            let profiles = profiles.chain(
                (0..64)
                    .map(|n| 1u64 << n)
                    .filter(|bit| all & float_atomics::ALL != 0 && bit & float_atomics::ALL != 0),
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
        static DYNAMIC_MASK: Cell<u64> = const { Cell::new(0) };
        static DYNAMIC_COUNT: Cell<u32> = const { Cell::new(0) };
        static COLOR_LOGIC: Cell<(u32, u32, u32)> = const { Cell::new((0, 0, 0)) };
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
        if FAILURE.get() == 5 {
            let flags = unsafe { &*(*info).pNext.cast::<vk::VkPipelineCreateFlags2CreateInfo>() };
            assert_ne!(
                flags.flags & vk::VK_PIPELINE_CREATE_2_FAIL_ON_PIPELINE_COMPILE_REQUIRED_BIT,
                0
            );
            unsafe {
                out.write(ptr::null_mut());
            }
            return vk::VkResult_VK_PIPELINE_COMPILE_REQUIRED;
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
        let dynamic = unsafe { &*(*info).pDynamicState };
        let blend = unsafe { &*(*info).pColorBlendState };
        COLOR_LOGIC.set((
            blend.logicOpEnable,
            blend.logicOp,
            if blend.attachmentCount == 0 {
                0
            } else {
                unsafe { (*blend.pAttachments).blendEnable }
            },
        ));
        let states = unsafe {
            std::slice::from_raw_parts(dynamic.pDynamicStates, dynamic.dynamicStateCount as usize)
        };
        let mut mask = 0;
        for state in states {
            mask |= match *state {
                vk::VkDynamicState_VK_DYNAMIC_STATE_VIEWPORT
                | vk::VkDynamicState_VK_DYNAMIC_STATE_SCISSOR => 1,
                vk::VkDynamicState_VK_DYNAMIC_STATE_BLEND_CONSTANTS => 2,
                vk::VkDynamicState_VK_DYNAMIC_STATE_STENCIL_REFERENCE => 4,
                vk::VkDynamicState_VK_DYNAMIC_STATE_STENCIL_COMPARE_MASK => 8,
                vk::VkDynamicState_VK_DYNAMIC_STATE_STENCIL_WRITE_MASK => 16,
                _ => u64::MAX,
            };
        }
        DYNAMIC_MASK.set(mask);
        DYNAMIC_COUNT.set(dynamic.dynamicStateCount);
        if FAILURE.get() == 2 {
            unsafe {
                out.write(ptr::null_mut());
            }
            return vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY;
        }
        if FAILURE.get() == 5 {
            // Graphics chains rendering info before the flags record.
            let rendering = unsafe { &*(*info).pNext.cast::<vk::VkPipelineRenderingCreateInfo>() };
            let flags = unsafe {
                &*rendering
                    .pNext
                    .cast::<vk::VkPipelineCreateFlags2CreateInfo>()
            };
            assert_ne!(
                flags.flags & vk::VK_PIPELINE_CREATE_2_FAIL_ON_PIPELINE_COMPILE_REQUIRED_BIT,
                0
            );
            unsafe {
                out.write(ptr::null_mut());
            }
            return vk::VkResult_VK_PIPELINE_COMPILE_REQUIRED;
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
                RASTER | (adapter.snapshot.features.available & CACHE_CONTROL),
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
                raster_options: ptr::null(),
                logic_op_enable: 0,
                logic_op: 0,
            };
            let req = ShaderRequirements {
                header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
                features: RASTER,
                local_size: [0; 3],
                shared_memory: 0,
            };
            let mut desc = ExecutableDesc {
                header: Record::new::<ExecutableDesc>(EXECUTABLE_DESC),
                kind: 2,
                shader_count: 2,
                shaders: shaders.as_ptr(),
                static_state: &state.header,
                dynamic_state: 1,
                requirements: &req.header,
                cache: ptr::null_mut(),
                compile_flags: 0,
                reserved: 0,
            };
            for (mode, modules, pipelines) in [
                (1, 0, 0),
                (4, 1, 0),
                (2, 2, 0),
                (3, 2, 1),
                (5, 2, 0),
                (0, 2, 1),
            ] {
                if mode == 5 && device.snapshot.features.enabled & CACHE_CONTROL == 0 {
                    continue;
                }
                desc.compile_flags = u32::from(mode == 5);
                FAILURE.set(mode);
                MODULE_CALLS.set(0);
                MODULE_DROPS.set(0);
                PIPELINE_DROPS.set(0);
                let mut out = ptr::dangling_mut();
                let status = unsafe { ogpu_next_executable_create(d, &desc, &mut out) };
                assert_eq!(
                    status,
                    match mode {
                        0 => OK,
                        5 => COMPILE_REQUIRED,
                        _ => OUT_OF_MEMORY,
                    }
                );
                assert_eq!(out.is_null(), mode != 0);
                unsafe {
                    ogpu_next_executable_destroy(out);
                }
                assert_eq!(MODULE_DROPS.get(), modules);
                assert_eq!(PIPELINE_DROPS.get(), pipelines);
                assert_eq!(device.ready(), Ok(()));
            }
            FAILURE.set(0);
            desc.compile_flags = 0;
            // All combinations of the independently selectable native states.
            for optional in 0..16 {
                desc.dynamic_state = 1 | (optional << 1);
                let mut out = ptr::null_mut();
                DYNAMIC_MASK.set(u64::MAX);
                assert_eq!(
                    unsafe { ogpu_next_executable_create(d, &desc, &mut out) },
                    OK
                );
                assert_eq!(DYNAMIC_MASK.get(), desc.dynamic_state);
                assert_eq!(DYNAMIC_COUNT.get(), desc.dynamic_state.count_ones() + 1);
                unsafe { ogpu_next_executable_destroy(out) };
            }
            for unsupported in [0, 2, 33, u64::MAX] {
                desc.dynamic_state = unsupported;
                let mut out = ptr::dangling_mut();
                DYNAMIC_MASK.set(0);
                assert_eq!(
                    unsafe { ogpu_next_executable_create(d, &desc, &mut out) },
                    UNSUPPORTED
                );
                assert!(out.is_null());
                assert_eq!(DYNAMIC_MASK.get(), 0); // Rejected before native preparation.
            }
            let profiles = [
                0,
                VERTEX_STORES_ATOMICS,
                FRAGMENT_STORES_ATOMICS,
                VERTEX_STORES_ATOMICS | FRAGMENT_STORES_ATOMICS,
                INDIRECT_FIRST_INSTANCE,
                INDEPENDENT_BLEND,
                INDIRECT_FIRST_INSTANCE | INDEPENDENT_BLEND,
                DEPTH_CLAMP,
                DEPTH_BIAS_CLAMP,
                NON_SOLID_FILL,
                WIDE_LINES,
                DEPTH_BOUNDS,
                SAMPLE_SHADING,
                ALPHA_TO_ONE,
                UNUSED_ATTACHMENTS,
                UNUSED_ATTACHMENTS | UNIFIED_IMAGES,
                LOGIC_OP,
                DUAL_SOURCE_BLEND,
                SHADER_OUTPUT_LAYER,
            ];
            let dual_code: Vec<u32> =
                include_bytes!("../../../../examples/shaders/foundation-dual-source.frag.spv")
                    .chunks_exact(4)
                    .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                    .collect();
            desc.dynamic_state = 1;
            let layer_code: Vec<u32> =
                include_bytes!("../../../../examples/shaders/foundation-layer.vert.spv")
                    .chunks_exact(4)
                    .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                    .collect();
            for enabled in profiles {
                if enabled & !adapter.snapshot.features.available != 0 {
                    continue;
                }
                let mut profile_device = Device::create(
                    &adapter,
                    &[QueueRequest {
                        domain,
                        count: 1,
                        priority: 0.5,
                    }],
                    RASTER | enabled,
                )
                .unwrap();
                CREATE_GRAPHICS.set(profile_device.f.vkCreateGraphicsPipelines);
                profile_device.f.vkCreateGraphicsPipelines = Some(graphics_pipeline);
                assert_eq!(profile_device.snapshot.features.enabled, RASTER | enabled);
                for required in profiles {
                    let requirements = ShaderRequirements {
                        header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
                        features: RASTER | required,
                        local_size: [0; 3],
                        shared_memory: 0,
                    };
                    desc.requirements = &requirements.header;
                    // The harmless artifact lets us verify requirements without issuing
                    // an invalid shader on disabled devices. C executes the writing artifacts.
                    let result = unsafe { Executable::create(&profile_device, &desc) };
                    if required & !enabled == 0 {
                        assert!(result.is_ok());
                    } else {
                        assert!(matches!(result, Err(UNSUPPORTED)));
                    }
                }
                let colors = [
                    color,
                    ColorState {
                        write_mask: 0,
                        ..color
                    },
                ];
                let mixed = GraphicsState {
                    color_count: 2,
                    colors: colors.as_ptr(),
                    ..state
                };
                let mixed_desc = ExecutableDesc {
                    static_state: &mixed.header,
                    requirements: &req.header,
                    ..desc
                };
                let result = unsafe { Executable::create(&profile_device, &mixed_desc) };
                if enabled & INDEPENDENT_BLEND != 0 {
                    assert!(result.is_ok());
                } else {
                    assert!(matches!(result, Err(UNSUPPORTED)));
                }
                let defaults = RasterOptions::default();
                // Use the actual Index=1 artifact, but reject disabled factors
                // before either native module or pipeline sees that interface.
                for field in 0..4 {
                    for factor in 15..=19 {
                        let mut dual_color = ColorState { blend: 1, ..color };
                        match field {
                            0 => dual_color.src_color = factor,
                            1 => dual_color.dst_color = factor,
                            2 => dual_color.src_alpha = factor,
                            _ => dual_color.dst_alpha = factor,
                        }
                        let dual_shader = Shader {
                            code: Bytes {
                                data: dual_code.as_ptr().cast(),
                                size: dual_code.len() * 4,
                            },
                            ..shaders[1]
                        };
                        let dual_shaders = [Shader { ..shaders[0] }, dual_shader];
                        let dual_state = GraphicsState {
                            colors: &dual_color,
                            ..state
                        };
                        let dual_desc = ExecutableDesc {
                            shaders: dual_shaders.as_ptr(),
                            static_state: &dual_state.header,
                            requirements: &req.header,
                            ..desc
                        };
                        COLOR_LOGIC.set((u32::MAX, u32::MAX, u32::MAX));
                        let result = unsafe { Executable::create(&profile_device, &dual_desc) };
                        if factor == 19 {
                            assert!(matches!(result, Err(INVALID)));
                        } else if enabled & DUAL_SOURCE_BLEND != 0 {
                            assert!(
                                profile_device
                                    .snapshot
                                    .graphics_limits
                                    .max_dual_source_attachments
                                    > 0
                            );
                            assert!(result.is_ok());
                        } else {
                            assert!(matches!(result, Err(UNSUPPORTED)));
                        }
                        if factor == 19 || enabled & DUAL_SOURCE_BLEND == 0 {
                            assert_eq!(COLOR_LOGIC.get(), (u32::MAX, u32::MAX, u32::MAX));
                        }
                    }
                }
                for op in 0..=15 {
                    let blend_color = ColorState { blend: 1, ..color };
                    let logic_state = GraphicsState {
                        logic_op_enable: 1,
                        logic_op: op,
                        colors: &blend_color,
                        ..state
                    };
                    let logic_desc = ExecutableDesc {
                        static_state: &logic_state.header,
                        requirements: &req.header,
                        ..desc
                    };
                    COLOR_LOGIC.set((u32::MAX, u32::MAX, u32::MAX));
                    let result = unsafe { Executable::create(&profile_device, &logic_desc) };
                    if enabled & LOGIC_OP != 0 {
                        assert!(result.is_ok());
                        assert_eq!(COLOR_LOGIC.get(), (1, op, 1)); // Native precedence; no mask/blend rewriting.
                    } else {
                        assert!(matches!(result, Err(UNSUPPORTED)));
                        assert_eq!(COLOR_LOGIC.get(), (u32::MAX, u32::MAX, u32::MAX));
                    }
                }
                for (enable, op) in [(2, 0), (0, 16), (1, u32::MAX)] {
                    let logic_state = GraphicsState {
                        logic_op_enable: enable,
                        logic_op: op,
                        ..state
                    };
                    let logic_desc = ExecutableDesc {
                        static_state: &logic_state.header,
                        requirements: &req.header,
                        ..desc
                    };
                    assert!(matches!(
                        unsafe { Executable::create(&profile_device, &logic_desc) },
                        Err(INVALID)
                    ));
                }
                let depth_only = GraphicsState {
                    color_count: 0,
                    depth_format: 14,
                    depth_test: 1,
                    depth_write: 1,
                    depth_compare: 1,
                    ..state
                };
                let depth_desc = ExecutableDesc {
                    shader_count: 1,
                    static_state: &depth_only.header,
                    requirements: &req.header,
                    ..desc
                };
                let depth_pipeline =
                    unsafe { Executable::create(&profile_device, &depth_desc) }.unwrap();
                assert_eq!(depth_pipeline.stages, 8);
                let layer_shader = Shader {
                    code: Bytes {
                        data: layer_code.as_ptr().cast(),
                        size: layer_code.len() * 4,
                    },
                    ..shaders[0]
                };
                let layer_req = ShaderRequirements {
                    features: RASTER | SHADER_OUTPUT_LAYER,
                    ..req
                };
                let layer_desc = ExecutableDesc {
                    shaders: &layer_shader,
                    requirements: &layer_req.header,
                    ..depth_desc
                };
                COLOR_LOGIC.set((u32::MAX, u32::MAX, u32::MAX));
                let layered = unsafe { Executable::create(&profile_device, &layer_desc) };
                if enabled & SHADER_OUTPUT_LAYER != 0 {
                    assert!(layered.is_ok());
                } else {
                    assert!(matches!(layered, Err(UNSUPPORTED)));
                    assert_eq!(COLOR_LOGIC.get(), (u32::MAX, u32::MAX, u32::MAX));
                }
                for count in [0, 3, u32::MAX] {
                    let invalid = ExecutableDesc {
                        shader_count: count,
                        ..depth_desc
                    };
                    assert!(matches!(
                        unsafe { Executable::create(&profile_device, &invalid) },
                        Err(UNSUPPORTED)
                    ));
                }
                // Undefined color slots are a native pipeline description even
                // without relaxed draw-time compatibility. No fake format query.
                let unused_color = ColorState { format: 0, ..color };
                let unused_state = GraphicsState {
                    colors: &unused_color,
                    ..state
                };
                let unused_desc = ExecutableDesc {
                    static_state: &unused_state.header,
                    requirements: &req.header,
                    ..desc
                };
                assert!(unsafe { Executable::create(&profile_device, &unused_desc) }.is_ok());
                for (bit, options) in [
                    (0, defaults),
                    (
                        0,
                        RasterOptions {
                            discard: 1,
                            ..defaults
                        },
                    ),
                    (
                        0,
                        RasterOptions {
                            primitive_restart: 1,
                            ..defaults
                        },
                    ),
                    (
                        0,
                        RasterOptions {
                            depth_bias: 1,
                            depth_bias_constant: -2.0,
                            depth_bias_slope: 3.0,
                            ..defaults
                        },
                    ),
                    (
                        0,
                        RasterOptions {
                            sample_mask: 0,
                            alpha_to_coverage: 1,
                            ..defaults
                        },
                    ),
                    (
                        DEPTH_CLAMP,
                        RasterOptions {
                            depth_clamp: 1,
                            ..defaults
                        },
                    ),
                    (
                        DEPTH_BIAS_CLAMP,
                        RasterOptions {
                            depth_bias: 1,
                            depth_bias_clamp: 0.5,
                            ..defaults
                        },
                    ),
                    (
                        NON_SOLID_FILL,
                        RasterOptions {
                            polygon_mode: 1,
                            ..defaults
                        },
                    ),
                    (
                        WIDE_LINES,
                        RasterOptions {
                            line_width: 2.0,
                            ..defaults
                        },
                    ),
                    (
                        DEPTH_BOUNDS,
                        RasterOptions {
                            depth_bounds: 1,
                            min_depth_bounds: 0.25,
                            max_depth_bounds: 0.75,
                            ..defaults
                        },
                    ),
                    (
                        SAMPLE_SHADING,
                        RasterOptions {
                            sample_shading: 1,
                            min_sample_shading: 0.5,
                            ..defaults
                        },
                    ),
                    (
                        ALPHA_TO_ONE,
                        RasterOptions {
                            alpha_to_one: 1,
                            ..defaults
                        },
                    ),
                ] {
                    let profile = GraphicsState {
                        topology: if options.primitive_restart != 0 { 4 } else { 3 },
                        color_count: if options.discard != 0 {
                            0
                        } else {
                            state.color_count
                        },
                        raster_options: &options.header,
                        ..state
                    };
                    let profile_desc = ExecutableDesc {
                        static_state: &profile.header,
                        shader_count: if options.discard != 0 { 1 } else { 2 },
                        requirements: &req.header,
                        ..desc
                    };
                    let result = unsafe { Executable::create(&profile_device, &profile_desc) };
                    let width_supported = bit != WIDE_LINES
                        || profile_device.snapshot.graphics_limits.line_width_range[1] >= 2.0;
                    if bit & !enabled == 0 && width_supported {
                        assert!(result.is_ok(), "raster option {bit}: {:?}", result.err());
                    } else {
                        assert!(matches!(result, Err(UNSUPPORTED)));
                    }
                }
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
                adapter.snapshot.features.available & CACHE_CONTROL,
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
            let mut desc = ExecutableDesc {
                header: Record::new::<ExecutableDesc>(EXECUTABLE_DESC),
                kind: 1,
                shader_count: 1,
                shaders: &shader,
                static_state: ptr::null(),
                dynamic_state: 0,
                requirements: &req.header,
                cache: ptr::null_mut(),
                compile_flags: 0,
                reserved: 0,
            };
            for (mode, modules, pipelines) in
                [(1, 0, 0), (2, 1, 0), (3, 1, 1), (5, 1, 0), (0, 1, 1)]
            {
                if mode == 5 && device.snapshot.features.enabled & CACHE_CONTROL == 0 {
                    continue;
                }
                desc.compile_flags = u32::from(mode == 5);
                FAILURE.set(mode);
                MODULE_DROPS.set(0);
                PIPELINE_DROPS.set(0);
                let mut out = ptr::dangling_mut();
                let status = unsafe { ogpu_next_executable_create(d, &desc, &mut out) };
                assert_eq!(
                    status,
                    match mode {
                        0 => OK,
                        5 => COMPILE_REQUIRED,
                        _ => OUT_OF_MEMORY,
                    }
                );
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
