use super::fft::Fft;
use super::spectrum::{BANDS, band_sum};
use super::stats::{median, percentile, round_to};
use super::words::describe_event;

const BACKGROUND_FRAMES: usize = 30;

#[derive(Clone, Debug, PartialEq)]
pub struct Onset {
    pub time_s: f64,
    pub rise_db: f64,
    pub length_ms: i64,
    pub band: &'static str,
    pub reads_as: &'static str,
}

pub fn frame_levels(mono: &[f64], rate: u32) -> Vec<f64> {
    let hop = (0.01 * f64::from(rate)) as usize;
    mono.chunks_exact(hop)
        .map(|chunk| {
            let mean = chunk.iter().map(|x| x * x).sum::<f64>() / hop as f64;
            20.0 * (mean + 1e-20).sqrt().log10()
        })
        .collect()
}

pub fn band_of(mono: &[f64], rate: u32, start_s: f64) -> &'static str {
    let rate_f = f64::from(rate);
    let from = ((start_s * rate_f) as usize).min(mono.len());
    let to = (((start_s + 0.05) * rate_f) as usize).clamp(from, mono.len());
    let piece = &mono[from..to];
    if piece.len() < 64 {
        return "unknown";
    }
    let mean = piece.iter().sum::<f64>() / piece.len() as f64;
    let detrended: Vec<f64> = piece.iter().map(|x| x - mean).collect();
    let power: Vec<f64> = Fft::new(piece.len())
        .real(&detrended)
        .iter()
        .map(|bin| bin.norm_sqr())
        .collect();
    let freqs: Vec<f64> = (0..power.len())
        .map(|k| k as f64 * rate_f / piece.len() as f64)
        .collect();
    let mut best = ("unknown", f64::NEG_INFINITY);
    for (name, low, high) in BANDS {
        let energy = band_sum(&freqs, &power, |hz| hz >= low && hz < high);
        if energy > best.1 {
            best = (name, energy);
        }
    }
    best.0
}

pub fn onsets(mono: &[f64], rate: u32, rise_db: f64, limit: usize) -> (Vec<Onset>, usize) {
    let levels = frame_levels(mono, rate);
    let mut found = Vec::new();
    if levels.is_empty() {
        return (found, 0);
    }
    let floor = percentile(&levels, 10.0);
    let mut index = BACKGROUND_FRAMES;
    while index < levels.len() {
        let background = median(&levels[index - BACKGROUND_FRAMES..index]);
        if levels[index] - background >= rise_db && levels[index] > floor + rise_db {
            let window = &levels[index..(index + 10).min(levels.len())];
            let offset =
                window.iter().enumerate().fold(
                    0,
                    |best, (i, level)| if *level > window[best] { i } else { best },
                );
            let peak = index + offset;
            let mut end = peak;
            while end < levels.len() - 1 && levels[end] > background + 3.0 {
                end += 1;
            }
            let start_s = index as f64 * 0.01;
            let band = band_of(mono, rate, start_s);
            let length_ms = ((end - index) * 10) as i64;
            found.push(Onset {
                time_s: round_to(start_s, 2),
                rise_db: round_to(levels[peak] - background, 1),
                length_ms,
                band,
                reads_as: describe_event(band, length_ms),
            });
            index = end + 1;
        } else {
            index += 1;
        }
    }
    let total = found.len();
    found.sort_by(|a, b| b.rise_db.total_cmp(&a.rise_db));
    found.truncate(limit);
    found.sort_by(|a, b| a.time_s.total_cmp(&b.time_s));
    (found, total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn clicks(times_s: &[f64], hz: f64) -> Vec<f64> {
        let mut mono = vec![0.0; 48_000 * 3];
        for (i, sample) in mono.iter_mut().enumerate() {
            *sample = 1e-4 * ((i * 7919) % 101) as f64 / 101.0;
        }
        for &time in times_s {
            let start = (time * 48_000.0) as usize;
            for j in 0..2400 {
                mono[start + j] += 0.5 * (2.0 * PI * hz * j as f64 / 48_000.0).sin();
            }
        }
        mono
    }

    #[test]
    fn bursts_are_found_where_they_start_with_their_band() {
        let (found, total) = onsets(&clicks(&[0.8, 1.6, 2.4], 1000.0), 48_000, 10.0, 15);
        assert_eq!(total, 3);
        let times: Vec<f64> = found.iter().map(|onset| onset.time_s).collect();
        assert_eq!(times, vec![0.8, 1.6, 2.4]);
        assert!(found.iter().all(|onset| onset.band == "mid"));
        assert!(found.iter().all(|onset| onset.length_ms <= 60));
        assert!(
            found
                .iter()
                .all(|onset| onset.reads_as == "knock, step or voice-band burst")
        );
    }

    #[test]
    fn the_limit_keeps_the_loudest_in_time_order() {
        let (found, total) = onsets(&clicks(&[0.8, 1.6, 2.4], 100.0), 48_000, 10.0, 2);
        assert_eq!((found.len(), total), (2, 3));
        assert!(found[0].time_s < found[1].time_s);
        assert_eq!(found[0].band, "low");
    }
}
