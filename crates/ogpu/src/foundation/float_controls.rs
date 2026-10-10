//! Physical numerical capabilities, not a selected arithmetic policy.
use super::*;
pub(super) fn float_snapshot(p: &vk::VkPhysicalDeviceVulkan12Properties) -> FloatControls {
    let flags = |values: [u32; 5]| {
        values.iter().enumerate().fold(0, |mask, (i, value)| {
            mask | if *value != 0 { 1 << i } else { 0 }
        })
    };
    FloatControls {
        binary16: flags([
            p.shaderSignedZeroInfNanPreserveFloat16,
            p.shaderDenormPreserveFloat16,
            p.shaderDenormFlushToZeroFloat16,
            p.shaderRoundingModeRTEFloat16,
            p.shaderRoundingModeRTZFloat16,
        ]),
        binary32: flags([
            p.shaderSignedZeroInfNanPreserveFloat32,
            p.shaderDenormPreserveFloat32,
            p.shaderDenormFlushToZeroFloat32,
            p.shaderRoundingModeRTEFloat32,
            p.shaderRoundingModeRTZFloat32,
        ]),
        binary64: flags([
            p.shaderSignedZeroInfNanPreserveFloat64,
            p.shaderDenormPreserveFloat64,
            p.shaderDenormFlushToZeroFloat64,
            p.shaderRoundingModeRTEFloat64,
            p.shaderRoundingModeRTZFloat64,
        ]),
        denorm_independence: p.denormBehaviorIndependence,
        rounding_independence: p.roundingModeIndependence,
        reserved: 0,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn float_properties_keep_widths_policies_and_independence_distinct() {
        for selected in 0..15 {
            let mut p = vk::VkPhysicalDeviceVulkan12Properties {
                denormBehaviorIndependence: 2,
                roundingModeIndependence: 1,
                ..Default::default()
            };
            let fields = [
                &mut p.shaderSignedZeroInfNanPreserveFloat16,
                &mut p.shaderDenormPreserveFloat16,
                &mut p.shaderDenormFlushToZeroFloat16,
                &mut p.shaderRoundingModeRTEFloat16,
                &mut p.shaderRoundingModeRTZFloat16,
                &mut p.shaderSignedZeroInfNanPreserveFloat32,
                &mut p.shaderDenormPreserveFloat32,
                &mut p.shaderDenormFlushToZeroFloat32,
                &mut p.shaderRoundingModeRTEFloat32,
                &mut p.shaderRoundingModeRTZFloat32,
                &mut p.shaderSignedZeroInfNanPreserveFloat64,
                &mut p.shaderDenormPreserveFloat64,
                &mut p.shaderDenormFlushToZeroFloat64,
                &mut p.shaderRoundingModeRTEFloat64,
                &mut p.shaderRoundingModeRTZFloat64,
            ];
            *fields.into_iter().nth(selected).unwrap() = 1;
            let result = float_snapshot(&p);
            let mut expected = [0; 3];
            expected[selected / 5] = 1 << (selected % 5);
            assert_eq!(
                [result.binary16, result.binary32, result.binary64],
                expected
            );
            assert_eq!(
                (
                    result.denorm_independence,
                    result.rounding_independence,
                    result.reserved
                ),
                (2, 1, 0)
            );
        }
        assert_eq!(
            float_snapshot(&vk::VkPhysicalDeviceVulkan12Properties::default()),
            FloatControls::default()
        );
    }
}
