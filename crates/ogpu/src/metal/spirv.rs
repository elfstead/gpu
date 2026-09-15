//! Optional input adapter; native Metal preparation does not depend on SPIR-V.
use super::fail;
use crate::{Error, OgpuSpecializationConstant, INVALID_ARGUMENT, UNSUPPORTED};
use ::metal::MTLDataType;
use spirv_cross2::{
    compile::{msl::MslVersion, CompilableTarget},
    spirv,
    targets::Msl,
    Compiler, Module,
};

pub(super) struct Translated {
    pub source: String,
    pub local: [u32; 3],
    pub constants: Vec<(u32, u32, MTLDataType)>,
}

pub(super) fn translate(
    words: &[u32],
    constants: &[OgpuSpecializationConstant],
) -> Result<Translated, Error> {
    let module = Module::from_words(words);
    let mut compiler = Compiler::<Msl>::new(module)
        .map_err(|e| fail(INVALID_ARGUMENT, format!("SPIR-V parse failed: {e:?}")))?;
    let has_entry = compiler
        .entry_points()
        .map_err(|e| {
            fail(
                INVALID_ARGUMENT,
                format!("SPIR-V entry query failed: {e:?}"),
            )
        })?
        .any(|e| e.name == "main" && e.execution_model == spirv::ExecutionModel::GLCompute);
    if !has_entry {
        return Err(fail(
            INVALID_ARGUMENT,
            "SPIR-V compute entry point main not found",
        ));
    }
    compiler
        .set_entry_point("main", spirv::ExecutionModel::GLCompute)
        .map_err(|e| {
            fail(
                INVALID_ARGUMENT,
                format!("Entry point selection failed: {e:?}"),
            )
        })?;
    let declared: Vec<_> = compiler
        .specialization_constants()
        .map_err(|e| {
            fail(
                INVALID_ARGUMENT,
                format!("Specialization query failed: {e:?}"),
            )
        })?
        .collect();
    for found in &declared {
        compiler
            .set_name(found.id, format!("ogpu_spec_{}", found.constant_id))
            .map_err(|e| {
                fail(
                    INVALID_ARGUMENT,
                    format!("Specialization rename failed: {e:?}"),
                )
            })?;
    }
    let mut metal_constants = Vec::new();
    for c in constants {
        if let Some(found) = declared.iter().find(|v| v.constant_id == c.id) {
            let type_id = compiler
                .specialization_constant_type(found.id)
                .map_err(|e| {
                    fail(
                        INVALID_ARGUMENT,
                        format!("Specialization type query failed: {e:?}"),
                    )
                })?;
            let ty = compiler.type_description(type_id).map_err(|e| {
                fail(
                    INVALID_ARGUMENT,
                    format!("Specialization type query failed: {e:?}"),
                )
            })?;
            let data_type = match ty.inner {
                spirv_cross2::reflect::TypeInner::Scalar(scalar)
                    if scalar.size == spirv_cross2::reflect::BitWidth::Word =>
                {
                    match scalar.kind {
                        spirv_cross2::reflect::ScalarKind::Int => MTLDataType::Int,
                        spirv_cross2::reflect::ScalarKind::Uint => MTLDataType::UInt,
                        spirv_cross2::reflect::ScalarKind::Float => MTLDataType::Float,
                        spirv_cross2::reflect::ScalarKind::Bool => MTLDataType::Bool,
                    }
                }
                spirv_cross2::reflect::TypeInner::Scalar(scalar)
                    if scalar.kind == spirv_cross2::reflect::ScalarKind::Bool =>
                {
                    MTLDataType::Bool
                }
                _ => {
                    return Err(fail(
                        INVALID_ARGUMENT,
                        "Specialization constants must be 32-bit scalars or bool",
                    ))
                }
            };
            compiler
                .set_specialization_constant_value(found.id, c.bits)
                .map_err(|e| fail(INVALID_ARGUMENT, format!("Specialization failed: {e:?}")))?;
            metal_constants.push((c.id, c.bits, data_type));
        }
    }
    let local = match compiler
        .execution_mode_arguments(spirv::ExecutionMode::LocalSize)
        .map_err(|e| {
            fail(
                INVALID_ARGUMENT,
                format!("Workgroup size query failed: {e:?}"),
            )
        })? {
        Some(spirv_cross2::reflect::ExecutionModeArguments::LocalSize { x, y, z }) => [x, y, z],
        _ => return Err(fail(INVALID_ARGUMENT, "Invalid workgroup size")),
    };
    let mut options = Msl::options();
    options.version = MslVersion::new(3, 0, 0);
    let artifact = compiler
        .compile(&options)
        .map_err(|e| fail(UNSUPPORTED, format!("SPIR-V to MSL failed: {e:?}")))?;
    let source = artifact.to_string();

    Ok(Translated {
        source,
        local,
        constants: metal_constants,
    })
}
