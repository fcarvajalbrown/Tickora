use std::sync::OnceLock;

pub const TABLE_BITS: u32 = 12;
pub const TABLE_SIZE: usize = 1 << TABLE_BITS;
pub const QUARTER: usize = TABLE_SIZE / 4;
pub const TABLE_ONE: i32 = 1 << 30;
pub const QUARTER_TURN: u32 = 1 << 30;

const HALF_PI: f64 = core::f64::consts::FRAC_PI_2;
const TAYLOR_TERMS: u32 = 14;

pub fn sine_table() -> &'static [i32; TABLE_SIZE] {
    static TABLE: OnceLock<[i32; TABLE_SIZE]> = OnceLock::new();
    TABLE.get_or_init(build_table)
}

pub fn sine(phase: u32) -> i32 {
    sine_table()[(phase >> (32 - TABLE_BITS)) as usize]
}

pub fn cosine(phase: u32) -> i32 {
    sine(phase.wrapping_add(QUARTER_TURN))
}

fn build_table() -> [i32; TABLE_SIZE] {
    let mut table = [0; TABLE_SIZE];
    for i in 0..=QUARTER {
        let angle = HALF_PI * i as f64 / QUARTER as f64;
        let value = (taylor_sine(angle) * f64::from(TABLE_ONE)).round() as i32;
        table[i] = value;
        table[2 * QUARTER - i] = value;
        table[2 * QUARTER + i] = -value;
        if i > 0 {
            table[4 * QUARTER - i] = -value;
        }
    }
    table
}

fn taylor_sine(x: f64) -> f64 {
    let square = x * x;
    let mut term = x;
    let mut sum = x;
    for k in 1..TAYLOR_TERMS {
        let k = f64::from(k);
        term = -term * square / ((2.0 * k) * (2.0 * k + 1.0));
        sum += term;
    }
    sum
}

pub fn isqrt(value: u128) -> u128 {
    if value == 0 {
        return 0;
    }
    let mut remainder = value;
    let mut root = 0u128;
    let mut bit = 1u128 << ((127 - value.leading_zeros()) & !1);
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

pub fn mul_fx(value: i64, gain_raw: i64) -> i64 {
    let product = (i128::from(value) * i128::from(gain_raw)) >> 32;
    product.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

pub fn ratio_fx(numerator: u64, denominator: u64) -> i64 {
    if denominator == 0 {
        return 1 << 32;
    }
    ((u128::from(numerator) << 32) / u128::from(denominator)).min(1 << 32) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use tickora_core::hash::StateHasher;

    #[test]
    fn the_sine_table_hash_is_pinned() {
        let mut hasher = StateHasher::new();
        for &value in sine_table() {
            hasher.write_i32(value);
        }
        assert_eq!(hasher.finish(), SINE_TABLE_HASH);
    }

    const SINE_TABLE_HASH: u64 = 12_690_677_489_224_471_301;

    #[test]
    fn the_sine_table_hits_its_peaks_and_zeros_exactly() {
        let table = sine_table();
        assert_eq!(table[0], 0);
        assert_eq!(table[QUARTER], TABLE_ONE);
        assert_eq!(table[2 * QUARTER], 0);
        assert_eq!(table[3 * QUARTER], -TABLE_ONE);
        assert_eq!(sine(0), 0);
        assert_eq!(cosine(0), TABLE_ONE);
        assert_eq!(sine(QUARTER_TURN), TABLE_ONE);
    }

    #[test]
    fn the_sine_table_is_close_to_the_true_sine() {
        let worst = sine_table()
            .iter()
            .enumerate()
            .map(|(i, &value)| {
                let angle = core::f64::consts::TAU * i as f64 / TABLE_SIZE as f64;
                (f64::from(value) / f64::from(TABLE_ONE) - angle.sin()).abs()
            })
            .fold(0.0, f64::max);
        assert!(worst < 1e-9, "{worst}");
    }

    #[test]
    fn isqrt_is_the_floor_of_the_square_root() {
        assert_eq!(isqrt(0), 0);
        assert_eq!(isqrt(1), 1);
        assert_eq!(isqrt(3), 1);
        assert_eq!(isqrt(4), 2);
        assert_eq!(isqrt(99), 9);
        assert_eq!(isqrt(100), 10);
        assert_eq!(isqrt(u128::MAX), u128::from(u64::MAX));
        let big = (1u128 << 100) + 12345;
        let root = isqrt(big);
        assert!(root * root <= big && (root + 1) * (root + 1) > big);
    }

    #[test]
    fn fixed_point_helpers_round_down_and_cap_at_one() {
        assert_eq!(mul_fx(1000, 1 << 31), 500);
        assert_eq!(mul_fx(-1000, 1 << 31), -500);
        assert_eq!(ratio_fx(1, 4), 1 << 30);
        assert_eq!(ratio_fx(5, 4), 1 << 32);
        assert_eq!(ratio_fx(3, 0), 1 << 32);
    }
}
