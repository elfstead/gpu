//! D3D12 graphics pipeline preparation: PSO construction from the shared
//! GRAPHICS_STATE/VERTEX_INPUT records. No recording-time compilation; every
//! state is validated and translated at preparation.
use super::*;
use std::ffi::CStr;
use windows::core::BOOL;
use windows::Win32::Graphics::Dxgi::Common as dxgic;

/// Vertex-attribute artifact ABI: input location N binds as semantic
/// `TEXCOORD<N>` (SemanticName "TEXCOORD", SemanticIndex = location).
fn semantic(location: u32) -> (windows::core::PCSTR, u32) {
    (windows::core::s!("TEXCOORD"), location)
}

/// Prepare graphics state from a DXIL vertex artifact with optional fragment
/// artifact and explicit static state.
/// # Safety
/// Live device and trusted description arrays.
pub(super) unsafe fn prepare(
    d: &Device,
    desc: &ExecutableDesc,
) -> Result<Box<Executable>, Status> {
    if desc.compile_flags != 0 || desc.reserved != 0 {
        return Err(UNSUPPORTED)
    }
    if desc.shader_count == 0 || desc.shader_count > 2 {
        return Err(UNSUPPORTED)
    }
    // SAFETY: trusted shader array of the declared count.
    let shaders = unsafe { super::executables::array(desc.shaders, desc.shader_count)? };
    let mut vertex: Option<&Shader> = None;
    let mut fragment: Option<&Shader> = None;
    for shader in shaders {
        if shader.format != SHADER_DXIL {
            // SPIR-V artifacts carry the pointer-in-bytes ABI (no DXIL form).
        return Err(UNSUPPORTED)
        }
        // DXIL containers are byte-granular (their header declares the size);
        // the word-granular size rule describes SPIR-V artifacts. The pointer
        // stays word-aligned for the native artifact readers.
        if shader.code.data.is_null()
            || shader.code.data as usize % 4 != 0
            || shader.code.size < 20
            || shader.code.size > isize::MAX as usize
            || shader.entry.is_null()
        {
        return Err(INVALID)
        }
        // SAFETY: trusted NUL-terminated entry name.
        let entry = unsafe { CStr::from_ptr(shader.entry) };
        if entry.to_bytes().is_empty() || entry.to_str().is_err() {
        return Err(INVALID)
        }
        // SAFETY: code has readable magic by the checks above.
        if unsafe { shader.code.data.cast::<u32>().read() } != u32::from_le_bytes(*b"DXBC") {
        return Err(INVALID)
        }
        match shader.stage {
            x if x == STAGE_VERTEX as u32 => {
                if vertex.is_some() {
        return Err(UNSUPPORTED)
                }
                vertex = Some(shader);
            }
            x if x == STAGE_FRAGMENT as u32 => {
                if fragment.is_some() {
        return Err(UNSUPPORTED)
                }
                fragment = Some(shader);
            }
            _ => return Err(UNSUPPORTED),
        }
    }
    let vertex = vertex.ok_or(UNSUPPORTED)?;
    if desc.dynamic_state & !DYNAMIC_D3D12_NATIVE != 0 {
        // Cull/front face/depth/stencil state is PSO state on this backend;
        // those dynamic strategies are rejected, never silently baked.
        return Err(UNSUPPORTED)
    }
    if desc.static_state.is_null() {
        return Err(UNSUPPORTED)
    }
    // SAFETY: complete GRAPHICS_STATE record behind the pointer.
    let state = unsafe {
        executable_record::<GraphicsState>(desc.static_state, GRAPHICS_STATE)?
    };
    if state.logic_op_enable != 0 {
        // Logic operations need the newer output-merger profile.
        return Err(UNSUPPORTED)
    }
    if !(POINTS..=TRIANGLE_STRIP).contains(&state.topology) {
        return Err(INVALID)
    }
    if state.cull > CULL_BACK + 1 || state.front_face > FRONT_CW {
        return Err(INVALID)
    }
    if state.color_count > 8 {
        return Err(UNSUPPORTED)
    }
    // SAFETY: trusted color-state array of the declared count.
    let colors = unsafe { super::executables::array(state.colors, state.color_count)? };
    let mut vertex_input: Option<&VertexInput> = None;
    if !state.vertex_input.is_null() {
        // SAFETY: complete VERTEX_INPUT record behind the pointer.
        vertex_input = Some(unsafe {
            executable_record::<VertexInput>(state.vertex_input, VERTEX_INPUT)?
        });
    }
    // SAFETY: complete SHADER_REQUIREMENTS record behind the pointer.
    let req = unsafe {
        executable_record::<ShaderRequirements>(desc.requirements, SHADER_REQUIREMENTS)?
    };
    if req.features & !d.snapshot.features.enabled != 0 {
        return Err(UNSUPPORTED)
    }
    let abi =
        // SAFETY: complete ARGUMENT_INTERFACE record behind the pointer.
        unsafe {
            executable_record::<ArgumentInterface>(vertex.interface_metadata, ARGUMENT_INTERFACE)?
        };
    // SAFETY: trusted root array of the declared count.
    let roots = unsafe { super::executables::array(abi.roots, abi.root_count)? };
    executables::interface(&d.snapshot.execution_limits, abi, roots)?;
    if roots.iter().any(|r| {
        r.stages & !(STAGE_VERTEX | STAGE_FRAGMENT | MESH_STAGE | TASK_STAGE) != 0 || r.stages == 0
    }) {
        return Err(UNSUPPORTED)
    }
    if !vertex.specialization.is_null() || !vertex.subgroup.is_null() {
        return Err(UNSUPPORTED)
    }
    if let Some(fragment) = fragment {
        if !fragment.specialization.is_null() || !fragment.subgroup.is_null() {
        return Err(UNSUPPORTED)
        }
    }

    // ---- Static state translation ----
    let topology_type = match state.topology {
        POINTS => d3d::D3D12_PRIMITIVE_TOPOLOGY_TYPE_POINT,
        LINES | LINE_STRIP => d3d::D3D12_PRIMITIVE_TOPOLOGY_TYPE_LINE,
        _ => d3d::D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
    };
    let fill_mode = if state.raster_options.is_null() {
        d3d::D3D12_FILL_MODE_SOLID
    } else {
        // SAFETY: complete RASTER_OPTIONS record behind the pointer.
        let raster = unsafe {
            executable_record::<RasterOptions>(state.raster_options, RASTER_OPTIONS)?
        };
        if raster.sample_shading != 0 || raster.alpha_to_one != 0 {
        return Err(UNSUPPORTED)
        }
        if raster.line_width != 1.0 {
            // D3D12 line width is fixed at 1.0.
        return Err(UNSUPPORTED)
        }
        match raster.polygon_mode {
            POLYGON_FILL => d3d::D3D12_FILL_MODE_SOLID,
            POLYGON_LINE => d3d::D3D12_FILL_MODE_WIREFRAME,
            _ => return Err(UNSUPPORTED),
        }
    };
    let rasterizer = d3d::D3D12_RASTERIZER_DESC {
        FillMode: fill_mode,
        CullMode: match state.cull {
            CULL_NONE => d3d::D3D12_CULL_MODE_NONE,
            CULL_FRONT => d3d::D3D12_CULL_MODE_FRONT,
            CULL_BACK => d3d::D3D12_CULL_MODE_BACK,
            _ => return Err(UNSUPPORTED),
        },
        FrontCounterClockwise: BOOL::from(state.front_face == FRONT_CCW),
        DepthBias: 0,
        DepthBiasClamp: 0.0,
        SlopeScaledDepthBias: 0.0,
        DepthClipEnable: BOOL::from(true),
        MultisampleEnable: BOOL::from(false),
        AntialiasedLineEnable: BOOL::from(false),
        ForcedSampleCount: 0,
        ConservativeRaster: d3d::D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF,
    };

    let mut blend = d3d::D3D12_BLEND_DESC {
        AlphaToCoverageEnable: BOOL::from(false),
        IndependentBlendEnable: BOOL::from(state.color_count > 1),
        RenderTarget: [d3d::D3D12_RENDER_TARGET_BLEND_DESC::default(); 8],
    };
    for (index, color) in colors.iter().enumerate() {
        let factor = blend_factor;
        blend.RenderTarget[index] = d3d::D3D12_RENDER_TARGET_BLEND_DESC {
            BlendEnable: BOOL::from(color.blend != 0),
            LogicOpEnable: BOOL::from(false),
            SrcBlend: factor(color.src_color)?,
            DestBlend: factor(color.dst_color)?,
            BlendOp: blend_op(color.color_op)?,
            SrcBlendAlpha: factor(color.src_alpha)?,
            DestBlendAlpha: factor(color.dst_alpha)?,
            BlendOpAlpha: blend_op(color.alpha_op)?,
            LogicOp: d3d::D3D12_LOGIC_OP_NOOP,
            RenderTargetWriteMask: color.write_mask as u8,
        };
    }

    let (depth_enable, depth_write, depth_func, dsv_format) = if state.depth_format == 0 {
        (false, false, d3d::D3D12_COMPARISON_FUNC_ALWAYS, 0)
    } else {
        (
            state.depth_test != 0,
            state.depth_write != 0,
            compare(state.depth_compare)?,
            state.depth_format,
        )
    };
    let depth_stencil = d3d::D3D12_DEPTH_STENCIL_DESC {
        DepthEnable: BOOL::from(depth_enable),
        DepthWriteMask: if depth_write {
            d3d::D3D12_DEPTH_WRITE_MASK_ALL
        } else {
            d3d::D3D12_DEPTH_WRITE_MASK_ZERO
        },
        DepthFunc: depth_func,
        StencilEnable: BOOL::from(state.stencil_test != 0),
        StencilReadMask: state.stencil_front.compare_mask as u8,
        StencilWriteMask: state.stencil_front.write_mask as u8,
        FrontFace: stencil_ops(&state.stencil_front)?,
        BackFace: stencil_ops(&state.stencil_back)?,
    };

    let mut elements: Vec<d3d::D3D12_INPUT_ELEMENT_DESC> = Vec::new();
    if let Some(input) = vertex_input {
        if input.binding_count > 32 {
        return Err(UNSUPPORTED)
        }
        // SAFETY: trusted binding and attribute arrays.
        let bindings = unsafe { super::executables::array(input.bindings, input.binding_count)? };
        let attributes =
            // SAFETY: trusted attribute array of the declared count.
            unsafe { super::executables::array(input.attributes, input.attribute_count)? };
        elements
            .try_reserve_exact(attributes.len())
            .map_err(|_| OUT_OF_MEMORY)?;
        for attribute in attributes {
            if attribute.binding >= input.binding_count {
        return Err(INVALID)
            }
            let (name, index) = semantic(attribute.location);
            elements.push(d3d::D3D12_INPUT_ELEMENT_DESC {
                SemanticName: name,
                SemanticIndex: index,
                Format: formats::dxgi_format(attribute.format),
                InputSlot: attribute.binding,
                AlignedByteOffset: attribute.offset,
                InputSlotClass: if bindings[attribute.binding as usize].rate == 0 {
                    d3d::D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA
                } else {
                    d3d::D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA
                },
                InstanceDataStepRate: u32::from(bindings[attribute.binding as usize].rate != 0),
            });
        }
    }

    let mut pipeline_desc = d3d::D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: core::mem::ManuallyDrop::new(None),
        VS: shader_bytecode(vertex),
        PS: fragment.map_or_else(
            || d3d::D3D12_SHADER_BYTECODE {
                pShaderBytecode: ptr::null(),
                BytecodeLength: 0,
            },
            shader_bytecode,
        ),
        DS: d3d::D3D12_SHADER_BYTECODE::default(),
        HS: d3d::D3D12_SHADER_BYTECODE::default(),
        GS: d3d::D3D12_SHADER_BYTECODE::default(),
        StreamOutput: d3d::D3D12_STREAM_OUTPUT_DESC::default(),
        BlendState: blend,
        SampleMask: u32::MAX,
        RasterizerState: rasterizer,
        DepthStencilState: depth_stencil,
        InputLayout: d3d::D3D12_INPUT_LAYOUT_DESC {
            pInputElementDescs: if elements.is_empty() {
                ptr::null()
            } else {
                elements.as_ptr()
            },
            NumElements: elements.len() as u32,
        },
        IBStripCutValue: d3d::D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED,
        PrimitiveTopologyType: topology_type,
        NumRenderTargets: state.color_count,
        RTVFormats: {
            let mut formats = [dxgic::DXGI_FORMAT_UNKNOWN; 8];
            for (slot, color) in formats.iter_mut().zip(colors) {
                *slot = formats::dxgi_format(color.format);
            }
            formats
        },
        DSVFormat: formats::dxgi_format(dsv_format),
        SampleDesc: dxgic::DXGI_SAMPLE_DESC {
            Count: state.samples.max(1),
            Quality: 0,
        },
        NodeMask: 0,
        CachedPSO: d3d::D3D12_CACHED_PIPELINE_STATE {
            pCachedBlob: ptr::null(),
            CachedBlobSizeInBytes: 0,
        },
        Flags: d3d::D3D12_PIPELINE_STATE_FLAG_NONE,
    };
    let _ = &mut pipeline_desc;
    let signature = super::executables::root_signature(d, abi, roots)?;
    pipeline_desc.pRootSignature = core::mem::ManuallyDrop::new(Some(unsafe {
        d3d::ID3D12RootSignature::from_raw(signature.as_raw())
    }));
    // SAFETY: prepared signature, artifacts and layout stay valid for the call.
    let pipeline: d3d::ID3D12PipelineState = unsafe {
        d.device
            .CreateGraphicsPipelineState(&pipeline_desc)
            .map_err(|e| {
                let status = map_error(e);
                eprintln!("DX12PROBE PSO creation failed: {}", status);
                status
            })?
    };
    let mut owned_roots = Vec::new();
    owned_roots
        .try_reserve_exact(roots.len())
        .map_err(|_| OUT_OF_MEMORY)?;
    owned_roots.extend_from_slice(roots);
    Ok(Box::new(Executable {
        device: d,
        pipeline,
        signature,
        byte_size: abi.byte_size,
        roots: owned_roots,
        stages: STAGE_VERTEX
            | if fragment.is_some() {
                STAGE_FRAGMENT
            } else {
                0
            },
        topology: state.topology,
        strides: {
            let mut strides = Vec::new();
            if let Some(input) = vertex_input {
                // SAFETY: trusted binding array of the declared count.
                let bindings =
                    unsafe { super::executables::array(input.bindings, input.binding_count)? };
                strides.extend(bindings.iter().map(|b| b.stride));
            }
            strides
        },
    }))
}

