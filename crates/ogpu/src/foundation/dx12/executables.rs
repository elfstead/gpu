//! Prepared D3D12 executable state: compute pipelines over DXIL artifacts with
//! the ROOT_DESCRIPTORS argument profile. No runtime shader translation; the
//! toolchain produces DXIL artifacts for this backend's argument ABI.
//!
//! Argument ABI (documented in docs/dx12-arguments-profile.md):
//! - the shared argument namespace of `byte_size` bytes maps to root constants,
//!   preserving every byte offset across backends (root-slot bytes are included
//!   but bind additionally through root descriptors);
//! - root slot `i` binds as a raw UAV root descriptor at `u(i)`, space 0;
//! - the namespace cbuffer occupies `b0`, space 0.
use super::*;
use std::ffi::CStr;

pub struct Executable {
    pub(super) device: *const Device,
    pub(super) pipeline: d3d::ID3D12PipelineState,
    pub(super) signature: d3d::ID3D12RootSignature,
    pub(super) byte_size: u32,
    pub(super) roots: Vec<RootSlot>,
    pub(super) stages: u64,
    /// Primitive topology of graphics executables (input-assembler state).
    pub(super) topology: u32,
    /// Per-binding vertex strides (input-assembler state); graphics only.
    pub(super) strides: Vec<u32>,
}

/// One unbounded descriptor range of the profile's tables.
fn unbounded_range(
    kind: d3d::D3D12_DESCRIPTOR_RANGE_TYPE,
    register: u32,
) -> d3d::D3D12_DESCRIPTOR_RANGE1 {
    d3d::D3D12_DESCRIPTOR_RANGE1 {
        RangeType: kind,
        NumDescriptors: u32::MAX,
        BaseShaderRegister: register,
        RegisterSpace: 1,
        Flags: d3d::D3D12_DESCRIPTOR_RANGE_FLAG_DESCRIPTORS_VOLATILE,
        OffsetInDescriptorsFromTableStart: 0,
    }
}

impl Executable {
    /// Root parameter index of one root slot: descriptors follow the optional
    /// namespace-constants parameter.
    pub(super) fn root_parameter(&self, slot: u32) -> u32 {
        u32::from(self.byte_size != 0) + slot
    }

    /// Root parameter index of the resource-descriptor tables (UAV/SRV share
    /// the heap range start; they are consecutive parameters).
    pub(super) fn table_parameter(&self) -> u32 {
        u32::from(self.byte_size != 0) + self.roots.len() as u32
    }

    /// Root parameter index of the sampler table.
    pub(super) fn sampler_parameter(&self) -> u32 {
        self.table_parameter() + 2
    }
}

impl Drop for Executable {
    fn drop(&mut self) {
        // COM references drop with the fields; the caller excluded all uses.
    }
}

/// Shared record validation of a nested description array.
pub(in crate::foundation) unsafe fn array<'a, T>(p: *const T, count: u32) -> Result<&'a [T], Status> {
    if count == 0 {
        return Ok(&[]);
    }
    if p.is_null() || count as u64 > isize::MAX as u64 / size_of::<T>() as u64 {
        return Err(INVALID);
    }
    // SAFETY: trusted C array extent; size arithmetic checked above.
    Ok(unsafe { std::slice::from_raw_parts(p, count as usize) })
}

/// Argument-interface validation, mirroring the shared contract exactly.
pub(in crate::foundation) fn interface(
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
        if root.stages == 0 || root.stages & !(STAGE_COMPUTE | MESH_STAGE | TASK_STAGE | 8 | 16) != 0
        {
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
        // Cold preparation only. No overlap scan in set_root.
        if roots[..i].iter().any(|r| r.offset == root.offset) {
            return Err(INVALID);
        }
    }
    Ok(())
}

/// Shader-requirement validation against device limits and enabled features.
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

