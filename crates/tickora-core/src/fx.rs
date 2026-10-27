use crate::hash::StateHasher;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

pub const FRAC_BITS: u32 = 32;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fx(i64);

impl Fx {
    pub const ZERO: Fx = Fx(0);
    pub const ONE: Fx = Fx(1 << FRAC_BITS);
    pub const MAX: Fx = Fx(i64::MAX);
    pub const MIN: Fx = Fx(i64::MIN);
    pub const EPSILON: Fx = Fx(1);

    pub const fn from_raw(raw: i64) -> Fx {
        Fx(raw)
    }

    pub const fn raw(self) -> i64 {
        self.0
    }

    pub const fn from_int(value: i32) -> Fx {
        Fx((value as i64) << FRAC_BITS)
    }

    pub fn from_ratio(numerator: i64, denominator: i64) -> Fx {
        divide_raw(i128::from(numerator) << FRAC_BITS, i128::from(denominator))
    }

    pub const fn floor_to_int(self) -> i64 {
        self.0 >> FRAC_BITS
    }

    pub fn to_f32(self) -> f32 {
        self.0 as f32 / (1u64 << FRAC_BITS) as f32
    }

    pub fn to_f64(self) -> f64 {
        self.0 as f64 / (1u64 << FRAC_BITS) as f64
    }

    pub const fn saturating_add(self, other: Fx) -> Fx {
        Fx(self.0.saturating_add(other.0))
    }

    pub const fn saturating_sub(self, other: Fx) -> Fx {
        Fx(self.0.saturating_sub(other.0))
    }

    pub fn saturating_mul(self, other: Fx) -> Fx {
        let product = i128::from(self.0) * i128::from(other.0);
        Fx(clamp_i128(product >> FRAC_BITS))
    }

    pub fn saturating_div(self, other: Fx) -> Fx {
        divide_raw(i128::from(self.0) << FRAC_BITS, i128::from(other.0))
    }

    pub const fn abs(self) -> Fx {
        Fx(self.0.saturating_abs())
    }

    pub fn sqrt(self) -> Fx {
        if self.0 <= 0 {
            return Fx::ZERO;
        }
        let scaled = (self.0 as u128) << FRAC_BITS;
        Fx(isqrt(scaled) as i64)
    }

    pub fn hash_into(self, hasher: &mut StateHasher) {
        hasher.write_i64(self.0);
    }
}

