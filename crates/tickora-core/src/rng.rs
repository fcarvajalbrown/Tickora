const MULTIPLIER: u64 = 6_364_136_223_846_793_005;
pub const DEFAULT_STREAM: u64 = 54;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    state: u64,
    increment: u64,
}

impl Rng {
    pub fn new(seed: u64, stream: u64) -> Self {
        let mut rng = Self {
            state: 0,
            increment: (stream << 1) | 1,
        };
        rng.advance();
        rng.state = rng.state.wrapping_add(seed);
        rng.advance();
        rng
    }

    pub fn from_seed(seed: u64) -> Self {
        Self::new(seed, DEFAULT_STREAM)
    }

    pub fn raw_parts(&self) -> (u64, u64) {
        (self.state, self.increment)
    }

    fn advance(&mut self) {
        self.state = self
            .state
            .wrapping_mul(MULTIPLIER)
            .wrapping_add(self.increment);
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.advance();
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rotation = (old >> 59) as u32;
        xorshifted.rotate_right(rotation)
    }

    pub fn next_u64(&mut self) -> u64 {
        let high = u64::from(self.next_u32());
        let low = u64::from(self.next_u32());
        (high << 32) | low
    }

    pub fn below(&mut self, bound: u32) -> u32 {
        assert!(bound > 0, "bound must be positive");
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let value = self.next_u32();
            if value >= threshold {
                return value % bound;
            }
        }
    }

    pub fn chance(&mut self, numerator: u32, denominator: u32) -> bool {
        self.below(denominator) < numerator
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_reference_pcg32_vector() {
        let mut rng = Rng::new(42, 54);
        let produced: Vec<u32> = (0..6).map(|_| rng.next_u32()).collect();
        assert_eq!(
            produced,
            [
                0xa15c02b7, 0x7b47f409, 0xba1d3330, 0x83d2f293, 0xbfa4784b, 0xcbed606e
            ]
        );
    }

    #[test]
    fn same_seed_same_stream() {
        let mut a = Rng::from_seed(7);
        let mut b = Rng::from_seed(7);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::from_seed(1);
        let mut b = Rng::from_seed(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn below_stays_in_range_and_covers_it() {
        let mut rng = Rng::from_seed(99);
        let mut seen = [false; 10];
        for _ in 0..1000 {
            let value = rng.below(10);
            assert!(value < 10);
            seen[value as usize] = true;
        }
        assert!(seen.iter().all(|&hit| hit));
    }

    #[test]
    fn chance_extremes() {
        let mut rng = Rng::from_seed(5);
        assert!((0..100).all(|_| rng.chance(1, 1)));
        assert!((0..100).all(|_| !rng.chance(0, 1)));
    }
}