/// Root-signature composition budget: one dword per root-constant declaration
/// plus its values, two dwords per root descriptor, within 64 dwords total.
fn signature_budget(byte_size: u32, roots: u32) -> Result<(), Status> {
    let constant_dwords = u64::from(byte_size / 4) + 1;
    let descriptor_dwords = u64::from(roots) * 2;
    if constant_dwords + descriptor_dwords > 64 {
        return Err(UNSUPPORTED);
    }
    Ok(())
}

impl Executable {
    /// Prepare compute state from a native DXIL artifact and its argument
    /// metadata. Preparation may allocate; recording never does.
    /// # Safety
    /// Live device and trusted description arrays; device outlives the object.
    pub(in crate::foundation) unsafe fn create(
        d: &Device,
        desc: &ExecutableDesc,
    ) -> Result<Box<Self>, Status> {
        d.ready()?;
        desc.header.validate::<ExecutableDesc>(EXECUTABLE_DESC)?;
        if desc.compile_flags != 0 || desc.reserved != 0 {
            // FAIL_IF_REQUIRED needs CACHE_CONTROL; INDIRECT_BINDABLE needs
            // GENERATED_COMMANDS. Neither is enabled in this profile yet.
            return Err(UNSUPPORTED);
        }
        if desc.kind == EXECUTABLE_GRAPHICS {
            // SAFETY: caller contract covers the description arrays.
            return unsafe { super::graphics::prepare(d, desc) };
        }
        if desc.kind != EXECUTABLE_COMPUTE
            || desc.shader_count != 1
            || !desc.static_state.is_null()
            || desc.dynamic_state != 0
        {
            return Err(UNSUPPORTED);
        }
        if !desc.cache.is_null() {
            // Pipeline-library caching is its own profile (D2 follow-up).
            return Err(UNSUPPORTED);
        }
        // SAFETY: one-element trusted shader array.
        let shader = unsafe { array(desc.shaders, 1)? }.first().unwrap();
        if shader.stage != STAGE_COMPUTE as u32 || shader.format != SHADER_DXIL {
            // SHADER_SPIRV artifacts carry the pointer-in-bytes ABI, which has
            // no DXIL representation; the toolchain emits DXIL for this backend.
            return Err(UNSUPPORTED);
        }
        // DXIL containers are byte-granular (their own header declares the
        // size); the word-granular size rule describes SPIR-V artifacts.
        if shader.code.data.is_null()
            || shader.code.data as usize % 4 != 0
            || shader.code.size < 20
            || shader.code.size > isize::MAX as usize
            || shader.entry.is_null()
        {
            return Err(INVALID);
        }
        // SAFETY: trusted NUL-terminated entry name.
        let entry = unsafe { CStr::from_ptr(shader.entry) };
        if entry.to_bytes().is_empty() || entry.to_str().is_err() {
            return Err(INVALID);
        }
        // DXIL container magic "DXBC".
        // SAFETY: code has at least four readable bytes by the checks above.
        if unsafe { shader.code.data.cast::<u32>().read() } != u32::from_le_bytes(*b"DXBC") {
            return Err(INVALID);
        }
        // SAFETY: complete ARGUMENT_INTERFACE record behind the pointer.
        let abi =
            unsafe { executable_record::<ArgumentInterface>(shader.interface_metadata, ARGUMENT_INTERFACE)? };
        // SAFETY: trusted root array of the declared count.
        let roots = unsafe { array(abi.roots, abi.root_count)? };
        interface(&d.snapshot.execution_limits, abi, roots)?;
        if roots.iter().any(|r| r.stages != STAGE_COMPUTE) {
            return Err(UNSUPPORTED);
        }
        if !shader.specialization.is_null() {
            // D3D12 has no specialization constants; a specialization strategy
            // is an open profile question (docs/dx12-backend-d2.md).
            return Err(UNSUPPORTED);
        }
        if !shader.subgroup.is_null() {
            return Err(UNSUPPORTED);
        }
        // SAFETY: complete SHADER_REQUIREMENTS record behind the pointer.
        let req = unsafe {
            executable_record::<ShaderRequirements>(desc.requirements, SHADER_REQUIREMENTS)?
        };
        requirements(d, req)?;
        signature_budget(abi.byte_size, abi.root_count)?;

        let signature = root_signature(d, abi, roots)?;
        let pipeline_desc = d3d::D3D12_COMPUTE_PIPELINE_STATE_DESC {
            pRootSignature: core::mem::ManuallyDrop::new(Some(unsafe {
                d3d::ID3D12RootSignature::from_raw(signature.as_raw())
            })),
            CS: d3d::D3D12_SHADER_BYTECODE {
                pShaderBytecode: shader.code.data,
                BytecodeLength: shader.code.size,
            },
            NodeMask: 0,
            CachedPSO: d3d::D3D12_CACHED_PIPELINE_STATE {
                pCachedBlob: ptr::null(),
                CachedBlobSizeInBytes: 0,
            },
            Flags: d3d::D3D12_PIPELINE_STATE_FLAG_NONE,
        };
        // SAFETY: prepared signature and artifact bytes stay valid for the call.
        let pipeline: d3d::ID3D12PipelineState = unsafe {
            d.device
                .CreateComputePipelineState(&pipeline_desc)
                .map_err(map_error)?
        };
        let mut owned_roots = Vec::new();
        owned_roots
            .try_reserve_exact(roots.len())
            .map_err(|_| OUT_OF_MEMORY)?;
        owned_roots.extend_from_slice(roots);
        Ok(Box::new(Self {
            device: d,
            pipeline,
            signature,
            byte_size: abi.byte_size,
            roots: owned_roots,
            stages: STAGE_COMPUTE,
            topology: 0,
            strides: Vec::new(),
        }))
    }
}

