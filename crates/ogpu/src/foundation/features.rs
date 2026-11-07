//! Private native enabling masks. Public selection is a counted list of opaque IDs.
//! This representation covers today's implemented catalog and is not an ABI limit.
//! Extending/changing private storage does not change any public record.
use super::types::*;
#[path = "feature_bits.rs"]
mod storage;
pub(super) use storage::FeatureBits;
pub(super) const SPARSE_BINDING: FeatureBits = FeatureBits::bit(feature_ids::SPARSE_BINDING - 1);
pub(super) const SPARSE_BUFFER: FeatureBits = FeatureBits::bit(feature_ids::SPARSE_BUFFER - 1);
pub(super) const SPARSE_IMAGE_2D: FeatureBits = FeatureBits::bit(feature_ids::SPARSE_IMAGE_2D - 1);
pub(super) const SPARSE_IMAGE_3D: FeatureBits = FeatureBits::bit(feature_ids::SPARSE_IMAGE_3D - 1);
pub(super) const SPARSE_IMAGE_2_SAMPLES: FeatureBits =
    FeatureBits::bit(feature_ids::SPARSE_IMAGE_2_SAMPLES - 1);
pub(super) const SPARSE_IMAGE_4_SAMPLES: FeatureBits =
    FeatureBits::bit(feature_ids::SPARSE_IMAGE_4_SAMPLES - 1);
pub(super) const SPARSE_IMAGE_8_SAMPLES: FeatureBits =
    FeatureBits::bit(feature_ids::SPARSE_IMAGE_8_SAMPLES - 1);
pub(super) const SPARSE_IMAGE_16_SAMPLES: FeatureBits =
    FeatureBits::bit(feature_ids::SPARSE_IMAGE_16_SAMPLES - 1);
pub(super) const SPARSE_ALIAS: FeatureBits = FeatureBits::bit(feature_ids::SPARSE_ALIAS - 1);
pub(super) const SHADER_SPARSE_RESIDENCY: FeatureBits =
    FeatureBits::bit(feature_ids::SHADER_SPARSE_RESIDENCY - 1);
pub(super) const SHADER_MIN_LOD: FeatureBits = FeatureBits::bit(feature_ids::SHADER_MIN_LOD - 1);
pub(super) const ACCELERATION_STRUCTURE: FeatureBits =
    FeatureBits::bit(feature_ids::ACCELERATION_STRUCTURE - 1);
pub(super) const RAY_QUERY: FeatureBits = FeatureBits::bit(feature_ids::RAY_QUERY - 1);
pub(super) const RAY_PIPELINE: FeatureBits = FeatureBits::bit(feature_ids::RAY_PIPELINE - 1);
pub(super) const RAY_INDIRECT: FeatureBits = FeatureBits::bit(feature_ids::RAY_INDIRECT - 1);
pub(super) const RAY_INDIRECT_TABLE: FeatureBits =
    FeatureBits::bit(feature_ids::RAY_INDIRECT_TABLE - 1);
pub(super) const ACCELERATION_INDIRECT_BUILD: FeatureBits =
    FeatureBits::bit(feature_ids::ACCELERATION_INDIRECT_BUILD - 1);
pub(super) const RAY_MAINTENANCE: FeatureBits = FeatureBits::bit(feature_ids::RAY_MAINTENANCE - 1);
pub(super) const RAY_PRIMITIVE_CULLING: FeatureBits =
    FeatureBits::bit(feature_ids::RAY_PRIMITIVE_CULLING - 1);

pub(super) const TRANSFORM_FEEDBACK: FeatureBits =
    FeatureBits::bit(feature_ids::TRANSFORM_FEEDBACK - 1);
pub(super) const GEOMETRY_STREAMS: FeatureBits =
    FeatureBits::bit(feature_ids::GEOMETRY_STREAMS - 1);
pub(super) const DYNAMIC_RASTERIZATION_STREAM: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_RASTERIZATION_STREAM - 1);
pub(super) const DYNAMIC_PATCH_CONTROL_POINTS: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_PATCH_CONTROL_POINTS - 1);
pub(super) const DYNAMIC_TESSELLATION_ORIGIN: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_TESSELLATION_ORIGIN - 1);
pub(super) const UNRESTRICTED_DEPTH_RANGE: FeatureBits =
    FeatureBits::bit(feature_ids::UNRESTRICTED_DEPTH_RANGE - 1);
pub(super) const DEPTH_CLAMP_CONTROL: FeatureBits =
    FeatureBits::bit(feature_ids::DEPTH_CLAMP_CONTROL - 1);
pub(super) const DEPTH_BIAS_CONTROL: FeatureBits =
    FeatureBits::bit(feature_ids::DEPTH_BIAS_CONTROL - 1);
pub(super) const DEPTH_BIAS_UNORM: FeatureBits =
    FeatureBits::bit(feature_ids::DEPTH_BIAS_UNORM - 1);
