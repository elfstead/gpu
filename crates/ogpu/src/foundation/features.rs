//! Private native enabling masks. Public selection is a counted list of opaque IDs.
//! This representation covers today's implemented catalog and is not an ABI limit.
//! Extending/changing private storage does not change any public record.
use super::types::*;
pub(super) const RASTER: u64 = 1;
pub(super) const FLOAT16: u64 = 2;
pub(super) const UNIFIED_IMAGES: u64 = 4;
pub(super) const SAMPLER_ANISOTROPY: u64 = 8;
pub(super) const CACHE_CONTROL: u64 = 16;
pub(super) const INT8: u64 = 32;
pub(super) const INT16: u64 = 64;
pub(super) const INT64: u64 = 128;
pub(super) const FLOAT64: u64 = 256;
pub(super) const STORAGE8: u64 = 512;
pub(super) const BUFFER_ATOMIC64: u64 = 1024;
pub(super) const SHARED_ATOMIC64: u64 = 2048;
pub(super) const SUBGROUP_SIZE_CONTROL: u64 = 4096;
pub(super) const FULL_SUBGROUPS: u64 = 8192;
pub(super) const SUBGROUP_EXTENDED_TYPES: u64 = 16384;
pub(super) const PIPELINE_STATISTICS: u64 = 32768;
pub(super) const PRECISE_OCCLUSION: u64 = 65536;
pub(super) const INHERITED_QUERIES: u64 = 131072;
pub(super) const VERTEX_STORES_ATOMICS: u64 = 262144;
pub(super) const FRAGMENT_STORES_ATOMICS: u64 = 524288;
pub(super) const INDIRECT_FIRST_INSTANCE: u64 = 1048576;
pub(super) const INDEPENDENT_BLEND: u64 = 2097152;
pub(super) const DEPTH_CLAMP: u64 = 4194304;
pub(super) const DEPTH_BIAS_CLAMP: u64 = 8388608;
pub(super) const NON_SOLID_FILL: u64 = 16777216;
pub(super) const WIDE_LINES: u64 = 33554432;
pub(super) const DEPTH_BOUNDS: u64 = 67108864;
pub(super) const SAMPLE_SHADING: u64 = 134217728;
pub(super) const ALPHA_TO_ONE: u64 = 268435456;
pub(super) const UNUSED_ATTACHMENTS: u64 = 536870912;
pub(super) const LOGIC_OP: u64 = 1073741824;
pub(super) const DUAL_SOURCE_BLEND: u64 = 2147483648;
pub(super) const SHADER_OUTPUT_LAYER: u64 = 4294967296;
pub(super) const BUFFER_FLOAT32_ATOMICS: u64 = 8589934592;
pub(super) const BUFFER_FLOAT32_ATOMIC_ADD: u64 = 17179869184;
pub(super) const BUFFER_FLOAT32_ATOMIC_MIN_MAX: u64 = 34359738368;
pub(super) const SHARED_FLOAT32_ATOMICS: u64 = 68719476736;
pub(super) const SHARED_FLOAT32_ATOMIC_ADD: u64 = 137438953472;
pub(super) const SHARED_FLOAT32_ATOMIC_MIN_MAX: u64 = 274877906944;
pub(super) const BUFFER_FLOAT64_ATOMICS: u64 = 549755813888;
pub(super) const BUFFER_FLOAT64_ATOMIC_ADD: u64 = 1099511627776;
pub(super) const BUFFER_FLOAT64_ATOMIC_MIN_MAX: u64 = 2199023255552;
pub(super) const SHARED_FLOAT64_ATOMICS: u64 = 4398046511104;
pub(super) const SHARED_FLOAT64_ATOMIC_ADD: u64 = 8796093022208;
pub(super) const SHARED_FLOAT64_ATOMIC_MIN_MAX: u64 = 17592186044416;
pub(super) const COOPERATIVE_MATRIX: u64 = 35184372088832;
pub(super) const VULKAN_MEMORY_MODEL: u64 = 70368744177664;
pub(super) const VULKAN_MEMORY_MODEL_DEVICE_SCOPE: u64 = 140737488355328;
pub(super) const INTEGER_DOT: u64 = 281474976710656;
pub(super) const BUFFER_FLOAT16_ATOMICS: u64 = 562949953421312;
pub(super) const BUFFER_FLOAT16_ATOMIC_ADD: u64 = 1125899906842624;
pub(super) const BUFFER_FLOAT16_ATOMIC_MIN_MAX: u64 = 2251799813685248;
pub(super) const SHARED_FLOAT16_ATOMICS: u64 = 4503599627370496;
pub(super) const SHARED_FLOAT16_ATOMIC_ADD: u64 = 9007199254740992;
pub(super) const SHARED_FLOAT16_ATOMIC_MIN_MAX: u64 = 18014398509481984;
pub(super) const VULKAN_MEMORY_MODEL_CHAINS: u64 = 36028797018963968;
pub(super) const CUBE_ARRAY: u64 = 72057594037927936;
pub(super) const PRESENTATION: u64 = 144115188075855872;
pub(super) const MESH_SHADER: u64 = 288230376151711744;
pub(super) const TASK_SHADER: u64 = 576460752303423488;
pub(super) const GENERATED_COMMANDS: u64 = 1152921504606846976;
#[derive(Clone, Copy, Default)]
pub(super) struct FeatureState {
    pub available: u64,
    pub enabled: u64,
    pub max_timeline_difference: u64,
    pub baseline_supported: u32,
    pub device_scope: u32,
}
pub(super) fn encode(ids: &[u32]) -> Result<u64, Status> {
    let mut mask = 0;
    for &id in ids {
        if id == 0 {
            return Err(INVALID);
        }
        if id > 61 {
            return Err(UNSUPPORTED);
        }
        mask |= 1u64 << (id - 1);
    }
    Ok(mask)
}
pub(super) fn decode(mask: u64) -> Vec<u32> {
    (1..=61)
        .filter(|id| mask & (1u64 << (id - 1)) != 0)
        .collect()
}
pub(super) unsafe fn required(features: Features) -> Result<u64, Status> {
    encode(unsafe { features.as_slice()? })
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
        for id in [62, 64, 65, 128, u32::MAX] {
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
}
