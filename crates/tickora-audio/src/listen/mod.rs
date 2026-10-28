pub mod analysis;
pub mod fft;
pub mod filter;
pub mod loudness;
pub mod onsets;
pub mod reading;
pub mod seam;
pub mod spectrum;
pub mod stats;
pub mod tones;
pub mod voice;
pub mod words;

use crate::format::{AudioFormat, FULL_SCALE};

pub const MAX_ANALYSIS_TICKS: u64 = 36_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clock {
    pub first_tick: u64,
    pub tick_rate: u32,
}

impl Default for Clock {
    fn default() -> Self {
        Clock {
            first_tick: 0,
            tick_rate: AudioFormat::DEFAULT.tick_rate(),
        }
    }
}

impl Clock {
    pub fn tick_at(self, seconds: f64) -> u64 {
        self.first_tick
            + (seconds * f64::from(self.tick_rate) + 1e-9)
                .floor()
                .max(0.0) as u64
    }
}

pub struct Audio {
    pub rate: u32,
    pub channels: [Vec<f64>; 2],
}

impl Audio {
    pub fn new(rate: u32, channels: [Vec<f64>; 2]) -> Self {
        Audio { rate, channels }
    }

    pub fn from_frames(rate: u32, frames: &[[i32; 2]]) -> Self {
        let scale = f64::from(FULL_SCALE);
        let channel = |c: usize| {
            frames
                .iter()
                .map(|frame| f64::from(frame[c]) / scale)
                .collect()
        };
        Audio::new(rate, [channel(0), channel(1)])
    }

    pub fn len(&self) -> usize {
        self.channels[0].len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn mono(&self) -> Vec<f64> {
        self.channels[0]
            .iter()
            .zip(&self.channels[1])
            .map(|(left, right)| (left + right) / 2.0)
            .collect()
    }

    pub fn duration(&self) -> f64 {
        self.len() as f64 / f64::from(self.rate)
    }
}