pub(super) const DEPTH_BIAS_FLOAT: FeatureBits =
    FeatureBits::bit(feature_ids::DEPTH_BIAS_FLOAT - 1);
pub(super) const DEPTH_BIAS_EXACT: FeatureBits =
    FeatureBits::bit(feature_ids::DEPTH_BIAS_EXACT - 1);
pub(super) const COLOR_WRITE_ENABLE: FeatureBits =
    FeatureBits::bit(feature_ids::COLOR_WRITE_ENABLE - 1);

pub(super) const MULTI_VIEWPORT: FeatureBits = FeatureBits::bit(feature_ids::MULTI_VIEWPORT - 1);
pub(super) const SHADER_OUTPUT_VIEWPORT_INDEX: FeatureBits =
    FeatureBits::bit(feature_ids::SHADER_OUTPUT_VIEWPORT_INDEX - 1);
pub(super) const DYNAMIC_DEPTH_CLAMP: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_DEPTH_CLAMP - 1);
pub(super) const DYNAMIC_POLYGON_MODE: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_POLYGON_MODE - 1);
pub(super) const DYNAMIC_RASTERIZATION_SAMPLES: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_RASTERIZATION_SAMPLES - 1);
pub(super) const DYNAMIC_SAMPLE_MASK: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_SAMPLE_MASK - 1);
pub(super) const DYNAMIC_ALPHA_TO_COVERAGE: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_ALPHA_TO_COVERAGE - 1);
pub(super) const DYNAMIC_ALPHA_TO_ONE: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_ALPHA_TO_ONE - 1);
pub(super) const DYNAMIC_LOGIC_OP_ENABLE: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_LOGIC_OP_ENABLE - 1);
pub(super) const DYNAMIC_COLOR_BLEND_ENABLE: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_COLOR_BLEND_ENABLE - 1);
pub(super) const DYNAMIC_COLOR_BLEND_EQUATION: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_COLOR_BLEND_EQUATION - 1);
pub(super) const DYNAMIC_COLOR_WRITE_MASK: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_COLOR_WRITE_MASK - 1);
pub(super) const DYNAMIC_LOGIC_OP: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_LOGIC_OP - 1);
pub(super) const DYNAMIC_VERTEX_INPUT: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_VERTEX_INPUT - 1);
pub(super) const SAMPLE_LOCATIONS: FeatureBits =
    FeatureBits::bit(feature_ids::SAMPLE_LOCATIONS - 1);
pub(super) const DYNAMIC_SAMPLE_LOCATIONS_ENABLE: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_SAMPLE_LOCATIONS_ENABLE - 1);
pub(super) const CONSERVATIVE_RASTER: FeatureBits =
    FeatureBits::bit(feature_ids::CONSERVATIVE_RASTER - 1);
pub(super) const PROVOKING_LAST: FeatureBits = FeatureBits::bit(feature_ids::PROVOKING_LAST - 1);
pub(super) const DEPTH_NEGATIVE_ONE_TO_ONE: FeatureBits =
    FeatureBits::bit(feature_ids::DEPTH_NEGATIVE_ONE_TO_ONE - 1);
pub(super) const RECTANGULAR_LINES: FeatureBits =
    FeatureBits::bit(feature_ids::RECTANGULAR_LINES - 1);
pub(super) const BRESENHAM_LINES: FeatureBits = FeatureBits::bit(feature_ids::BRESENHAM_LINES - 1);
pub(super) const SMOOTH_LINES: FeatureBits = FeatureBits::bit(feature_ids::SMOOTH_LINES - 1);
pub(super) const STIPPLED_RECTANGULAR_LINES: FeatureBits =
    FeatureBits::bit(feature_ids::STIPPLED_RECTANGULAR_LINES - 1);
pub(super) const STIPPLED_BRESENHAM_LINES: FeatureBits =
    FeatureBits::bit(feature_ids::STIPPLED_BRESENHAM_LINES - 1);
pub(super) const STIPPLED_SMOOTH_LINES: FeatureBits =
    FeatureBits::bit(feature_ids::STIPPLED_SMOOTH_LINES - 1);
pub(super) const DYNAMIC_CONSERVATIVE_MODE: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_CONSERVATIVE_MODE - 1);
pub(super) const DYNAMIC_EXTRA_OVERESTIMATION: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_EXTRA_OVERESTIMATION - 1);
pub(super) const DYNAMIC_PROVOKING_VERTEX: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_PROVOKING_VERTEX - 1);
pub(super) const DYNAMIC_DEPTH_CLIP: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_DEPTH_CLIP - 1);
pub(super) const DYNAMIC_DEPTH_CONVENTION: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_DEPTH_CONVENTION - 1);
pub(super) const DYNAMIC_LINE_MODE: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_LINE_MODE - 1);
pub(super) const DYNAMIC_LINE_STIPPLE_ENABLE: FeatureBits =
    FeatureBits::bit(feature_ids::DYNAMIC_LINE_STIPPLE_ENABLE - 1);

