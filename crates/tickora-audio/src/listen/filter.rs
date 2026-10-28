use std::f64::consts::PI;

pub struct Coefficients {
    pub b: Vec<f64>,
    pub a: Vec<f64>,
}

pub enum Biquad {
    HighShelf { gain_db: f64 },
    HighPass,
}

pub fn biquad(kind: Biquad, fc: f64, rate: f64, q: f64) -> Coefficients {
    let w0 = 2.0 * PI * fc / rate;
    let alpha = w0.sin() / (2.0 * q);
    let cos = w0.cos();
    let (b, den) = match kind {
        Biquad::HighShelf { gain_db } => {
            let a = 10f64.powf(gain_db / 40.0);
            let root = 2.0 * a.sqrt() * alpha;
            (
                [
                    a * ((a + 1.0) + (a - 1.0) * cos + root),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                    a * ((a + 1.0) + (a - 1.0) * cos - root),
                ],
                [
                    (a + 1.0) - (a - 1.0) * cos + root,
                    2.0 * ((a - 1.0) - (a + 1.0) * cos),
                    (a + 1.0) - (a - 1.0) * cos - root,
                ],
            )
        }
        Biquad::HighPass => (
            [(1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0],
            [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
        ),
    };
    Coefficients {
        b: b.iter().map(|value| value / den[0]).collect(),
        a: den.iter().map(|value| value / den[0]).collect(),
    }
}

pub fn lfilter(coefficients: &Coefficients, input: &[f64]) -> Vec<f64> {
    lfilter_from(coefficients, input, &vec![0.0; state_len(coefficients)])
}

pub fn state_len(coefficients: &Coefficients) -> usize {
    coefficients.a.len().max(coefficients.b.len()) - 1
}

pub fn lfilter_from(coefficients: &Coefficients, input: &[f64], initial: &[f64]) -> Vec<f64> {
    let order = state_len(coefficients);
    let at = |values: &[f64], i: usize| values.get(i).copied().unwrap_or(0.0) / coefficients.a[0];
    let b: Vec<f64> = (0..=order).map(|i| at(&coefficients.b, i)).collect();
    let a: Vec<f64> = (0..=order).map(|i| at(&coefficients.a, i)).collect();
    let mut state = initial.to_vec();
    input
        .iter()
        .map(|&x| {
            let y = b[0] * x + state.first().copied().unwrap_or(0.0);
            for i in 0..order {
                let next = state.get(i + 1).copied().unwrap_or(0.0);
                state[i] = b[i + 1] * x + next - a[i + 1] * y;
            }
            y
        })
        .collect()
}

pub fn k_weight(channel: &[f64], rate: f64) -> Vec<f64> {
    let shelf = biquad(Biquad::HighShelf { gain_db: 4.0 }, 1681.97, rate, 0.7072);
    let pass = biquad(Biquad::HighPass, 38.14, rate, 0.5003);
    lfilter(&pass, &lfilter(&shelf, channel))
}

pub fn bessel_i0(x: f64) -> f64 {
    let quarter = x * x / 4.0;
    let mut term = 1.0;
    let mut sum = 1.0;
    let mut k = 1.0;
    while term > sum * 1e-17 {
        term *= quarter / (k * k);
        sum += term;
        k += 1.0;
    }
    sum
}

pub fn upsampling_filter(up: usize) -> Vec<f64> {
    let half = 10 * up;
    let taps = 2 * half + 1;
    let cutoff = 1.0 / up as f64;
    let alpha = (taps - 1) as f64 / 2.0;
    let beta = 5.0;
    let mut h: Vec<f64> = (0..taps)
        .map(|n| {
            let m = n as f64 - alpha;
            let ratio = m / alpha;
            let window = bessel_i0(beta * (1.0 - ratio * ratio).max(0.0).sqrt()) / bessel_i0(beta);
            cutoff * sinc(cutoff * m) * window
        })
        .collect();
    let sum: f64 = h.iter().sum();
    for value in &mut h {
        *value = *value / sum * up as f64;
    }
    h
}

fn sinc(x: f64) -> f64 {
    if x == 0.0 {
        1.0
    } else {
        (PI * x).sin() / (PI * x)
    }
}

pub fn upsampled(channel: &[f64], up: usize, h: &[f64], j: usize) -> f64 {
    let half = (h.len() - 1) / 2;
    let shifted = j + half;
    let mut k = shifted % up;
    let mut sum = 0.0;
    while k < h.len() {
        if let Some(&x) = channel.get((shifted - k) / up) {
            sum += h[k] * x;
        }
        if shifted < k + up {
            break;
        }
        k += up;
    }
    sum
}

pub fn upsampled_peak(channel: &[f64], up: usize, h: &[f64]) -> f64 {
    (0..channel.len() * up)
        .map(|j| upsampled(channel, up, h, j).abs())
        .fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lfilter_runs_a_one_pole_recursion() {
        let coefficients = Coefficients {
            b: vec![1.0],
            a: vec![1.0, -0.5],
        };
        assert_eq!(
            lfilter(&coefficients, &[1.0, 0.0, 0.0]),
            vec![1.0, 0.5, 0.25]
        );
    }

    #[test]
    fn k_weighting_leaves_1_khz_near_its_level_and_cuts_20_hz() {
        let rate = 48_000.0;
        let gain = |hz: f64| {
            let tone: Vec<f64> = (0..48_000)
                .map(|i| (2.0 * PI * hz * i as f64 / rate).sin())
                .collect();
            let out = k_weight(&tone, rate);
            let rms = |x: &[f64]| (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64).sqrt();
            20.0 * (rms(&out[24_000..]) / rms(&tone[24_000..])).log10()
        };
        assert!((gain(1000.0) - 0.438).abs() < 0.005);
        assert!(gain(20.0) < -10.0);
    }

    #[test]
    fn the_upsampler_keeps_dc_and_interpolates_a_slow_sine() {
        let h = upsampling_filter(4);
        assert_eq!(h.len(), 81);
        assert!((h.iter().sum::<f64>() - 4.0).abs() < 1e-12);
        let tone: Vec<f64> = (0..400)
            .map(|i| (2.0 * PI * i as f64 / 100.0).sin())
            .collect();
        for j in 200..1400 {
            let expected = (2.0 * PI * j as f64 / 400.0).sin();
            assert!(
                (upsampled(&tone, 4, &h, j) - expected).abs() < 1e-3,
                "sample {j}"
            );
        }
        assert!((upsampled_peak(&tone, 4, &h) - 1.0).abs() < 1e-3);
        let direct = |j: usize| -> f64 {
            (0..81)
                .filter_map(|k| {
                    let index = (j + 40).checked_sub(k)?;
                    (index < 1600 && index % 4 == 0).then(|| h[k] * tone[index / 4])
                })
                .sum()
        };
        for j in [0, 1, 2, 3, 39, 40, 41, 1597, 1598, 1599] {
            assert!(
                (upsampled(&tone, 4, &h, j) - direct(j)).abs() < 1e-15,
                "sample {j}"
            );
        }
    }
}