fn shader_bytecode(shader: &Shader) -> d3d::D3D12_SHADER_BYTECODE {
    d3d::D3D12_SHADER_BYTECODE {
        pShaderBytecode: shader.code.data,
        BytecodeLength: shader.code.size,
    }
}

fn blend_factor(value: u32) -> Result<d3d::D3D12_BLEND, Status> {
    Ok(match value {
        BLEND_ZERO => d3d::D3D12_BLEND_ZERO,
        BLEND_ONE => d3d::D3D12_BLEND_ONE,
        BLEND_SRC_COLOR => d3d::D3D12_BLEND_SRC_COLOR,
        BLEND_ONE_MINUS_SRC_COLOR => d3d::D3D12_BLEND_INV_SRC_COLOR,
        BLEND_DST_COLOR => d3d::D3D12_BLEND_DEST_COLOR,
        BLEND_ONE_MINUS_DST_COLOR => d3d::D3D12_BLEND_INV_DEST_COLOR,
        BLEND_SRC_ALPHA => d3d::D3D12_BLEND_SRC_ALPHA,
        BLEND_ONE_MINUS_SRC_ALPHA => d3d::D3D12_BLEND_INV_SRC_ALPHA,
        BLEND_DST_ALPHA => d3d::D3D12_BLEND_DEST_ALPHA,
        BLEND_ONE_MINUS_DST_ALPHA => d3d::D3D12_BLEND_INV_DEST_ALPHA,
        BLEND_CONSTANT_COLOR => d3d::D3D12_BLEND_BLEND_FACTOR,
        BLEND_ONE_MINUS_CONSTANT_COLOR => d3d::D3D12_BLEND_INV_BLEND_FACTOR,
        BLEND_CONSTANT_ALPHA => d3d::D3D12_BLEND_BLEND_FACTOR,
        BLEND_ONE_MINUS_CONSTANT_ALPHA => d3d::D3D12_BLEND_INV_BLEND_FACTOR,
        BLEND_SRC_ALPHA_SATURATE => d3d::D3D12_BLEND_SRC_ALPHA_SAT,
        // Dual-source factors need the dual-source blend profile.
        15..=18 => return Err(UNSUPPORTED),
        _ => return Err(INVALID),
    })
}