pub(super) const LARGE_POINTS: FeatureBits = FeatureBits::bit(feature_ids::LARGE_POINTS - 1);
pub(super) const DEPTH_CLIP: FeatureBits = FeatureBits::bit(feature_ids::DEPTH_CLIP - 1);
pub(super) const VERTEX_DIVISOR: FeatureBits = FeatureBits::bit(feature_ids::VERTEX_DIVISOR - 1);
pub(super) const VERTEX_ZERO_DIVISOR: FeatureBits =
    FeatureBits::bit(feature_ids::VERTEX_ZERO_DIVISOR - 1);
pub(super) const LIST_RESTART: FeatureBits = FeatureBits::bit(feature_ids::LIST_RESTART - 1);
pub(super) const PATCH_RESTART: FeatureBits = FeatureBits::bit(feature_ids::PATCH_RESTART - 1);
pub(super) const TESSELLATION: FeatureBits = FeatureBits::bit(feature_ids::TESSELLATION - 1);
pub(super) const GEOMETRY: FeatureBits = FeatureBits::bit(feature_ids::GEOMETRY - 1);
pub(super) const TESSELLATION_MULTIVIEW: FeatureBits =
    FeatureBits::bit(feature_ids::TESSELLATION_MULTIVIEW - 1);
pub(super) const GEOMETRY_MULTIVIEW: FeatureBits =
    FeatureBits::bit(feature_ids::GEOMETRY_MULTIVIEW - 1);
pub(super) const TESSELLATION_GEOMETRY_POINT_SIZE: FeatureBits =
    FeatureBits::bit(feature_ids::TESSELLATION_GEOMETRY_POINT_SIZE - 1);
pub(super) const LOCAL_READ: FeatureBits = FeatureBits::bit(feature_ids::LOCAL_READ - 1);
pub(super) const PIPELINE_SHADING_RATE: FeatureBits =
    FeatureBits::bit(feature_ids::PIPELINE_SHADING_RATE - 1);
pub(super) const PRIMITIVE_SHADING_RATE: FeatureBits =
    FeatureBits::bit(feature_ids::PRIMITIVE_SHADING_RATE - 1);
pub(super) const ATTACHMENT_SHADING_RATE: FeatureBits =
    FeatureBits::bit(feature_ids::ATTACHMENT_SHADING_RATE - 1);
pub(super) const MESH_PRIMITIVE_SHADING_RATE: FeatureBits =
    FeatureBits::bit(feature_ids::MESH_PRIMITIVE_SHADING_RATE - 1);

pub(super) const RASTER: FeatureBits = FeatureBits::bit(feature_ids::RASTER - 1);
pub(super) const FLOAT16: FeatureBits = FeatureBits::bit(feature_ids::FLOAT16 - 1);
pub(super) const UNIFIED_IMAGES: FeatureBits = FeatureBits::bit(feature_ids::UNIFIED_IMAGES - 1);
pub(super) const SAMPLER_ANISOTROPY: FeatureBits =
    FeatureBits::bit(feature_ids::SAMPLER_ANISOTROPY - 1);
pub(super) const CACHE_CONTROL: FeatureBits = FeatureBits::bit(feature_ids::CACHE_CONTROL - 1);
pub(super) const INT8: FeatureBits = FeatureBits::bit(feature_ids::INT8 - 1);
pub(super) const INT16: FeatureBits = FeatureBits::bit(feature_ids::INT16 - 1);
pub(super) const INT64: FeatureBits = FeatureBits::bit(feature_ids::INT64 - 1);
pub(super) const FLOAT64: FeatureBits = FeatureBits::bit(feature_ids::FLOAT64 - 1);
pub(super) const STORAGE8: FeatureBits = FeatureBits::bit(feature_ids::STORAGE8 - 1);
pub(super) const BUFFER_ATOMIC64: FeatureBits = FeatureBits::bit(feature_ids::BUFFER_ATOMIC64 - 1);
pub(super) const SHARED_ATOMIC64: FeatureBits = FeatureBits::bit(feature_ids::SHARED_ATOMIC64 - 1);
pub(super) const SUBGROUP_SIZE_CONTROL: FeatureBits =
    FeatureBits::bit(feature_ids::SUBGROUP_SIZE_CONTROL - 1);
pub(super) const FULL_SUBGROUPS: FeatureBits = FeatureBits::bit(feature_ids::FULL_SUBGROUPS - 1);
pub(super) const SUBGROUP_EXTENDED_TYPES: FeatureBits =
    FeatureBits::bit(feature_ids::SUBGROUP_EXTENDED_TYPES - 1);
