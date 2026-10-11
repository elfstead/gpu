//! Native acceleration hints remain distinct from executable feature support.
use super::*;
pub(super) fn snapshot(
    p: &vk::VkPhysicalDeviceShaderIntegerDotProductProperties,
) -> IntegerDotInfo {
    fn mask(values: [u32; 15]) -> u32 {
        values
            .iter()
            .enumerate()
            .fold(0, |bits, (i, v)| bits | if *v != 0 { 1 << i } else { 0 })
    }
    IntegerDotInfo {
        accelerated: mask([
            p.integerDotProduct8BitUnsignedAccelerated,
            p.integerDotProduct8BitSignedAccelerated,
            p.integerDotProduct8BitMixedSignednessAccelerated,
            p.integerDotProduct4x8BitPackedUnsignedAccelerated,
            p.integerDotProduct4x8BitPackedSignedAccelerated,
            p.integerDotProduct4x8BitPackedMixedSignednessAccelerated,
            p.integerDotProduct16BitUnsignedAccelerated,
            p.integerDotProduct16BitSignedAccelerated,
            p.integerDotProduct16BitMixedSignednessAccelerated,
            p.integerDotProduct32BitUnsignedAccelerated,
            p.integerDotProduct32BitSignedAccelerated,
            p.integerDotProduct32BitMixedSignednessAccelerated,
            p.integerDotProduct64BitUnsignedAccelerated,
            p.integerDotProduct64BitSignedAccelerated,
            p.integerDotProduct64BitMixedSignednessAccelerated,
        ]),
        accumulating_saturating_accelerated: mask([
            p.integerDotProductAccumulatingSaturating8BitUnsignedAccelerated,
            p.integerDotProductAccumulatingSaturating8BitSignedAccelerated,
            p.integerDotProductAccumulatingSaturating8BitMixedSignednessAccelerated,
            p.integerDotProductAccumulatingSaturating4x8BitPackedUnsignedAccelerated,
            p.integerDotProductAccumulatingSaturating4x8BitPackedSignedAccelerated,
            p.integerDotProductAccumulatingSaturating4x8BitPackedMixedSignednessAccelerated,
            p.integerDotProductAccumulatingSaturating16BitUnsignedAccelerated,
            p.integerDotProductAccumulatingSaturating16BitSignedAccelerated,
            p.integerDotProductAccumulatingSaturating16BitMixedSignednessAccelerated,
            p.integerDotProductAccumulatingSaturating32BitUnsignedAccelerated,
            p.integerDotProductAccumulatingSaturating32BitSignedAccelerated,
            p.integerDotProductAccumulatingSaturating32BitMixedSignednessAccelerated,
            p.integerDotProductAccumulatingSaturating64BitUnsignedAccelerated,
            p.integerDotProductAccumulatingSaturating64BitSignedAccelerated,
            p.integerDotProductAccumulatingSaturating64BitMixedSignednessAccelerated,
        ]),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integer_dot_acceleration_keeps_all_thirty_properties_independent() {
        for selected in 0..30 {
            let mut p = vk::VkPhysicalDeviceShaderIntegerDotProductProperties::default();
            let fields = [
                &mut p.integerDotProduct8BitUnsignedAccelerated,
                &mut p.integerDotProduct8BitSignedAccelerated,
                &mut p.integerDotProduct8BitMixedSignednessAccelerated,
                &mut p.integerDotProduct4x8BitPackedUnsignedAccelerated,
                &mut p.integerDotProduct4x8BitPackedSignedAccelerated,
                &mut p.integerDotProduct4x8BitPackedMixedSignednessAccelerated,
                &mut p.integerDotProduct16BitUnsignedAccelerated,
                &mut p.integerDotProduct16BitSignedAccelerated,
                &mut p.integerDotProduct16BitMixedSignednessAccelerated,
                &mut p.integerDotProduct32BitUnsignedAccelerated,
                &mut p.integerDotProduct32BitSignedAccelerated,
                &mut p.integerDotProduct32BitMixedSignednessAccelerated,
                &mut p.integerDotProduct64BitUnsignedAccelerated,
                &mut p.integerDotProduct64BitSignedAccelerated,
                &mut p.integerDotProduct64BitMixedSignednessAccelerated,
                &mut p.integerDotProductAccumulatingSaturating8BitUnsignedAccelerated,
                &mut p.integerDotProductAccumulatingSaturating8BitSignedAccelerated,
                &mut p.integerDotProductAccumulatingSaturating8BitMixedSignednessAccelerated,
                &mut p.integerDotProductAccumulatingSaturating4x8BitPackedUnsignedAccelerated,
                &mut p.integerDotProductAccumulatingSaturating4x8BitPackedSignedAccelerated,
                &mut p
                    .integerDotProductAccumulatingSaturating4x8BitPackedMixedSignednessAccelerated,
                &mut p.integerDotProductAccumulatingSaturating16BitUnsignedAccelerated,
                &mut p.integerDotProductAccumulatingSaturating16BitSignedAccelerated,
                &mut p.integerDotProductAccumulatingSaturating16BitMixedSignednessAccelerated,
                &mut p.integerDotProductAccumulatingSaturating32BitUnsignedAccelerated,
                &mut p.integerDotProductAccumulatingSaturating32BitSignedAccelerated,
                &mut p.integerDotProductAccumulatingSaturating32BitMixedSignednessAccelerated,
                &mut p.integerDotProductAccumulatingSaturating64BitUnsignedAccelerated,
                &mut p.integerDotProductAccumulatingSaturating64BitSignedAccelerated,
                &mut p.integerDotProductAccumulatingSaturating64BitMixedSignednessAccelerated,
            ];
            *fields.into_iter().nth(selected).unwrap() = 1;
            let got = snapshot(&p);
            assert_eq!(
                got.accelerated,
                if selected < 15 { 1 << selected } else { 0 }
            );
            assert_eq!(
                got.accumulating_saturating_accelerated,
                if selected >= 15 {
                    1 << (selected - 15)
                } else {
                    0
                }
            );
        }
        assert_eq!(snapshot(&Default::default()), IntegerDotInfo::default());
    }
}
