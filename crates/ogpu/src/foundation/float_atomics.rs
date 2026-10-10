//! Exact operation/domain enabling. No shader lowering or numerical policy.
use super::*;

pub(super) const MIN_MAX: u64 = BUFFER_FLOAT32_ATOMIC_MIN_MAX
    | SHARED_FLOAT32_ATOMIC_MIN_MAX
    | BUFFER_FLOAT64_ATOMIC_MIN_MAX
    | SHARED_FLOAT64_ATOMIC_MIN_MAX;
pub(super) const ALL: u64 = BUFFER_FLOAT32_ATOMICS
    | BUFFER_FLOAT32_ATOMIC_ADD
    | SHARED_FLOAT32_ATOMICS
    | SHARED_FLOAT32_ATOMIC_ADD
    | BUFFER_FLOAT64_ATOMICS
    | BUFFER_FLOAT64_ATOMIC_ADD
    | SHARED_FLOAT64_ATOMICS
    | SHARED_FLOAT64_ATOMIC_ADD
    | MIN_MAX;

pub(super) fn available(
    a: &vk::VkPhysicalDeviceShaderAtomicFloatFeaturesEXT,
    b: &vk::VkPhysicalDeviceShaderAtomicFloat2FeaturesEXT,
) -> u64 {
    [
        (a.shaderBufferFloat32Atomics, BUFFER_FLOAT32_ATOMICS),
        (a.shaderBufferFloat64Atomics, BUFFER_FLOAT64_ATOMICS),
        (a.shaderBufferFloat64AtomicAdd, BUFFER_FLOAT64_ATOMIC_ADD),
        (
            b.shaderBufferFloat64AtomicMinMax,
            BUFFER_FLOAT64_ATOMIC_MIN_MAX,
        ),
        (a.shaderSharedFloat64Atomics, SHARED_FLOAT64_ATOMICS),
        (a.shaderSharedFloat64AtomicAdd, SHARED_FLOAT64_ATOMIC_ADD),
        (
            b.shaderSharedFloat64AtomicMinMax,
            SHARED_FLOAT64_ATOMIC_MIN_MAX,
        ),
        (a.shaderBufferFloat32AtomicAdd, BUFFER_FLOAT32_ATOMIC_ADD),
        (
            b.shaderBufferFloat32AtomicMinMax,
            BUFFER_FLOAT32_ATOMIC_MIN_MAX,
        ),
        (a.shaderSharedFloat32Atomics, SHARED_FLOAT32_ATOMICS),
        (a.shaderSharedFloat32AtomicAdd, SHARED_FLOAT32_ATOMIC_ADD),
        (
            b.shaderSharedFloat32AtomicMinMax,
            SHARED_FLOAT32_ATOMIC_MIN_MAX,
        ),
    ]
    .into_iter()
    .filter(|(supported, _)| *supported != 0)
    .fold(0, |mask, (_, bit)| mask | bit)
}

pub(super) fn requested(
    enabled: u64,
) -> (
    vk::VkPhysicalDeviceShaderAtomicFloatFeaturesEXT,
    vk::VkPhysicalDeviceShaderAtomicFloat2FeaturesEXT,
) {
    (vk::VkPhysicalDeviceShaderAtomicFloatFeaturesEXT {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SHADER_ATOMIC_FLOAT_FEATURES_EXT,
        shaderBufferFloat32Atomics: u32::from(enabled & BUFFER_FLOAT32_ATOMICS != 0),
        shaderBufferFloat64Atomics: u32::from(enabled & BUFFER_FLOAT64_ATOMICS != 0),
        shaderBufferFloat64AtomicAdd: u32::from(enabled & BUFFER_FLOAT64_ATOMIC_ADD != 0),
        shaderSharedFloat64Atomics: u32::from(enabled & SHARED_FLOAT64_ATOMICS != 0),
        shaderSharedFloat64AtomicAdd: u32::from(enabled & SHARED_FLOAT64_ATOMIC_ADD != 0),
        shaderBufferFloat32AtomicAdd: u32::from(enabled & BUFFER_FLOAT32_ATOMIC_ADD != 0),
        shaderSharedFloat32Atomics: u32::from(enabled & SHARED_FLOAT32_ATOMICS != 0),
        shaderSharedFloat32AtomicAdd: u32::from(enabled & SHARED_FLOAT32_ATOMIC_ADD != 0),
        ..Default::default()
    }, vk::VkPhysicalDeviceShaderAtomicFloat2FeaturesEXT {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SHADER_ATOMIC_FLOAT_2_FEATURES_EXT,
        shaderBufferFloat32AtomicMinMax: u32::from(enabled & BUFFER_FLOAT32_ATOMIC_MIN_MAX != 0),
        shaderBufferFloat64AtomicMinMax: u32::from(enabled & BUFFER_FLOAT64_ATOMIC_MIN_MAX != 0),
        shaderSharedFloat64AtomicMinMax: u32::from(enabled & SHARED_FLOAT64_ATOMIC_MIN_MAX != 0),
        shaderSharedFloat32AtomicMinMax: u32::from(enabled & SHARED_FLOAT32_ATOMIC_MIN_MAX != 0),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn enabling_keeps_float_atomic_domains_and_operations_independent() {
        for selection in 0u64..4096 {
            let enabled = selection << 33;
            let (a, b) = requested(enabled | INT64 | FLOAT64);
            assert_eq!(available(&a, &b), enabled);
            assert_eq!(
                (
                    a.shaderImageFloat32Atomics,
                    a.shaderImageFloat32AtomicAdd,
                    a.sparseImageFloat32Atomics,
                    a.sparseImageFloat32AtomicAdd
                ),
                (0, 0, 0, 0)
            );
            assert_eq!(
                (
                    b.shaderBufferFloat16Atomics,
                    b.shaderBufferFloat16AtomicAdd,
                    b.shaderBufferFloat16AtomicMinMax,
                    b.shaderSharedFloat16Atomics,
                    b.shaderSharedFloat16AtomicAdd,
                    b.shaderSharedFloat16AtomicMinMax,
                    b.shaderImageFloat32AtomicMinMax,
                    b.sparseImageFloat32AtomicMinMax
                ),
                (0, 0, 0, 0, 0, 0, 0, 0)
            );
        }
    }
}