pub(super) const PIPELINE_STATISTICS: FeatureBits =
    FeatureBits::bit(feature_ids::PIPELINE_STATISTICS - 1);
pub(super) const PRECISE_OCCLUSION: FeatureBits =
    FeatureBits::bit(feature_ids::PRECISE_OCCLUSION - 1);
pub(super) const INHERITED_QUERIES: FeatureBits =
    FeatureBits::bit(feature_ids::INHERITED_QUERIES - 1);
pub(super) const VERTEX_STORES_ATOMICS: FeatureBits =
    FeatureBits::bit(feature_ids::VERTEX_STORES_ATOMICS - 1);
pub(super) const FRAGMENT_STORES_ATOMICS: FeatureBits =
    FeatureBits::bit(feature_ids::FRAGMENT_STORES_ATOMICS - 1);
pub(super) const INDIRECT_FIRST_INSTANCE: FeatureBits =
    FeatureBits::bit(feature_ids::INDIRECT_FIRST_INSTANCE - 1);
pub(super) const INDEPENDENT_BLEND: FeatureBits =
    FeatureBits::bit(feature_ids::INDEPENDENT_BLEND - 1);
pub(super) const DEPTH_CLAMP: FeatureBits = FeatureBits::bit(feature_ids::DEPTH_CLAMP - 1);
pub(super) const DEPTH_BIAS_CLAMP: FeatureBits =
    FeatureBits::bit(feature_ids::DEPTH_BIAS_CLAMP - 1);
pub(super) const NON_SOLID_FILL: FeatureBits = FeatureBits::bit(feature_ids::NON_SOLID_FILL - 1);
pub(super) const WIDE_LINES: FeatureBits = FeatureBits::bit(feature_ids::WIDE_LINES - 1);
pub(super) const DEPTH_BOUNDS: FeatureBits = FeatureBits::bit(feature_ids::DEPTH_BOUNDS - 1);
pub(super) const SAMPLE_SHADING: FeatureBits = FeatureBits::bit(feature_ids::SAMPLE_SHADING - 1);
pub(super) const ALPHA_TO_ONE: FeatureBits = FeatureBits::bit(feature_ids::ALPHA_TO_ONE - 1);
pub(super) const UNUSED_ATTACHMENTS: FeatureBits =
    FeatureBits::bit(feature_ids::UNUSED_ATTACHMENTS - 1);
pub(super) const LOGIC_OP: FeatureBits = FeatureBits::bit(feature_ids::LOGIC_OP - 1);
pub(super) const DUAL_SOURCE_BLEND: FeatureBits =
    FeatureBits::bit(feature_ids::DUAL_SOURCE_BLEND - 1);
pub(super) const SHADER_OUTPUT_LAYER: FeatureBits =
    FeatureBits::bit(feature_ids::SHADER_OUTPUT_LAYER - 1);
pub(super) const BUFFER_FLOAT32_ATOMICS: FeatureBits =
    FeatureBits::bit(feature_ids::BUFFER_FLOAT32_ATOMICS - 1);
pub(super) const BUFFER_FLOAT32_ATOMIC_ADD: FeatureBits =
    FeatureBits::bit(feature_ids::BUFFER_FLOAT32_ATOMIC_ADD - 1);
pub(super) const BUFFER_FLOAT32_ATOMIC_MIN_MAX: FeatureBits =
    FeatureBits::bit(feature_ids::BUFFER_FLOAT32_ATOMIC_MIN_MAX - 1);
pub(super) const SHARED_FLOAT32_ATOMICS: FeatureBits =
    FeatureBits::bit(feature_ids::SHARED_FLOAT32_ATOMICS - 1);
pub(super) const SHARED_FLOAT32_ATOMIC_ADD: FeatureBits =
    FeatureBits::bit(feature_ids::SHARED_FLOAT32_ATOMIC_ADD - 1);
pub(super) const SHARED_FLOAT32_ATOMIC_MIN_MAX: FeatureBits =
    FeatureBits::bit(feature_ids::SHARED_FLOAT32_ATOMIC_MIN_MAX - 1);
pub(super) const BUFFER_FLOAT64_ATOMICS: FeatureBits =
    FeatureBits::bit(feature_ids::BUFFER_FLOAT64_ATOMICS - 1);
pub(super) const BUFFER_FLOAT64_ATOMIC_ADD: FeatureBits =
    FeatureBits::bit(feature_ids::BUFFER_FLOAT64_ATOMIC_ADD - 1);
pub(super) const BUFFER_FLOAT64_ATOMIC_MIN_MAX: FeatureBits =
    FeatureBits::bit(feature_ids::BUFFER_FLOAT64_ATOMIC_MIN_MAX - 1);
pub(super) const SHARED_FLOAT64_ATOMICS: FeatureBits =
    FeatureBits::bit(feature_ids::SHARED_FLOAT64_ATOMICS - 1);
