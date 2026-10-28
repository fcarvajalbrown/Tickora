pub const FULL_SCALE: i32 = 1 << 23;
pub const MAX_SAMPLE: i32 = FULL_SCALE - 1;
pub const CHANNELS: u16 = 2;
pub const BITS_PER_SAMPLE: u16 = 24;
pub const MAX_SAMPLE_RATE: u32 = 192_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioFormat {
    sample_rate: u32,
    tick_rate: u32,
}

impl AudioFormat {
    pub const DEFAULT: AudioFormat = AudioFormat {
        sample_rate: 48_000,
        tick_rate: 60,
    };

    pub fn new(sample_rate: u32, tick_rate: u32) -> Result<AudioFormat, String> {
        if sample_rate == 0 || sample_rate > MAX_SAMPLE_RATE {
            return Err(format!(
                "sample_rate must be from 1 to {MAX_SAMPLE_RATE}, got {sample_rate}"
            ));
        }
        if tick_rate == 0 || !sample_rate.is_multiple_of(tick_rate) {
            return Err(format!(
                "sample_rate {sample_rate} must be a multiple of tick_rate {tick_rate}"
            ));
        }
        Ok(AudioFormat {
            sample_rate,
            tick_rate,
        })
    }

    pub const fn sample_rate(self) -> u32 {
        self.sample_rate
    }

    pub const fn tick_rate(self) -> u32 {
        self.tick_rate
    }

    pub const fn samples_per_tick(self) -> u32 {
        self.sample_rate / self.tick_rate
    }
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_800_samples_per_tick() {
        assert_eq!(AudioFormat::default().samples_per_tick(), 800);
        assert_eq!(AudioFormat::new(48_000, 60), Ok(AudioFormat::DEFAULT));
    }

    #[test]
    fn the_sample_rate_must_be_a_multiple_of_the_tick_rate() {
        assert!(AudioFormat::new(44_100, 64).is_err());
        assert!(AudioFormat::new(44_100, 60).is_ok());
        assert!(AudioFormat::new(48_000, 0).is_err());
        assert!(AudioFormat::new(0, 60).is_err());
        assert!(AudioFormat::new(384_000, 60).is_err());
    }
}
