//! Private native enabling masks. Public selection is a counted list of opaque IDs.
//! This representation covers today's implemented catalog and is not an ABI limit.
//! Extending/changing private storage does not change any public record.
use super::types::*;
pub(super) type FeatureBits = u128;
pub(super) const MULTI_VIEWPORT: FeatureBits = 1 << 109;
pub(super) const SHADER_OUTPUT_VIEWPORT_INDEX: FeatureBits = 1 << 110;
pub(super) const DYNAMIC_DEPTH_CLAMP: FeatureBits = 1 << 98;
pub(super) const DYNAMIC_POLYGON_MODE: FeatureBits = 1 << 99;
pub(super) const DYNAMIC_RASTERIZATION_SAMPLES: FeatureBits = 1 << 100;
pub(super) const DYNAMIC_SAMPLE_MASK: FeatureBits = 1 << 101;
pub(super) const DYNAMIC_ALPHA_TO_COVERAGE: FeatureBits = 1 << 102;
pub(super) const DYNAMIC_ALPHA_TO_ONE: FeatureBits = 1 << 103;
pub(super) const DYNAMIC_LOGIC_OP_ENABLE: FeatureBits = 1 << 104;
pub(super) const DYNAMIC_COLOR_BLEND_ENABLE: FeatureBits = 1 << 105;
pub(super) const DYNAMIC_COLOR_BLEND_EQUATION: FeatureBits = 1 << 106;
pub(super) const DYNAMIC_COLOR_WRITE_MASK: FeatureBits = 1 << 107;
pub(super) const DYNAMIC_LOGIC_OP: FeatureBits = 1 << 108;
pub(super) const DYNAMIC_VERTEX_INPUT: FeatureBits = 1 << 97;
pub(super) const SAMPLE_LOCATIONS: FeatureBits = 1 << 95;
pub(super) const DYNAMIC_SAMPLE_LOCATIONS_ENABLE: FeatureBits = 1 << 96;
pub(super) const CONSERVATIVE_RASTER: FeatureBits = 1 << 79;
pub(super) const PROVOKING_LAST: FeatureBits = 1 << 80;
pub(super) const DEPTH_NEGATIVE_ONE_TO_ONE: FeatureBits = 1 << 81;
pub(super) const RECTANGULAR_LINES: FeatureBits = 1 << 82;
pub(super) const BRESENHAM_LINES: FeatureBits = 1 << 83;
pub(super) const SMOOTH_LINES: FeatureBits = 1 << 84;
pub(super) const STIPPLED_RECTANGULAR_LINES: FeatureBits = 1 << 85;
pub(super) const STIPPLED_BRESENHAM_LINES: FeatureBits = 1 << 86;
pub(super) const STIPPLED_SMOOTH_LINES: FeatureBits = 1 << 87;
pub(super) const DYNAMIC_CONSERVATIVE_MODE: FeatureBits = 1 << 88;
pub(super) const DYNAMIC_EXTRA_OVERESTIMATION: FeatureBits = 1 << 89;
pub(super) const DYNAMIC_PROVOKING_VERTEX: FeatureBits = 1 << 90;
pub(super) const DYNAMIC_DEPTH_CLIP: FeatureBits = 1 << 91;
pub(super) const DYNAMIC_DEPTH_CONVENTION: FeatureBits = 1 << 92;
pub(super) const DYNAMIC_LINE_MODE: FeatureBits = 1 << 93;
pub(super) const DYNAMIC_LINE_STIPPLE_ENABLE: FeatureBits = 1 << 94;

