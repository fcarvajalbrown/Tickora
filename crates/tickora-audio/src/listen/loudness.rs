use super::filter::{k_weight, upsampled_peak, upsampling_filter};
use super::stats::power_mean_db;

pub struct Blocks {
    pub times: Vec<f64>,
    pub levels: Vec<f64>,
}

pub fn block_loudness(channels: &[Vec<f64>], rate: u32, window_s: f64, hop_s: f64) -> Blocks {
    let rate = f64::from(rate);
    let window = (window_s * rate) as usize;
    let hop = ((hop_s * rate) as usize).max(1);
    let length = channels.first().map_or(0, Vec::len);
    if length < window || window == 0 {
        return Blocks {
            times: Vec::new(),
            levels: Vec::new(),
        };
    }
    let cumulative: Vec<Vec<f64>> = channels
        .iter()
        .map(|channel| {
            let mut sums = Vec::with_capacity(length + 1);
            sums.push(0.0);
            let mut total = 0.0;
            for value in k_weight(channel, rate) {
                total += value * value;
                sums.push(total);
            }
            sums
        })
        .collect();
    let starts: Vec<usize> = (0..=length - window).step_by(hop).collect();
    let levels = starts
        .iter()
        .map(|&start| {
            let power: f64 = cumulative
                .iter()
                .map(|sums| (sums[start + window] - sums[start]) / window as f64)
                .sum();
            -0.691 + 10.0 * power.max(1e-12).log10()
        })
        .collect();
    Blocks {
        times: starts.iter().map(|&start| start as f64 / rate).collect(),
        levels,
    }
}

pub fn integrated(channels: &[Vec<f64>], rate: u32) -> Option<f64> {
    let blocks: Vec<f64> = block_loudness(channels, rate, 0.4, 0.1)
        .levels
        .into_iter()
        .filter(|level| *level > -70.0)
        .collect();
    if blocks.is_empty() {
        return None;
    }
    let relative = power_mean_db(&blocks) - 10.0;
    let gated: Vec<f64> = blocks
        .into_iter()
        .filter(|level| *level > relative)
        .collect();
    Some(power_mean_db(&gated))
}

pub fn true_peak(channels: &[Vec<f64>]) -> f64 {
    let h = upsampling_filter(4);
    let peak = channels
        .iter()
        .map(|channel| upsampled_peak(channel, 4, &h))
        .fold(0.0, f64::max);
    20.0 * peak.max(1e-12).log10()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::listen::Audio;
    use std::f64::consts::PI;

    fn sine(hz: f64, amplitude: f64, seconds: f64) -> Audio {
        let frames = (seconds * 48_000.0) as usize;
        let channel: Vec<f64> = (0..frames)
            .map(|i| amplitude * (2.0 * PI * hz * i as f64 / 48_000.0).sin())
            .collect();
        Audio::new(48_000, [channel.clone(), channel])
    }

    #[test]
    fn a_1_khz_sine_at_minus_20_dbfs_reads_as_listen_py_does() {
        let audio = sine(1000.0, 0.1, 3.0);
        assert!((integrated(&audio.channels, audio.rate).unwrap() + 20.2529).abs() < 0.001);
        assert!((true_peak(&audio.channels) + 19.9945).abs() < 0.001);
    }

    #[test]
    fn silence_has_no_integrated_loudness() {
        let audio = sine(1000.0, 0.0, 1.0);
        assert_eq!(integrated(&audio.channels, audio.rate), None);
        assert_eq!(true_peak(&audio.channels), -240.0);
    }

    #[test]
    fn blocks_step_by_the_hop() {
        let blocks = block_loudness(&sine(1000.0, 0.1, 1.0).channels, 48_000, 0.4, 0.1);
        assert_eq!(blocks.times.len(), 7);
        assert!((blocks.times[6] - 0.6).abs() < 1e-12);
        assert!(
            block_loudness(&sine(1000.0, 0.1, 0.3).channels, 48_000, 0.4, 0.1)
                .levels
                .is_empty()
        );
    }
}
