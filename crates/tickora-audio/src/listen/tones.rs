use super::spectrum::Spectrogram;
use super::stats::{median, round_to};
use super::words::tone;

const MAINS: [f64; 6] = [50.0, 60.0, 100.0, 120.0, 150.0, 180.0];

#[derive(Clone, Debug, PartialEq)]
pub struct Tone {
    pub hz: f64,
    pub above_noise_db: f64,
    pub reads_as: &'static str,
}

pub fn medfilt(values: &[f64], kernel: usize) -> Vec<f64> {
    let half = kernel / 2;
    (0..values.len())
        .map(|i| {
            let window: Vec<f64> = (0..kernel)
                .map(|j| {
                    (i + j)
                        .checked_sub(half)
                        .and_then(|index| values.get(index))
                        .copied()
                        .unwrap_or(0.0)
                })
                .collect();
            median(&window)
        })
        .collect()
}

pub fn local_maxima(x: &[f64]) -> Vec<usize> {
    let mut peaks = Vec::new();
    if x.len() < 3 {
        return peaks;
    }
    let mut i = 1;
    let last = x.len() - 1;
    while i < last {
        if x[i - 1] < x[i] {
            let mut ahead = i + 1;
            while ahead < last && x[ahead] == x[i] {
                ahead += 1;
            }
            if x[ahead] < x[i] {
                peaks.push((i + ahead - 1) / 2);
                i = ahead;
            }
        }
        i += 1;
    }
    peaks
}

pub fn find_peaks(x: &[f64], height: f64, distance: usize) -> Vec<usize> {
    let peaks: Vec<usize> = local_maxima(x)
        .into_iter()
        .filter(|&peak| x[peak] >= height)
        .collect();
    let mut keep = vec![true; peaks.len()];
    let mut order: Vec<usize> = (0..peaks.len()).collect();
    order.sort_by(|&a, &b| x[peaks[a]].total_cmp(&x[peaks[b]]));
    for &j in order.iter().rev() {
        if !keep[j] {
            continue;
        }
        let mut k = j;
        while k > 0 && peaks[j] - peaks[k - 1] < distance {
            keep[k - 1] = false;
            k -= 1;
        }
        let mut k = j + 1;
        while k < peaks.len() && peaks[k] - peaks[j] < distance {
            keep[k] = false;
            k += 1;
        }
    }
    peaks
        .into_iter()
        .zip(keep)
        .filter_map(|(peak, kept)| kept.then_some(peak))
        .collect()
}

pub fn tones(spectrogram: &Spectrogram) -> Vec<Tone> {
    let bins = spectrogram.freqs.len();
    let log: Vec<f64> = (0..bins)
        .map(|bin| {
            let across: Vec<f64> = spectrogram.power.iter().map(|frame| frame[bin]).collect();
            10.0 * (median(&across) + 1e-20).log10()
        })
        .collect();
    let smooth = medfilt(&log, 31);
    let lifted: Vec<f64> = log.iter().zip(&smooth).map(|(a, b)| a - b).collect();
    let mut peaks: Vec<(usize, f64)> = find_peaks(&lifted, 10.0, 5)
        .into_iter()
        .map(|peak| (peak, lifted[peak]))
        .collect();
    peaks.sort_by(|a, b| b.1.total_cmp(&a.1));
    peaks
        .into_iter()
        .take(6)
        .filter_map(|(peak, height)| {
            let hz = spectrogram.freqs[peak];
            if hz < 20.0 {
                return None;
            }
            let mains = MAINS.iter().any(|m| (hz - m).abs() < 3.0);
            Some(Tone {
                hz: round_to(hz, 1),
                above_noise_db: round_to(height, 1),
                reads_as: tone(hz, mains),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::listen::spectrum::spectrogram;
    use std::f64::consts::PI;

    #[test]
    fn medfilt_pads_the_edges_with_zeros() {
        assert_eq!(medfilt(&[5.0, 5.0, 5.0], 3), vec![5.0, 5.0, 5.0]);
        assert_eq!(medfilt(&[5.0, 1.0, 9.0, 2.0], 3), vec![1.0, 5.0, 2.0, 2.0]);
    }

    #[test]
    fn peaks_take_plateau_midpoints_and_honour_the_distance() {
        let x = [0.0, 2.0, 2.0, 2.0, 0.0, 5.0, 0.0, 3.0, 0.0];
        assert_eq!(local_maxima(&x), vec![2, 5, 7]);
        assert_eq!(find_peaks(&x, 2.5, 1), vec![5, 7]);
        assert_eq!(find_peaks(&x, 0.0, 3), vec![2, 5]);
    }

    #[test]
    fn a_whistle_over_noise_reads_as_a_tone() {
        let mut rng = tickora_core::rng::Rng::from_seed(9);
        let mono: Vec<f64> = (0..48_000)
            .map(|i| {
                let noise = f64::from(rng.next_u32()) / f64::from(u32::MAX) - 0.5;
                0.05 * noise + 0.2 * (2.0 * PI * 3000.0 * i as f64 / 48_000.0).sin()
            })
            .collect();
        let found = tones(&spectrogram(&mono, 48_000).unwrap());
        assert!((found[0].hz - 3000.0).abs() < 24.0);
        assert_eq!(found[0].reads_as, "whistle or tone");
        assert!(found[0].above_noise_db > 15.0);
    }
}