/// Build the ROOT_DESCRIPTORS profile root signature: namespace constants at
/// b0, one raw UAV root descriptor per root slot at u(slot), and one unbounded
/// UAV descriptor table at u0/space1 for the descriptor profile.
pub(in crate::foundation) fn root_signature(
    d: &Device,
    abi: &ArgumentInterface,
    roots: &[RootSlot],
) -> Result<d3d::ID3D12RootSignature, Status> {
    let constants = d3d::D3D12_ROOT_CONSTANTS {
        ShaderRegister: 0,
        RegisterSpace: 0,
        Num32BitValues: abi.byte_size / 4,
    };
    let descriptors: Vec<d3d::D3D12_ROOT_DESCRIPTOR1> = (0..abi.root_count)
        .map(|slot| d3d::D3D12_ROOT_DESCRIPTOR1 {
            ShaderRegister: slot,
            RegisterSpace: 0,
            Flags: d3d::D3D12_ROOT_DESCRIPTOR_FLAG_DATA_VOLATILE,
        })
        .collect();
    let mut parameters: Vec<d3d::D3D12_ROOT_PARAMETER1> =
        Vec::with_capacity(descriptors.len() + 2);
    if abi.byte_size != 0 {
        // A zero-length root-constant parameter is invalid; an empty argument
        // namespace omits the parameter entirely (and shifts root indices).
        parameters.push(d3d::D3D12_ROOT_PARAMETER1 {
            ParameterType: d3d::D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
            Anonymous: d3d::D3D12_ROOT_PARAMETER1_0 {
                Constants: constants,
            },
            ShaderVisibility: d3d::D3D12_SHADER_VISIBILITY_ALL,
        });
    }
    for descriptor in &descriptors {
        parameters.push(d3d::D3D12_ROOT_PARAMETER1 {
            ParameterType: d3d::D3D12_ROOT_PARAMETER_TYPE_UAV,
            Anonymous: d3d::D3D12_ROOT_PARAMETER1_0 {
                Descriptor: *descriptor,
            },
            ShaderVisibility: d3d::D3D12_SHADER_VISIBILITY_ALL,
        });
    }
    // Descriptor-profile tables: separate table parameters may share the same
    // heap range start (ranges within one table must not overlap). Buffer
    // access binds the UAV table (u0/space1), image access the SRV table
    // (t0/space1), samplers their own heap (s0/space1).
    let resource_ranges: Vec<d3d::D3D12_DESCRIPTOR_RANGE1> =
        vec![unbounded_range(d3d::D3D12_DESCRIPTOR_RANGE_TYPE_UAV, 0)];
    let image_ranges: Vec<d3d::D3D12_DESCRIPTOR_RANGE1> =
        vec![unbounded_range(d3d::D3D12_DESCRIPTOR_RANGE_TYPE_SRV, 0)];
    let sampler_ranges: Vec<d3d::D3D12_DESCRIPTOR_RANGE1> =
        vec![unbounded_range(d3d::D3D12_DESCRIPTOR_RANGE_TYPE_SAMPLER, 0)];
    for ranges in [&resource_ranges, &image_ranges, &sampler_ranges] {
        parameters.push(d3d::D3D12_ROOT_PARAMETER1 {
            ParameterType: d3d::D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
            Anonymous: d3d::D3D12_ROOT_PARAMETER1_0 {
                DescriptorTable: d3d::D3D12_ROOT_DESCRIPTOR_TABLE1 {
                    NumDescriptorRanges: 1,
                    pDescriptorRanges: ranges.as_ptr(),
                },
            },
            ShaderVisibility: d3d::D3D12_SHADER_VISIBILITY_ALL,
        });
    }
    let signature_desc = d3d::D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
        Version: d3d::D3D_ROOT_SIGNATURE_VERSION_1_1,
        Anonymous: d3d::D3D12_VERSIONED_ROOT_SIGNATURE_DESC_0 {
            Desc_1_1: d3d::D3D12_ROOT_SIGNATURE_DESC1 {
                NumParameters: parameters.len() as u32,
                pParameters: parameters.as_ptr(),
                NumStaticSamplers: 0,
                pStaticSamplers: ptr::null(),
                // The profile's artifacts may declare vertex input; the flag
                // permits input layouts on the signature (required by the
                // native PSO validation when one is bound).
                Flags: d3d::D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
            },
        },
    };
    let mut serialized: Option<windows::Win32::Graphics::Direct3D::ID3DBlob> = None;
    let mut errors: Option<windows::Win32::Graphics::Direct3D::ID3DBlob> = None;
    // SAFETY: self-contained description; out blobs are adopted.
    let serialized_result = unsafe {
        d3d::D3D12SerializeVersionedRootSignature(
            &signature_desc,
            ptr::from_mut(&mut serialized),
            Some(ptr::from_mut(&mut errors)),
        )
    };
    if let Err(error) = serialized_result {
        if let Some(blob) = errors.as_ref() {
            // SAFETY: blob bytes are immutable and length-reported.
            let text = unsafe {
                std::slice::from_raw_parts(
                    blob.GetBufferPointer().cast::<u8>(),
                    blob.GetBufferSize(),
                )
            };
            eprintln!(
                "DX12PROBE root signature error: {}",
                String::from_utf8_lossy(text)
            );
        }
        return Err(map_error(error));
    }
    let serialized = serialized.ok_or(BACKEND_ERROR)?;
    // SAFETY: blob bytes are immutable and length-reported.
    let blob = unsafe {
        std::slice::from_raw_parts(
            serialized.GetBufferPointer().cast::<u8>(),
            serialized.GetBufferSize(),
        )
    };
    // SAFETY: validated serialized signature blob.
    let signature: d3d::ID3D12RootSignature = unsafe {
        d.device.CreateRootSignature(0, blob).map_err(|e| {
            let status = map_error(e);
            eprintln!("DX12PROBE root signature creation failed: {}", status);
            status
        })?
    };
    let _ = roots;
    Ok(signature)
}