pub(super) const SHARED_FLOAT64_ATOMIC_ADD: FeatureBits =
    FeatureBits::bit(feature_ids::SHARED_FLOAT64_ATOMIC_ADD - 1);
pub(super) const SHARED_FLOAT64_ATOMIC_MIN_MAX: FeatureBits =
    FeatureBits::bit(feature_ids::SHARED_FLOAT64_ATOMIC_MIN_MAX - 1);
pub(super) const COOPERATIVE_MATRIX: FeatureBits =
    FeatureBits::bit(feature_ids::COOPERATIVE_MATRIX - 1);
pub(super) const VULKAN_MEMORY_MODEL: FeatureBits =
    FeatureBits::bit(feature_ids::VULKAN_MEMORY_MODEL - 1);
pub(super) const VULKAN_MEMORY_MODEL_DEVICE_SCOPE: FeatureBits =
    FeatureBits::bit(feature_ids::VULKAN_MEMORY_MODEL_DEVICE_SCOPE - 1);
pub(super) const INTEGER_DOT: FeatureBits = FeatureBits::bit(feature_ids::INTEGER_DOT - 1);
pub(super) const BUFFER_FLOAT16_ATOMICS: FeatureBits =
    FeatureBits::bit(feature_ids::BUFFER_FLOAT16_ATOMICS - 1);
pub(super) const BUFFER_FLOAT16_ATOMIC_ADD: FeatureBits =
    FeatureBits::bit(feature_ids::BUFFER_FLOAT16_ATOMIC_ADD - 1);
pub(super) const BUFFER_FLOAT16_ATOMIC_MIN_MAX: FeatureBits =
    FeatureBits::bit(feature_ids::BUFFER_FLOAT16_ATOMIC_MIN_MAX - 1);
pub(super) const SHARED_FLOAT16_ATOMICS: FeatureBits =
    FeatureBits::bit(feature_ids::SHARED_FLOAT16_ATOMICS - 1);
pub(super) const SHARED_FLOAT16_ATOMIC_ADD: FeatureBits =
    FeatureBits::bit(feature_ids::SHARED_FLOAT16_ATOMIC_ADD - 1);
pub(super) const SHARED_FLOAT16_ATOMIC_MIN_MAX: FeatureBits =
    FeatureBits::bit(feature_ids::SHARED_FLOAT16_ATOMIC_MIN_MAX - 1);
pub(super) const VULKAN_MEMORY_MODEL_CHAINS: FeatureBits =
    FeatureBits::bit(feature_ids::VULKAN_MEMORY_MODEL_CHAINS - 1);
pub(super) const CUBE_ARRAY: FeatureBits = FeatureBits::bit(feature_ids::CUBE_ARRAY - 1);
pub(super) const PRESENTATION: FeatureBits = FeatureBits::bit(feature_ids::PRESENTATION - 1);
pub(super) const MESH_SHADER: FeatureBits = FeatureBits::bit(feature_ids::MESH_SHADER - 1);
pub(super) const TASK_SHADER: FeatureBits = FeatureBits::bit(feature_ids::TASK_SHADER - 1);
pub(super) const GENERATED_COMMANDS: FeatureBits =
    FeatureBits::bit(feature_ids::GENERATED_COMMANDS - 1);
pub(super) const MULTIVIEW: FeatureBits = FeatureBits::bit(feature_ids::MULTIVIEW - 1);
pub(super) const MESH_MULTIVIEW: FeatureBits = FeatureBits::bit(feature_ids::MESH_MULTIVIEW - 1);
#[derive(Clone, Copy, Default)]
pub(super) struct FeatureState {
    pub available: FeatureBits,
    pub enabled: FeatureBits,
    pub max_timeline_difference: u64,
    pub baseline_supported: u32,
    pub device_scope: u32,
}
const MAX_ID: u32 = feature_ids::RAY_CAPTURE_REPLAY_MIXED;
pub(super) const RAY_LIBRARIES: FeatureBits = FeatureBits::bit(feature_ids::RAY_LIBRARIES - 1);
pub(super) const RAY_LIBRARY_HANDLES: FeatureBits =
    FeatureBits::bit(feature_ids::RAY_LIBRARY_HANDLES - 1);
pub(super) const RAY_CAPTURE_REPLAY: FeatureBits =
    FeatureBits::bit(feature_ids::RAY_CAPTURE_REPLAY - 1);
pub(super) const RAY_CAPTURE_REPLAY_MIXED: FeatureBits =
    FeatureBits::bit(feature_ids::RAY_CAPTURE_REPLAY_MIXED - 1);

pub(super) const PIPELINE_BINARIES: FeatureBits =
    FeatureBits::bit(feature_ids::PIPELINE_BINARIES - 1);
