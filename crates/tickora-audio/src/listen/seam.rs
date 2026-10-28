use std::collections::BTreeMap;
use std::f64::consts::PI;

use super::Clock;
use super::fft::Fft;
use super::stats::round_to;

#[derive(Clone, Debug, PartialEq)]
pub struct Seam {
    pub level_jump_db: f64,
    pub spectral_jump_db: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoopCandidate {
    pub start_s: f64,
    pub end_s: f64,
    pub start_tick: u64,
    pub end_tick: u64,
    pub seam: Seam,
}

pub struct SeamProbe<'a> {
    mono: &'a [f64],
    rate: u32,
    window: usize,
    plan: Fft,
    hanning: Vec<f64>,
    spectra: BTreeMap<usize, Vec<f64>>,
}

impl<'a> SeamProbe<'a> {
    pub fn new(mono: &'a [f64], rate: u32) -> Self {
        let window = (0.2 * f64::from(rate)) as usize;
        let hanning = (0..window)
            .map(|n| 0.5 - 0.5 * (2.0 * PI * n as f64 / (window - 1) as f64).cos())
            .collect();
        SeamProbe {
            mono,
            rate,
            window,
            plan: Fft::new(window),
            hanning,
            spectra: BTreeMap::new(),
        }
    }

    pub fn seam(&mut self, start_s: f64, end_s: f64) -> Option<Seam> {
        let rate = f64::from(self.rate);
        let head_at = (start_s * rate) as usize;
        let tail_end = (end_s * rate) as usize;
        let tail_at = tail_end.checked_sub(self.window)?;
        if head_at + self.window > self.mono.len() || tail_end > self.mono.len() {
            return None;
        }
        let rms = |at: usize| {
            let piece = &self.mono[at..at + self.window];
            (piece.iter().map(|x| x * x).sum::<f64>() / self.window as f64).sqrt()
        };
        let level = (20.0 * ((rms(head_at) + 1e-12) / (rms(tail_at) + 1e-12)).log10()).abs();
        let head = self.third_octaves(head_at);
        let tail = self.third_octaves(tail_at);
        let shape = head
            .iter()
            .zip(&tail)
            .map(|(a, b)| (a - b).abs())
            .sum::<f64>()
            / head.len() as f64;
        Some(Seam {
            level_jump_db: round_to(level, 1),
            spectral_jump_db: round_to(shape, 1),
        })
    }

    fn third_octaves(&mut self, at: usize) -> Vec<f64> {
        if let Some(levels) = self.spectra.get(&at) {
            return levels.clone();
        }
        let piece: Vec<f64> = self.mono[at..at + self.window]
            .iter()
            .zip(&self.hanning)
            .map(|(x, w)| x * w)
            .collect();
        let power: Vec<f64> = self
            .plan
            .real(&piece)
            .iter()
            .map(|bin| bin.norm_sqr())
            .collect();
        let rate = f64::from(self.rate);
        let freqs: Vec<f64> = (0..power.len())
            .map(|k| k as f64 * rate / self.window as f64)
            .collect();
        let edge = 2f64.powf(1.0 / 6.0);
        let levels: Vec<f64> = (-17..13)
            .map(|i| 1000.0 * 2f64.powf(f64::from(i) / 3.0))
            .filter(|centre| *centre < rate / 2.3)
            .map(|centre| {
                freqs
                    .iter()
                    .zip(&power)
                    .filter(|(hz, _)| **hz >= centre / edge && **hz < centre * edge)
                    .map(|(_, p)| p)
                    .sum::<f64>()
            })
            .collect();
        let total = levels.iter().sum::<f64>() + 1e-20;
        let levels: Vec<f64> = levels
            .iter()
            .map(|level| 10.0 * (level / total + 1e-9).log10())
            .collect();
        self.spectra.insert(at, levels.clone());
        levels
    }
}

pub fn loop_search(
    mono: &[f64],
    rate: u32,
    clock: Clock,
    min_s: f64,
    max_s: f64,
) -> Vec<LoopCandidate> {
    let duration = mono.len() as f64 / f64::from(rate);
    let steps = ((duration - 0.2 - 0.2) / 0.25).ceil().max(0.0) as usize;
    let grid: Vec<f64> = (0..steps).map(|i| 0.2 + i as f64 * 0.25).collect();
    let mut probe = SeamProbe::new(mono, rate);
    let mut candidates = Vec::new();
    for &start in &grid {
        for &end in &grid {
            let length = end - start;
            if min_s <= length && length <= max_s {
                if let Some(seam) = probe.seam(start, end) {
                    let score = seam.level_jump_db + seam.spectral_jump_db * 0.5;
                    candidates.push((score, start, end, seam));
                }
            }
        }
    }
    candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut picked: Vec<LoopCandidate> = Vec::new();
    for (_, start, end, seam) in candidates {
        let apart = picked
            .iter()
            .all(|other| (start - other.start_s).abs() > 2.0 || (end - other.end_s).abs() > 2.0);
        if apart {
            picked.push(LoopCandidate {
                start_s: round_to(start, 2),
                end_s: round_to(end, 2),
                start_tick: clock.tick_at(start),
                end_tick: clock.tick_at(end),
                seam,
            });
        }
        if picked.len() == 3 {
            break;
        }
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(seconds: f64) -> Vec<f64> {
        (0..(seconds * 48_000.0) as usize)
            .map(|i| 0.1 * (2.0 * PI * 500.0 * i as f64 / 48_000.0).sin())
            .collect()
    }

    #[test]
    fn a_steady_tone_loops_without_a_jump() {
        let mono = tone(2.0);
        let seam = SeamProbe::new(&mono, 48_000).seam(0.0, 2.0).unwrap();
        assert_eq!(seam.level_jump_db, 0.0);
        assert!(seam.spectral_jump_db < 0.5);
    }

    #[test]
    fn a_fade_shows_as_a_level_jump() {
        let mut mono = tone(2.0);
        for value in &mut mono[48_000..] {
            *value *= 0.1;
        }
        let seam = SeamProbe::new(&mono, 48_000).seam(0.0, 2.0).unwrap();
        assert_eq!(seam.level_jump_db, 20.0);
        assert!(
            SeamProbe::new(&mono[..5000], 48_000)
                .seam(0.0, 0.1)
                .is_none()
        );
    }

    #[test]
    fn loop_candidates_sit_inside_the_length_range() {
        let mono = tone(6.0);
        let found = loop_search(&mono, 48_000, Clock::default(), 1.0, 2.0);
        assert_eq!(found.len(), 3);
        for candidate in found {
            let length = candidate.end_s - candidate.start_s;
            assert!((1.0..=2.0).contains(&length));
        }
    }
}