fn blend_op(value: u32) -> Result<d3d::D3D12_BLEND_OP, Status> {
    Ok(match value {
        BLEND_OP_ADD => d3d::D3D12_BLEND_OP_ADD,
        BLEND_OP_SUBTRACT => d3d::D3D12_BLEND_OP_SUBTRACT,
        BLEND_OP_REVERSE_SUBTRACT => d3d::D3D12_BLEND_OP_REV_SUBTRACT,
        BLEND_OP_MIN => d3d::D3D12_BLEND_OP_MIN,
        BLEND_OP_MAX => d3d::D3D12_BLEND_OP_MAX,
        _ => return Err(INVALID),
    })
}

pub(super) fn compare(value: u32) -> Result<d3d::D3D12_COMPARISON_FUNC, Status> {
    Ok(match value {
        0 => d3d::D3D12_COMPARISON_FUNC_NEVER,
        1 => d3d::D3D12_COMPARISON_FUNC_LESS,
        2 => d3d::D3D12_COMPARISON_FUNC_EQUAL,
        3 => d3d::D3D12_COMPARISON_FUNC_LESS_EQUAL,
        4 => d3d::D3D12_COMPARISON_FUNC_GREATER,
        5 => d3d::D3D12_COMPARISON_FUNC_NOT_EQUAL,
        6 => d3d::D3D12_COMPARISON_FUNC_GREATER_EQUAL,
        7 => d3d::D3D12_COMPARISON_FUNC_ALWAYS,
        _ => return Err(INVALID),
    })
}