pub(super) const DISABLE_BINARY_INTERNAL_CACHE: FeatureBits =
    FeatureBits::bit(feature_ids::DISABLE_BINARY_INTERNAL_CACHE - 1);
pub(super) const MATRIX_WORKGROUP: FeatureBits =
    FeatureBits::bit(feature_ids::MATRIX_WORKGROUP - 1);
pub(super) const MATRIX_FLEXIBLE: FeatureBits = FeatureBits::bit(feature_ids::MATRIX_FLEXIBLE - 1);
pub(super) const MATRIX_REDUCTIONS: FeatureBits =
    FeatureBits::bit(feature_ids::MATRIX_REDUCTIONS - 1);
pub(super) const MATRIX_CONVERSIONS: FeatureBits =
    FeatureBits::bit(feature_ids::MATRIX_CONVERSIONS - 1);
pub(super) const MATRIX_PER_ELEMENT: FeatureBits =
    FeatureBits::bit(feature_ids::MATRIX_PER_ELEMENT - 1);
pub(super) const MATRIX_TENSOR_ADDRESSING: FeatureBits =
    FeatureBits::bit(feature_ids::MATRIX_TENSOR_ADDRESSING - 1);
pub(super) const MATRIX_BLOCK_LOADS: FeatureBits =
    FeatureBits::bit(feature_ids::MATRIX_BLOCK_LOADS - 1);
pub(super) const BFLOAT16_TYPE: FeatureBits = FeatureBits::bit(feature_ids::BFLOAT16_TYPE - 1);
pub(super) const BFLOAT16_DOT: FeatureBits = FeatureBits::bit(feature_ids::BFLOAT16_DOT - 1);
pub(super) const BFLOAT16_MATRIX: FeatureBits = FeatureBits::bit(feature_ids::BFLOAT16_MATRIX - 1);
pub(super) const FLOAT8_TYPE: FeatureBits = FeatureBits::bit(feature_ids::FLOAT8_TYPE - 1);
pub(super) const FLOAT8_MATRIX: FeatureBits = FeatureBits::bit(feature_ids::FLOAT8_MATRIX - 1);
pub(super) const MATRIX_ROBUST: FeatureBits = FeatureBits::bit(feature_ids::MATRIX_ROBUST - 1);

const _: () = assert!(MAX_ID <= FeatureBits::CAPACITY);
pub(super) fn encode(ids: &[u32]) -> Result<FeatureBits, Status> {
    let mut mask = FeatureBits::ZERO;
    for &id in ids {
        if id == 0 {
            return Err(INVALID);
        }
        if id > MAX_ID {
            return Err(UNSUPPORTED);
        }
        mask |= FeatureBits::bit(id - 1);
    }
    Ok(mask)
}
pub(super) fn decode(mask: FeatureBits) -> Vec<u32> {
    (1..=MAX_ID)
        .filter(|id| mask & (FeatureBits::bit(id - 1)) != 0)
        .collect()
}
pub(super) unsafe fn required(features: Features) -> Result<FeatureBits, Status> {
    encode(unsafe { features.as_slice()? })
}