pub(super) const LARGE_POINTS: FeatureBits = 1 << 73;
pub(super) const DEPTH_CLIP: FeatureBits = 1 << 74;
pub(super) const VERTEX_DIVISOR: FeatureBits = 1 << 75;
pub(super) const VERTEX_ZERO_DIVISOR: FeatureBits = 1 << 76;
pub(super) const LIST_RESTART: FeatureBits = 1 << 77;
pub(super) const PATCH_RESTART: FeatureBits = 1 << 78;
pub(super) const TESSELLATION: FeatureBits = 1 << 68;
pub(super) const GEOMETRY: FeatureBits = 1 << 69;
pub(super) const TESSELLATION_MULTIVIEW: FeatureBits = 1 << 70;
pub(super) const GEOMETRY_MULTIVIEW: FeatureBits = 1 << 71;
pub(super) const TESSELLATION_GEOMETRY_POINT_SIZE: FeatureBits = 1 << 72;
pub(super) const LOCAL_READ: FeatureBits = 1 << 67;
pub(super) const PIPELINE_SHADING_RATE: FeatureBits = 1 << 63;
pub(super) const PRIMITIVE_SHADING_RATE: FeatureBits = 1 << 64;
pub(super) const ATTACHMENT_SHADING_RATE: FeatureBits = 1 << 65;
pub(super) const MESH_PRIMITIVE_SHADING_RATE: FeatureBits = 1 << 66;

