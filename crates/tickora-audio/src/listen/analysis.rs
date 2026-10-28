use super::loudness::{block_loudness, integrated, true_peak};
use super::onsets::{Onset, onsets};
use super::seam::{LoopCandidate, Seam, SeamProbe, loop_search};
use super::spectrum::{BANDS, band_shares, centroids, flatness, spectrogram};
use super::stats::{mean, median, percentile, power_mean_db, round_to};
use super::tones::{Tone, tones};
use super::voice::voice_likeness;
use super::words::{colour, steadiness, texture, width};
use super::{Audio, Clock};

#[derive(Clone, Debug)]
pub struct Settings {
    pub segments: usize,
    pub event_db: f64,
    pub max_events: usize,
    pub loop_search: Option<(f64, f64)>,
    pub clock: Clock,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            segments: 12,
            event_db: 10.0,
            max_events: 15,
            loop_search: None,
            clock: Clock::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Overall {
    pub centroid_hz: f64,
    pub bands_pct: [f64; 5],
    pub reads_as: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stereo {
    pub correlation: f64,
    pub side_to_mid: f64,
    pub mono_fold_loss_db: Option<f64>,
    pub reads_as: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub from_s: f64,
    pub to_s: f64,
    pub from_tick: u64,
    pub to_tick: u64,
    pub loudness_lufs: f64,
    pub swing_db: f64,
    pub centroid_hz: f64,
    pub bands_pct: [f64; 5],
    pub reads_as: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Analysis {
    pub duration_s: f64,
    pub first_tick: u64,
    pub ticks: u64,
    pub rate: u32,
    pub integrated_lufs: Option<f64>,
    pub true_peak_dbtp: f64,
    pub short_term_range_db: f64,
    pub clipped_samples: u64,
    pub dc_offset: f64,
    pub overall: Overall,
    pub stereo: Stereo,
    pub voice_likeness: Option<f64>,
    pub tones: Vec<Tone>,
    pub events: Vec<Onset>,
    pub event_count: usize,
    pub event_rise_db: f64,
    pub segments: Vec<Segment>,
    pub whole_file_seam: Option<Seam>,
    pub loops: Option<Vec<LoopCandidate>>,
}

pub fn band_names() -> [&'static str; 5] {
    BANDS.map(|(name, _, _)| name)
}

fn stereo(audio: &Audio) -> Stereo {
    let [left, right] = &audio.channels;
    let folded = audio.mono();
    let fold_loss = match (
        integrated(&audio.channels, audio.rate),
        integrated(std::slice::from_ref(&folded), audio.rate),
    ) {
        (Some(both), Some(mono)) => Some(round_to(both - (mono + 3.01), 1)),
        _ => None,
    };
    let std = |x: &[f64]| {
        let m = mean(x);
        (x.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / x.len() as f64).sqrt()
    };
    let correlation = if std(left) > 0.0 && std(right) > 0.0 {
        let (ml, mr) = (mean(left), mean(right));
        let covariance: f64 = left
            .iter()
            .zip(right)
            .map(|(l, r)| (l - ml) * (r - mr))
            .sum();
        let norm_l: f64 = left.iter().map(|l| (l - ml) * (l - ml)).sum();
        let norm_r: f64 = right.iter().map(|r| (r - mr) * (r - mr)).sum();
        (covariance / (norm_l * norm_r).sqrt()).clamp(-1.0, 1.0)
    } else {
        1.0
    };
    let rms = |values: Vec<f64>| (mean(&values)).sqrt();
    let side = rms(left
        .iter()
        .zip(right)
        .map(|(l, r)| (l - r) * (l - r))
        .collect())
        / (rms(left
            .iter()
            .zip(right)
            .map(|(l, r)| (l + r) * (l + r))
            .collect())
            + 1e-20);
    Stereo {
        correlation: round_to(correlation, 2),
        side_to_mid: round_to(side, 2),
        mono_fold_loss_db: fold_loss,
        reads_as: width(correlation),
    }
}

fn bands_pct(shares: &[[f64; 5]], keep: impl Fn(usize) -> bool) -> [f64; 5] {
    let mut out = [0.0; 5];
    for (band, value) in out.iter_mut().enumerate() {
        let picked: Vec<f64> = shares
            .iter()
            .enumerate()
            .filter(|(i, _)| keep(*i))
            .map(|(_, share)| share[band])
            .collect();
        *value = round_to(mean(&picked) * 100.0, 0);
    }
    out
}

pub fn analyse(audio: &Audio, settings: &Settings) -> Result<Analysis, String> {
    let rate = audio.rate;
    let mono = audio.mono();
    let spectrum = spectrogram(&mono, rate)?;
    let short_term: Vec<f64> = block_loudness(&audio.channels, rate, 3.0, 0.1)
        .levels
        .into_iter()
        .filter(|level| *level > -70.0)
        .collect();
    let shares = band_shares(&spectrum);
    let centres = centroids(&spectrum);
    let flat = flatness(&spectrum);
    let overall_spread = if short_term.is_empty() {
        0.0
    } else {
        percentile(&short_term, 90.0) - percentile(&short_term, 10.0)
    };
    let overall_centroid = median(&centres);
    let (events, event_count) = onsets(
        &mono,
        rate,
        settings.clock,
        settings.event_db,
        settings.max_events,
    );
    let tone_list = tones(&spectrum);
    let tonal = tone_list.iter().any(|tone| tone.above_noise_db >= 15.0);
    let duration = audio.duration();
    let blocks = block_loudness(&audio.channels, rate, 0.4, 0.1);
    let count = settings.segments.max(1);
    let edges: Vec<f64> = (0..=count)
        .map(|i| {
            if i == count {
                duration
            } else {
                i as f64 * (duration / count as f64)
            }
        })
        .collect();
    let segments = edges
        .windows(2)
        .filter_map(|edge| {
            let (start, end) = (edge[0], edge[1]);
            let level: Vec<f64> = blocks
                .times
                .iter()
                .zip(&blocks.levels)
                .filter(|(t, _)| **t >= start && **t < end)
                .map(|(_, level)| *level)
                .collect();
            let frames: Vec<usize> = (0..spectrum.times.len())
                .filter(|&i| spectrum.times[i] >= start && spectrum.times[i] < end)
                .collect();
            if level.is_empty() || frames.is_empty() {
                return None;
            }
            let hz = median(&frames.iter().map(|&i| centres[i]).collect::<Vec<_>>());
            let noise = median(&frames.iter().map(|&i| flat[i]).collect::<Vec<_>>());
            let spread = percentile(&level, 90.0) - percentile(&level, 10.0);
            Some(Segment {
                from_s: round_to(start, 1),
                to_s: round_to(end, 1),
                from_tick: settings.clock.tick_at(start),
                to_tick: settings.clock.tick_at(end),
                loudness_lufs: round_to(power_mean_db(&level), 1),
                swing_db: round_to(spread, 1),
                centroid_hz: round_to(hz, 0),
                bands_pct: bands_pct(&shares, |i| frames.contains(&i)),
                reads_as: format!(
                    "{}, {}, {}",
                    steadiness(spread),
                    colour(hz),
                    texture(noise, tonal)
                ),
            })
        })
        .collect();
    let clipped = audio
        .channels
        .iter()
        .flatten()
        .filter(|value| value.abs() >= 0.9999)
        .count() as u64;
    let short_term_range = if short_term.is_empty() {
        0.0
    } else {
        round_to(
            percentile(&short_term, 95.0) - percentile(&short_term, 10.0),
            1,
        )
    };
    Ok(Analysis {
        duration_s: round_to(duration, 2),
        first_tick: settings.clock.first_tick,
        ticks: settings.clock.tick_at(duration) - settings.clock.first_tick,
        rate,
        integrated_lufs: integrated(&audio.channels, rate).map(|lufs| round_to(lufs, 1)),
        true_peak_dbtp: round_to(true_peak(&audio.channels), 1),
        short_term_range_db: short_term_range,
        clipped_samples: clipped,
        dc_offset: round_to(mean(&mono), 4),
        overall: Overall {
            centroid_hz: round_to(overall_centroid, 0),
            bands_pct: bands_pct(&shares, |_| true),
            reads_as: format!(
                "{}, {}, {}",
                steadiness(overall_spread),
                colour(overall_centroid),
                texture(median(&flat), tonal)
            ),
        },
        stereo: stereo(audio),
        voice_likeness: voice_likeness(&mono, rate),
        tones: tone_list,
        events,
        event_count,
        event_rise_db: settings.event_db,
        segments,
        whole_file_seam: SeamProbe::new(&mono, rate).seam(0.0, mono.len() as f64 / f64::from(rate)),
        loops: settings
            .loop_search
            .map(|(min_s, max_s)| loop_search(&mono, rate, settings.clock, min_s, max_s)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn a_hard_left_tone_reads_as_listen_py_reads_it() {
        let left: Vec<f64> = (0..48_000 * 2)
            .map(|i| 0.1 * (2.0 * PI * 440.0 * i as f64 / 48_000.0).sin())
            .collect();
        let audio = Audio::new(48_000, [left, vec![0.0; 96_000]]);
        let analysis = analyse(&audio, &Settings::default()).unwrap();
        assert_eq!(analysis.stereo.correlation, 1.0);
        assert_eq!(analysis.stereo.side_to_mid, 1.0);
        assert_eq!(analysis.stereo.mono_fold_loss_db, Some(3.0));
        assert_eq!(analysis.segments.len(), 10);
        assert_eq!(analysis.clipped_samples, 0);
        assert_eq!(analysis.overall.bands_pct[2], 100.0);
        assert!(
            analysis
                .overall
                .reads_as
                .starts_with("steady, warm, low-mid")
        );
    }

    #[test]
    fn a_range_shorter_than_one_frame_is_an_error() {
        let audio = Audio::new(48_000, [vec![0.0; 1000], vec![0.0; 1000]]);
        assert!(analyse(&audio, &Settings::default()).is_err());
    }
}