/// A view mask selects indices, not a count. Sparse high bits need matching layers.
pub(super) fn view_mask(
    enabled: FeatureBits,
    limits: &GraphicsLimits,
    mask: u32,
) -> Result<(), Status> {
    if mask != 0
        && (enabled & MULTIVIEW == 0 || 32 - mask.leading_zeros() > limits.max_multiview_views)
    {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
pub(super) fn render_layers(layers: u32, mask: u32) -> u32 {
    if mask == 0 {
        layers
    } else {
        32 - mask.leading_zeros()
    }
}
pub(super) fn instance_range(
    mask: u32,
    maximum: u32,
    first: u32,
    count: u32,
) -> Result<(), Status> {
    if mask != 0 && count != 0 && u64::from(first) + u64::from(count) - 1 > u64::from(maximum) {
        return Err(INVALID);
    }
    Ok(())
}

pub(super) const CONDITIONAL_RENDERING: FeatureBits =
    FeatureBits::bit(feature_ids::CONDITIONAL_RENDERING - 1);

pub(super) const INHERITED_CONDITIONAL_RENDERING: FeatureBits =
    FeatureBits::bit(feature_ids::INHERITED_CONDITIONAL_RENDERING - 1);

pub(super) const PRIMITIVES_GENERATED_QUERY: FeatureBits =
    FeatureBits::bit(feature_ids::PRIMITIVES_GENERATED_QUERY - 1);

pub(super) const PRIMITIVES_GENERATED_DISCARD: FeatureBits =
    FeatureBits::bit(feature_ids::PRIMITIVES_GENERATED_DISCARD - 1);

pub(super) const PRIMITIVES_GENERATED_STREAMS: FeatureBits =
    FeatureBits::bit(feature_ids::PRIMITIVES_GENERATED_STREAMS - 1);

pub(super) const UNIFORM_STORAGE8: FeatureBits =
    FeatureBits::bit(feature_ids::UNIFORM_STORAGE8 - 1);
pub(super) const PUSH_STORAGE8: FeatureBits = FeatureBits::bit(feature_ids::PUSH_STORAGE8 - 1);
pub(super) const SCALAR_BLOCK_LAYOUT: FeatureBits =
    FeatureBits::bit(feature_ids::SCALAR_BLOCK_LAYOUT - 1);
pub(super) const UNIFORM_STANDARD_LAYOUT: FeatureBits =
    FeatureBits::bit(feature_ids::UNIFORM_STANDARD_LAYOUT - 1);
pub(super) const SUBGROUP_BROADCAST_DYNAMIC: FeatureBits =
    FeatureBits::bit(feature_ids::SUBGROUP_BROADCAST_DYNAMIC - 1);
pub(super) const SHADER_TERMINATE: FeatureBits =
    FeatureBits::bit(feature_ids::SHADER_TERMINATE - 1);
pub(super) const SHADER_DEMOTE: FeatureBits = FeatureBits::bit(feature_ids::SHADER_DEMOTE - 1);
pub(super) const SUBGROUP_ROTATE: FeatureBits = FeatureBits::bit(feature_ids::SUBGROUP_ROTATE - 1);
pub(super) const SUBGROUP_ROTATE_CLUSTERED: FeatureBits =
    FeatureBits::bit(feature_ids::SUBGROUP_ROTATE_CLUSTERED - 1);
pub(super) const SHADER_EXPECT_ASSUME: FeatureBits =
    FeatureBits::bit(feature_ids::SHADER_EXPECT_ASSUME - 1);
pub(super) const SHADER_FLOAT_CONTROLS2: FeatureBits =
    FeatureBits::bit(feature_ids::SHADER_FLOAT_CONTROLS2 - 1);
pub(super) const STORAGE_IMAGE_EXTENDED_FORMATS: FeatureBits =
    FeatureBits::bit(feature_ids::STORAGE_IMAGE_EXTENDED_FORMATS - 1);
pub(super) const STORAGE_IMAGE_READ_WITHOUT_FORMAT: FeatureBits =
    FeatureBits::bit(feature_ids::STORAGE_IMAGE_READ_WITHOUT_FORMAT - 1);
pub(super) const STORAGE_IMAGE_WRITE_WITHOUT_FORMAT: FeatureBits =
    FeatureBits::bit(feature_ids::STORAGE_IMAGE_WRITE_WITHOUT_FORMAT - 1);
pub(super) const STORAGE_IMAGE_MULTISAMPLE: FeatureBits =
    FeatureBits::bit(feature_ids::STORAGE_IMAGE_MULTISAMPLE - 1);
pub(super) const IMAGE_GATHER_EXTENDED: FeatureBits =
    FeatureBits::bit(feature_ids::IMAGE_GATHER_EXTENDED - 1);
pub(super) const SHADER_CLIP_DISTANCE: FeatureBits =
    FeatureBits::bit(feature_ids::SHADER_CLIP_DISTANCE - 1);
pub(super) const SHADER_CULL_DISTANCE: FeatureBits =
    FeatureBits::bit(feature_ids::SHADER_CULL_DISTANCE - 1);
pub(super) const UNIFORM_STORAGE16: FeatureBits =
    FeatureBits::bit(feature_ids::UNIFORM_STORAGE16 - 1);
pub(super) const PUSH_STORAGE16: FeatureBits = FeatureBits::bit(feature_ids::PUSH_STORAGE16 - 1);
pub(super) const INPUT_OUTPUT16: FeatureBits = FeatureBits::bit(feature_ids::INPUT_OUTPUT16 - 1);
pub(super) const VARIABLE_POINTERS_STORAGE: FeatureBits =
    FeatureBits::bit(feature_ids::VARIABLE_POINTERS_STORAGE - 1);
pub(super) const VARIABLE_POINTERS: FeatureBits =
    FeatureBits::bit(feature_ids::VARIABLE_POINTERS - 1);
pub(super) const SUBGROUP_UNIFORM_CONTROL_FLOW: FeatureBits =
    FeatureBits::bit(feature_ids::SUBGROUP_UNIFORM_CONTROL_FLOW - 1);
pub(super) const MAXIMAL_RECONVERGENCE: FeatureBits =
    FeatureBits::bit(feature_ids::MAXIMAL_RECONVERGENCE - 1);
pub(super) const QUAD_CONTROL: FeatureBits = FeatureBits::bit(feature_ids::QUAD_CONTROL - 1);
pub(super) const WORKGROUP_EXPLICIT_LAYOUT: FeatureBits =
    FeatureBits::bit(feature_ids::WORKGROUP_EXPLICIT_LAYOUT - 1);
pub(super) const WORKGROUP_SCALAR_LAYOUT: FeatureBits =
    FeatureBits::bit(feature_ids::WORKGROUP_SCALAR_LAYOUT - 1);
pub(super) const WORKGROUP_STORAGE8: FeatureBits =
    FeatureBits::bit(feature_ids::WORKGROUP_STORAGE8 - 1);
pub(super) const WORKGROUP_STORAGE16: FeatureBits =
    FeatureBits::bit(feature_ids::WORKGROUP_STORAGE16 - 1);
pub(super) const COMPUTE_DERIVATIVE_QUADS: FeatureBits =
    FeatureBits::bit(feature_ids::COMPUTE_DERIVATIVE_QUADS - 1);
pub(super) const COMPUTE_DERIVATIVE_LINEAR: FeatureBits =
    FeatureBits::bit(feature_ids::COMPUTE_DERIVATIVE_LINEAR - 1);
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_are_order_independent_and_duplicates_do_not_enable_neighbors() {
        let ids = [
            feature_ids::GENERATED_COMMANDS,
            feature_ids::RASTER,
            feature_ids::GENERATED_COMMANDS,
        ];
        assert_eq!(encode(&ids), Ok(GENERATED_COMMANDS | RASTER));
        assert_eq!(
            decode(encode(&ids).unwrap()),
            vec![feature_ids::RASTER, feature_ids::GENERATED_COMMANDS]
        );
        // The public count is a list length, not the size of the native mask.
        assert_eq!(encode(&[feature_ids::RASTER; 129]), Ok(RASTER));
        assert_eq!(encode(&[]), Ok(FeatureBits::ZERO));
    }

    #[test]
    fn unknown_and_zero_ids_are_rejected_without_truncation() {
        assert_eq!(encode(&[0]), Err(INVALID));
        for id in [MAX_ID + 1, 256, u32::MAX] {
            assert_eq!(encode(&[feature_ids::RASTER, id]), Err(UNSUPPORTED));
        }
    }

    #[test]
    fn malformed_storage_is_rejected_before_dereferencing() {
        // Empty input must ignore the pointer, even when non-null.
        let mut list = Features {
            ids: std::ptr::dangling(),
            ..Default::default()
        };
        assert_eq!(unsafe { required(list) }, Ok(FeatureBits::ZERO));
        list.reserved = 1;
        assert_eq!(unsafe { required(list) }, Err(INVALID));
        list.reserved = 0;
        list.count = 1;
        list.ids = std::ptr::null();
        assert_eq!(unsafe { required(list) }, Err(INVALID));
    }

    #[test]
    fn multiview_uses_highest_index_not_population() {
        let limits = GraphicsLimits {
            max_multiview_views: 4,
            ..Default::default()
        };
        assert_eq!(view_mask(FeatureBits::ZERO, &limits, 0), Ok(()));
        assert_eq!(view_mask(FeatureBits::ZERO, &limits, 1), Err(UNSUPPORTED));
        assert_eq!(view_mask(MULTIVIEW, &limits, 0b1010), Ok(()));
        assert_eq!(view_mask(MULTIVIEW, &limits, 0b10000), Err(UNSUPPORTED));
        assert_eq!(render_layers(9, 0), 9);
        assert_eq!(render_layers(0, 0b1010), 4);
        assert_eq!(render_layers(1, 1 << 31), 32);
        let full = GraphicsLimits {
            max_multiview_views: 32,
            ..Default::default()
        };
        assert_eq!(view_mask(MULTIVIEW, &full, 1 << 31), Ok(()));
    }

    #[test]
    fn multiview_instance_range_is_wide_and_zero_work_is_valid() {
        assert_eq!(instance_range(1, 15, 12, 4), Ok(()));
        assert_eq!(instance_range(1, 15, 12, 5), Err(INVALID));
        assert_eq!(instance_range(1, u32::MAX, u32::MAX, 2), Err(INVALID));
        assert_eq!(instance_range(1, 15, u32::MAX, 0), Ok(()));
        assert_eq!(instance_range(0, 0, u32::MAX, 2), Ok(()));
    }

    #[test]
    fn mesh_multiview_requires_explicit_native_dependencies() {
        assert_eq!(validate_feature_dependencies(MULTIVIEW), Ok(()));
        for bits in [
            MESH_MULTIVIEW,
            MESH_MULTIVIEW | MULTIVIEW,
            MESH_MULTIVIEW | MESH_SHADER | RASTER,
        ] {
            assert_eq!(validate_feature_dependencies(bits), Err(INVALID));
        }
        let bits = MESH_MULTIVIEW | MULTIVIEW | MESH_SHADER | RASTER;
        assert_eq!(validate_feature_dependencies(bits), Ok(()));
        assert_eq!(encode(&decode(bits)), Ok(bits));
    }
}
