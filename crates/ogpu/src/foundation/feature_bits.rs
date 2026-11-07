//! Private fixed storage; public capabilities remain counted opaque IDs.
//! Cold catalog selection allocates IDs, while recording tests inline constant bits.
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::foundation) struct FeatureBits([u128; 2]);
impl FeatureBits {
    pub const ZERO: Self = Self([0; 2]);
    pub const MAX: Self = Self([u128::MAX; 2]);
    pub const CAPACITY: u32 = 256;
    #[cfg(test)]
    pub const fn low(value: u128) -> Self {
        Self([value, 0])
    }
    pub const fn bit(index: u32) -> Self {
        assert!(index < Self::CAPACITY);
        let mut words = [0; 2];
        words[(index / 128) as usize] = 1 << (index % 128);
        Self(words)
    }
    pub const fn union(values: &[Self]) -> Self {
        let mut out = Self::ZERO;
        let mut i = 0;
        while i < values.len() {
            out.0[0] |= values[i].0[0];
            out.0[1] |= values[i].0[1];
            i += 1;
        }
        out
    }
}
impl BitAnd for FeatureBits {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self([self.0[0] & rhs.0[0], self.0[1] & rhs.0[1]])
    }
}
impl BitAnd<&Self> for FeatureBits {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: &Self) -> Self {
        self & *rhs
    }
}
impl BitOr for FeatureBits {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self([self.0[0] | rhs.0[0], self.0[1] | rhs.0[1]])
    }
}
impl Not for FeatureBits {
    type Output = Self;
    #[inline]
    fn not(self) -> Self {
        Self([!self.0[0], !self.0[1]])
    }
}
impl BitOrAssign for FeatureBits {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}
impl BitAndAssign for FeatureBits {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        *self = *self & rhs;
    }
}
// Preserve legible zero tests without permitting integer masks to select high bits.
impl PartialEq<u128> for FeatureBits {
    #[inline]
    fn eq(&self, rhs: &u128) -> bool {
        self.0[0] == *rhs && self.0[1] == 0
    }
}
impl std::fmt::LowerHex for FeatureBits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0[1] == 0 {
            write!(f, "{:x}", self.0[0])
        } else {
            write!(f, "{:x}{:032x}", self.0[1], self.0[0])
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn word_boundaries_never_alias_or_disappear() {
        let values = [0, 63, 64, 127, 128, 191, 192, 255].map(FeatureBits::bit);
        let all = FeatureBits::union(&values);
        for (i, &bit) in values.iter().enumerate() {
            assert_eq!(all & bit, bit);
            assert_eq!((all & !bit) & bit, 0);
            for (j, &other) in values.iter().enumerate() {
                assert_eq!(bit & other != 0, i == j);
            }
        }
        assert_ne!(FeatureBits::bit(128), 0);
        assert_eq!(FeatureBits::low(u128::MAX) & FeatureBits::bit(128), 0);
        assert_eq!(FeatureBits::union(&[]), FeatureBits::ZERO);
    }
}