fn stencil_op(value: u32) -> Result<d3d::D3D12_STENCIL_OP, Status> {
    Ok(match value {
        STENCIL_KEEP => d3d::D3D12_STENCIL_OP_KEEP,
        STENCIL_ZERO => d3d::D3D12_STENCIL_OP_ZERO,
        STENCIL_REPLACE => d3d::D3D12_STENCIL_OP_REPLACE,
        STENCIL_INCREMENT_CLAMP => d3d::D3D12_STENCIL_OP_INCR_SAT,
        STENCIL_DECREMENT_CLAMP => d3d::D3D12_STENCIL_OP_DECR_SAT,
        STENCIL_INVERT => d3d::D3D12_STENCIL_OP_INVERT,
        STENCIL_INCREMENT_WRAP => d3d::D3D12_STENCIL_OP_INCR,
        STENCIL_DECREMENT_WRAP => d3d::D3D12_STENCIL_OP_DECR,
        _ => return Err(INVALID),
    })
}

fn stencil_ops(state: &StencilState) -> Result<d3d::D3D12_DEPTH_STENCILOP_DESC, Status> {
    Ok(d3d::D3D12_DEPTH_STENCILOP_DESC {
        StencilFailOp: stencil_op(state.fail)?,
        StencilDepthFailOp: stencil_op(state.depth_fail)?,
        StencilPassOp: stencil_op(state.pass)?,
        StencilFunc: compare(state.compare)?,
    })
}
