use std::f64::consts::PI;

use super::fft::Fft;

pub const SEGMENT: usize = 2048;
pub const HOP: usize = 512;

pub const BANDS: [(&str, f64, f64); 5] = [
    ("sub", 20.0, 80.0),
    ("low", 80.0, 300.0),
    ("mid", 300.0, 2000.0),
    ("presence", 2000.0, 6000.0),
    ("air", 6000.0, 20000.0),
];

pub struct Spectrogram {
    pub freqs: Vec<f64>,
    pub times: Vec<f64>,
    pub power: Vec<Vec<f64>>,
}

pub fn hann_periodic(size: usize) -> Vec<f64> {
    (0..size)
        .map(|n| 0.5 - 0.5 * (2.0 * PI * n as f64 / size as f64).cos())
        .collect()
}

pub fn spectrogram(mono: &[f64], rate: u32) -> Result<Spectrogram, String> {
    if mono.len() < SEGMENT {
        return Err(format!(
            "the range holds {} samples; the analysis needs at least {SEGMENT}",
            mono.len()
        ));
    }
    let extra = (HOP - (mono.len() - SEGMENT) % HOP) % HOP;
    let padded = mono.len() + extra;
    let window = hann_periodic(SEGMENT);
    let scale = 1.0 / window.iter().sum::<f64>();
    let plan = Fft::new(SEGMENT);
    let rate = f64::from(rate);
    let starts: Vec<usize> = (0..=padded - SEGMENT).step_by(HOP).collect();
    let power = starts
        .iter()
        .map(|&start| {
            let frame: Vec<f64> = (0..SEGMENT)
                .map(|i| mono.get(start + i).copied().unwrap_or(0.0) * window[i])
                .collect();
            plan.real(&frame)
                .iter()
                .map(|bin| bin.norm_sqr() * scale * scale)
                .collect()
        })
        .collect();
    Ok(Spectrogram {
        freqs: (0..=SEGMENT / 2)
            .map(|k| k as f64 * rate / SEGMENT as f64)
            .collect(),
        times: starts
            .iter()
            .map(|&start| (start + SEGMENT / 2) as f64 / rate)
            .collect(),
        power,
    })
}

pub fn band_shares(spectrogram: &Spectrogram) -> Vec<[f64; 5]> {
    spectrogram
        .power
        .iter()
        .map(|frame| {
            let total = frame.iter().sum::<f64>() + 1e-20;
            BANDS.map(|(_, low, high)| {
                band_sum(&spectrogram.freqs, frame, |hz| hz >= low && hz < high) / total
            })
        })
        .collect()
}

pub fn band_sum(freqs: &[f64], frame: &[f64], keep: impl Fn(f64) -> bool) -> f64 {
    freqs
        .iter()
        .zip(frame)
        .filter(|(hz, _)| keep(**hz))
        .map(|(_, power)| power)
        .sum()
}

pub fn centroids(spectrogram: &Spectrogram) -> Vec<f64> {
    spectrogram
        .power
        .iter()
        .map(|frame| {
            let weighted: f64 = spectrogram
                .freqs
                .iter()
                .zip(frame)
                .map(|(hz, power)| hz * power)
                .sum();
            weighted / (frame.iter().sum::<f64>() + 1e-20)
        })
        .collect()
}

pub fn flatness(spectrogram: &Spectrogram) -> Vec<f64> {
    spectrogram
        .power
        .iter()
        .map(|frame| {
            let floor: Vec<f64> = spectrogram
                .freqs
                .iter()
                .zip(frame)
                .filter(|(hz, _)| **hz >= 100.0 && **hz <= 8000.0)
                .map(|(_, power)| power + 1e-20)
                .collect();
            let count = floor.len() as f64;
            let log_mean = floor.iter().map(|value| value.ln()).sum::<f64>() / count;
            log_mean.exp() / (floor.iter().sum::<f64>() / count)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f64, samples: usize) -> Vec<f64> {
        (0..samples)
            .map(|i| 0.1 * (2.0 * PI * hz * i as f64 / 48_000.0).sin())
            .collect()
    }

    #[test]
    fn frames_follow_scipy_padding_and_centres() {
        let spectrogram = spectrogram(&tone(1000.0, 48_000), 48_000).unwrap();
        assert_eq!(spectrogram.freqs.len(), 1025);
        assert_eq!(spectrogram.power.len(), 91);
        assert!((spectrogram.times[0] - 1024.0 / 48_000.0).abs() < 1e-12);
        assert!(super::spectrogram(&tone(1000.0, 2000), 48_000).is_err());
    }

    #[test]
    fn a_1_khz_tone_is_mid_band_centred_and_not_flat() {
        let spectrogram = spectrogram(&tone(1000.0, 48_000), 48_000).unwrap();
        let shares = band_shares(&spectrogram);
        assert!(shares.iter().all(|share| share[2] > 0.99));
        assert!(
            centroids(&spectrogram)
                .iter()
                .all(|hz| (hz - 1000.0).abs() < 30.0)
        );
        assert!(flatness(&spectrogram).iter().all(|value| *value < 0.01));
    }

    #[test]
    fn white_noise_is_flat_and_bright() {
        let mut rng = tickora_core::rng::Rng::from_seed(3);
        let noise: Vec<f64> = (0..48_000)
            .map(|_| f64::from(rng.next_u32()) / f64::from(u32::MAX) - 0.5)
            .collect();
        let spectrogram = spectrogram(&noise, 48_000).unwrap();
        let flat = flatness(&spectrogram);
        assert!(flat.iter().all(|value| *value > 0.4));
        assert!(centroids(&spectrogram).iter().all(|hz| *hz > 9000.0));
    }
}