fn clamp_i128(value: i128) -> i64 {
    value.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn divide_raw(numerator: i128, denominator: i128) -> Fx {
    if denominator == 0 {
        return match numerator.signum() {
            1 => Fx::MAX,
            -1 => Fx::MIN,
            _ => Fx::ZERO,
        };
    }
    Fx(clamp_i128(numerator / denominator))
}

fn isqrt(value: u128) -> u128 {
    let mut remainder = value;
    let mut root = 0u128;
    let mut bit = 1u128 << 126;
    while bit > value {
        bit >>= 2;
    }
    while bit != 0 {
        if remainder >= root + bit {
            remainder -= root + bit;
            root = (root >> 1) + bit;
        } else {
            root >>= 1;
        }
        bit >>= 2;
    }
    root
}

impl Add for Fx {
    type Output = Fx;
    fn add(self, other: Fx) -> Fx {
        self.saturating_add(other)
    }
}

impl Sub for Fx {
    type Output = Fx;
    fn sub(self, other: Fx) -> Fx {
        self.saturating_sub(other)
    }
}

impl Mul for Fx {
    type Output = Fx;
    fn mul(self, other: Fx) -> Fx {
        self.saturating_mul(other)
    }
}

impl Div for Fx {
    type Output = Fx;
    fn div(self, other: Fx) -> Fx {
        self.saturating_div(other)
    }
}

impl Neg for Fx {
    type Output = Fx;
    fn neg(self) -> Fx {
        Fx(self.0.saturating_neg())
    }
}

impl AddAssign for Fx {
    fn add_assign(&mut self, other: Fx) {
        *self = *self + other;
    }
}

impl SubAssign for Fx {
    fn sub_assign(&mut self, other: Fx) {
        *self = *self - other;
    }
}

impl MulAssign for Fx {
    fn mul_assign(&mut self, other: Fx) {
        *self = *self * other;
    }
}

impl DivAssign for Fx {
    fn div_assign(&mut self, other: Fx) {
        *self = *self / other;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fx(value: i32) -> Fx {
        Fx::from_int(value)
    }

    #[test]
    fn one_is_two_to_the_thirty_two() {
        assert_eq!(Fx::ONE.raw(), 1i64 << 32);
        assert_eq!(fx(3).floor_to_int(), 3);
        assert_eq!(fx(-3).floor_to_int(), -3);
    }

    #[test]
    fn integer_arithmetic_is_exact() {
        assert_eq!(fx(2) + fx(3), fx(5));
        assert_eq!(fx(2) - fx(5), fx(-3));
        assert_eq!(fx(6) * fx(7), fx(42));
        assert_eq!(fx(42) / fx(6), fx(7));
        assert_eq!(-fx(4), fx(-4));
    }

    #[test]
    fn fractions_are_exact_in_binary() {
        let half = Fx::ONE / fx(2);
        assert_eq!(half.raw(), 1i64 << 31);
        assert_eq!(half * half, Fx::ONE / fx(4));
        assert_eq!(half + half, Fx::ONE);
    }

    #[test]
    fn multiply_rounds_toward_negative_infinity() {
        let tiny = Fx::EPSILON;
        assert_eq!((tiny * tiny).raw(), 0);
        assert_eq!((-tiny * tiny).raw(), -1);
    }

    #[test]
    fn divide_truncates_toward_zero() {
        assert_eq!((fx(1) / fx(3)).raw(), 0x5555_5555);
        assert_eq!((fx(-1) / fx(3)).raw(), -0x5555_5555);
    }

    #[test]
    fn arithmetic_saturates_instead_of_wrapping() {
        assert_eq!(Fx::MAX + Fx::ONE, Fx::MAX);
        assert_eq!(Fx::MIN - Fx::ONE, Fx::MIN);
        assert_eq!(Fx::MAX * fx(2), Fx::MAX);
        assert_eq!(Fx::MIN * fx(2), Fx::MIN);
        assert_eq!(-Fx::MIN, Fx::MAX);
        assert_eq!(Fx::MIN.abs(), Fx::MAX);
    }

    #[test]
    fn divide_by_zero_saturates_by_sign() {
        assert_eq!(fx(5) / Fx::ZERO, Fx::MAX);
        assert_eq!(fx(-5) / Fx::ZERO, Fx::MIN);
        assert_eq!(Fx::ZERO / Fx::ZERO, Fx::ZERO);
    }

    #[test]
    fn sqrt_of_perfect_squares_is_exact() {
        assert_eq!(fx(0).sqrt(), fx(0));
        assert_eq!(fx(1).sqrt(), fx(1));
        assert_eq!(fx(4).sqrt(), fx(2));
        assert_eq!(fx(144).sqrt(), fx(12));
        assert_eq!((Fx::ONE / fx(4)).sqrt(), Fx::ONE / fx(2));
    }

    #[test]
    fn sqrt_floors_and_handles_edges() {
        let two = fx(2).sqrt();
        assert_eq!(two.raw(), 6_074_000_999);
        let root = i128::from(two.raw());
        let scaled = i128::from(fx(2).raw()) << FRAC_BITS;
        assert!(root * root <= scaled);
        assert!((root + 1) * (root + 1) > scaled);
        assert_eq!(fx(-4).sqrt(), Fx::ZERO);
        assert!(Fx::MAX.sqrt() > Fx::ZERO);
    }

    #[test]
    fn from_ratio_matches_division() {
        assert_eq!(Fx::from_ratio(1, 3), fx(1) / fx(3));
        assert_eq!(Fx::from_ratio(-7, 2).raw(), -(7i64 << 31));
        assert_eq!(Fx::from_ratio(1, 0), Fx::MAX);
    }

    #[test]
    fn sqrt_is_the_floor_root_for_many_values() {
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        for _ in 0..2000 {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let value = Fx::from_raw((state >> 1) as i64);
            let root = value.sqrt();
            let upper = root.raw() as i128 + 1;
            let scaled = (value.raw() as i128) << FRAC_BITS;
            assert!((root.raw() as i128) * (root.raw() as i128) <= scaled);
            assert!(upper * upper > scaled);
        }
    }

    #[test]
    fn ordering_follows_raw_value() {
        assert!(fx(-1) < fx(0));
        assert!(fx(2) > fx(1));
        assert_eq!(fx(3).min(fx(2)), fx(2));
    }

    #[test]
    fn assign_operators_match_binary_operators() {
        let mut value = fx(10);
        value += fx(5);
        value -= fx(3);
        value *= fx(2);
        value /= fx(4);
        assert_eq!(value, fx(6));
    }

    #[test]
    fn hash_into_writes_the_raw_value_little_endian() {
        let mut direct = StateHasher::new();
        direct.write_i64(fx(7).raw());
        let mut through = StateHasher::new();
        fx(7).hash_into(&mut through);
        assert_eq!(direct.finish(), through.finish());
    }

    #[test]
    fn float_conversion_is_for_derived_output() {
        assert_eq!(fx(3).to_f64(), 3.0);
        assert_eq!((Fx::ONE / fx(2)).to_f32(), 0.5);
    }
}