pub(super) const RASTER: FeatureBits = 1;
pub(super) const FLOAT16: FeatureBits = 2;
pub(super) const UNIFIED_IMAGES: FeatureBits = 4;
pub(super) const SAMPLER_ANISOTROPY: FeatureBits = 8;
pub(super) const CACHE_CONTROL: FeatureBits = 16;
pub(super) const INT8: FeatureBits = 32;
pub(super) const INT16: FeatureBits = 64;
pub(super) const INT64: FeatureBits = 128;
pub(super) const FLOAT64: FeatureBits = 256;
pub(super) const STORAGE8: FeatureBits = 512;
pub(super) const BUFFER_ATOMIC64: FeatureBits = 1024;
pub(super) const SHARED_ATOMIC64: FeatureBits = 2048;
pub(super) const SUBGROUP_SIZE_CONTROL: FeatureBits = 4096;
pub(super) const FULL_SUBGROUPS: FeatureBits = 8192;
pub(super) const SUBGROUP_EXTENDED_TYPES: FeatureBits = 16384;
pub(super) const PIPELINE_STATISTICS: FeatureBits = 32768;
pub(super) const PRECISE_OCCLUSION: FeatureBits = 65536;
pub(super) const INHERITED_QUERIES: FeatureBits = 131072;
pub(super) const VERTEX_STORES_ATOMICS: FeatureBits = 262144;
pub(super) const FRAGMENT_STORES_ATOMICS: FeatureBits = 524288;
pub(super) const INDIRECT_FIRST_INSTANCE: FeatureBits = 1048576;
pub(super) const INDEPENDENT_BLEND: FeatureBits = 2097152;
pub(super) const DEPTH_CLAMP: FeatureBits = 4194304;
pub(super) const DEPTH_BIAS_CLAMP: FeatureBits = 8388608;
pub(super) const NON_SOLID_FILL: FeatureBits = 16777216;
pub(super) const WIDE_LINES: FeatureBits = 33554432;
pub(super) const DEPTH_BOUNDS: FeatureBits = 67108864;
pub(super) const SAMPLE_SHADING: FeatureBits = 134217728;
pub(super) const ALPHA_TO_ONE: FeatureBits = 268435456;
pub(super) const UNUSED_ATTACHMENTS: FeatureBits = 536870912;
pub(super) const LOGIC_OP: FeatureBits = 1073741824;
pub(super) const DUAL_SOURCE_BLEND: FeatureBits = 2147483648;
pub(super) const SHADER_OUTPUT_LAYER: FeatureBits = 4294967296;
pub(super) const BUFFER_FLOAT32_ATOMICS: FeatureBits = 8589934592;
pub(super) const BUFFER_FLOAT32_ATOMIC_ADD: FeatureBits = 17179869184;
pub(super) const BUFFER_FLOAT32_ATOMIC_MIN_MAX: FeatureBits = 34359738368;
pub(super) const SHARED_FLOAT32_ATOMICS: FeatureBits = 68719476736;
pub(super) const SHARED_FLOAT32_ATOMIC_ADD: FeatureBits = 137438953472;
pub(super) const SHARED_FLOAT32_ATOMIC_MIN_MAX: FeatureBits = 274877906944;
pub(super) const BUFFER_FLOAT64_ATOMICS: FeatureBits = 549755813888;
pub(super) const BUFFER_FLOAT64_ATOMIC_ADD: FeatureBits = 1099511627776;
pub(super) const BUFFER_FLOAT64_ATOMIC_MIN_MAX: FeatureBits = 2199023255552;
pub(super) const SHARED_FLOAT64_ATOMICS: FeatureBits = 4398046511104;
pub(super) const SHARED_FLOAT64_ATOMIC_ADD: FeatureBits = 8796093022208;
pub(super) const SHARED_FLOAT64_ATOMIC_MIN_MAX: FeatureBits = 17592186044416;
pub(super) const COOPERATIVE_MATRIX: FeatureBits = 35184372088832;
pub(super) const VULKAN_MEMORY_MODEL: FeatureBits = 70368744177664;
pub(super) const VULKAN_MEMORY_MODEL_DEVICE_SCOPE: FeatureBits = 140737488355328;
pub(super) const INTEGER_DOT: FeatureBits = 281474976710656;
pub(super) const BUFFER_FLOAT16_ATOMICS: FeatureBits = 562949953421312;
pub(super) const BUFFER_FLOAT16_ATOMIC_ADD: FeatureBits = 1125899906842624;
pub(super) const BUFFER_FLOAT16_ATOMIC_MIN_MAX: FeatureBits = 2251799813685248;
pub(super) const SHARED_FLOAT16_ATOMICS: FeatureBits = 4503599627370496;
pub(super) const SHARED_FLOAT16_ATOMIC_ADD: FeatureBits = 9007199254740992;
pub(super) const SHARED_FLOAT16_ATOMIC_MIN_MAX: FeatureBits = 18014398509481984;
pub(super) const VULKAN_MEMORY_MODEL_CHAINS: FeatureBits = 36028797018963968;
pub(super) const CUBE_ARRAY: FeatureBits = 72057594037927936;
pub(super) const PRESENTATION: FeatureBits = 144115188075855872;
pub(super) const MESH_SHADER: FeatureBits = 288230376151711744;
pub(super) const TASK_SHADER: FeatureBits = 576460752303423488;
pub(super) const GENERATED_COMMANDS: FeatureBits = 1152921504606846976;
pub(super) const MULTIVIEW: FeatureBits = 1 << 61;
pub(super) const MESH_MULTIVIEW: FeatureBits = 1 << 62;
#[derive(Clone, Copy, Default)]
pub(super) struct FeatureState {
    pub available: FeatureBits,
    pub enabled: FeatureBits,
    pub max_timeline_difference: u64,
    pub baseline_supported: u32,
    pub device_scope: u32,
}
pub(super) fn encode(ids: &[u32]) -> Result<FeatureBits, Status> {
    let mut mask = 0;
    for &id in ids {
        if id == 0 {
            return Err(INVALID);
        }
        if id > 111 {
            return Err(UNSUPPORTED);
        }
        mask |= 1u128 << (id - 1);
    }
    Ok(mask)
}
pub(super) fn decode(mask: FeatureBits) -> Vec<u32> {
    (1..=111)
        .filter(|id| mask & (1u128 << (id - 1)) != 0)
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
        assert_eq!(encode(&[]), Ok(0));
    }

    #[test]
    fn unknown_and_zero_ids_are_rejected_without_truncation() {
        assert_eq!(encode(&[0]), Err(INVALID));
        for id in [112, 128, u32::MAX] {
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
        assert_eq!(unsafe { required(list) }, Ok(0));
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
        assert_eq!(view_mask(0, &limits, 0), Ok(()));
        assert_eq!(view_mask(0, &limits, 1), Err(UNSUPPORTED));
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
