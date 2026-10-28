use std::f64::consts::PI;

use super::fft::{Complex, Fft};
use super::filter::{Coefficients, lfilter_from, state_len};
use super::spectrum::hann_periodic;
use super::stats::round_to;

fn divide(a: Complex, b: Complex) -> Complex {
    let denominator = b.norm_sqr();
    Complex::new(
        (a.re * b.re + a.im * b.im) / denominator,
        (a.im * b.re - a.re * b.im) / denominator,
    )
}

fn sqrt(z: Complex) -> Complex {
    let modulus = z.norm_sqr().sqrt();
    let re = ((modulus + z.re) / 2.0).sqrt();
    let im = ((modulus - z.re) / 2.0).sqrt();
    Complex::new(re, if z.im < 0.0 { -im } else { im })
}

fn poly(roots: &[Complex]) -> Vec<Complex> {
    let mut coefficients = vec![Complex::new(1.0, 0.0)];
    for &root in roots {
        let mut next = vec![Complex::ZERO; coefficients.len() + 1];
        for (i, &c) in coefficients.iter().enumerate() {
            next[i] = next[i] + c;
            next[i + 1] = next[i + 1] - c * root;
        }
        coefficients = next;
    }
    coefficients
}

pub fn butter_bandpass(order: usize, low_hz: f64, high_hz: f64, rate: f64) -> Coefficients {
    let poles: Vec<Complex> = (0..order)
        .map(|i| {
            let m = -(order as f64) + 1.0 + 2.0 * i as f64;
            let angle = PI * m / (2.0 * order as f64);
            Complex::new(-angle.cos(), -angle.sin())
        })
        .collect();
    let fs = 2.0;
    let warp = |hz: f64| 2.0 * fs * (PI * (2.0 * hz / rate) / fs).tan();
    let (low, high) = (warp(low_hz), warp(high_hz));
    let bandwidth = high - low;
    let centre_sq = Complex::new(low * high, 0.0);
    let half = Complex::new(bandwidth / 2.0, 0.0);
    let mut band_poles = Vec::with_capacity(2 * order);
    for sign in [1.0, -1.0] {
        for &pole in &poles {
            let scaled = pole * half;
            let root = sqrt(scaled * scaled - centre_sq);
            band_poles.push(scaled + Complex::new(sign * root.re, sign * root.im));
        }
    }
    let gain = bandwidth.powi(order as i32);
    let fs2 = Complex::new(2.0 * fs, 0.0);
    let z_poles: Vec<Complex> = band_poles
        .iter()
        .map(|&p| divide(fs2 + p, fs2 - p))
        .collect();
    let mut z_zeros = vec![Complex::new(1.0, 0.0); order];
    z_zeros.extend(vec![Complex::new(-1.0, 0.0); order]);
    let numerator = fs2.re.powi(order as i32);
    let denominator = band_poles
        .iter()
        .fold(Complex::new(1.0, 0.0), |product, &p| product * (fs2 - p));
    let z_gain = gain * divide(Complex::new(numerator, 0.0), denominator).re;
    Coefficients {
        b: poly(&z_zeros).iter().map(|c| c.re * z_gain).collect(),
        a: poly(&z_poles).iter().map(|c| c.re).collect(),
    }
}

fn solve(mut matrix: Vec<Vec<f64>>, mut rhs: Vec<f64>) -> Vec<f64> {
    let n = rhs.len();
    for column in 0..n {
        let pivot = (column..n)
            .max_by(|&a, &b| matrix[a][column].abs().total_cmp(&matrix[b][column].abs()))
            .unwrap_or(column);
        matrix.swap(column, pivot);
        rhs.swap(column, pivot);
        let (done, rest) = matrix.split_at_mut(column + 1);
        let pivot_row = &done[column];
        for (offset, row) in rest.iter_mut().enumerate() {
            let factor = row[column] / pivot_row[column];
            for (value, pivot_value) in row[column..].iter_mut().zip(&pivot_row[column..]) {
                *value -= factor * pivot_value;
            }
            rhs[column + 1 + offset] -= factor * rhs[column];
        }
    }
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let sum: f64 = (row + 1..n).map(|k| matrix[row][k] * x[k]).sum();
        x[row] = (rhs[row] - sum) / matrix[row][row];
    }
    x
}

pub fn lfilter_zi(coefficients: &Coefficients) -> Vec<f64> {
    let n = state_len(coefficients);
    let a0 = coefficients.a[0];
    let at = |values: &[f64], i: usize| values.get(i).copied().unwrap_or(0.0) / a0;
    let a: Vec<f64> = (0..=n).map(|i| at(&coefficients.a, i)).collect();
    let b: Vec<f64> = (0..=n).map(|i| at(&coefficients.b, i)).collect();
    let matrix = (0..n)
        .map(|row| {
            (0..n)
                .map(|column| {
                    let identity = if row == column { 1.0 } else { 0.0 };
                    let companion_t = if column == 0 {
                        -a[row + 1]
                    } else if column == row + 1 {
                        1.0
                    } else {
                        0.0
                    };
                    identity - companion_t
                })
                .collect()
        })
        .collect();
    let rhs = (1..=n).map(|i| b[i] - a[i] * b[0]).collect();
    solve(matrix, rhs)
}

pub fn filtfilt(coefficients: &Coefficients, x: &[f64]) -> Vec<f64> {
    let padlen = 3 * coefficients.a.len().max(coefficients.b.len());
    if x.len() <= padlen {
        return x.to_vec();
    }
    let last = x.len() - 1;
    let mut extended: Vec<f64> = (1..=padlen).rev().map(|i| 2.0 * x[0] - x[i]).collect();
    extended.extend_from_slice(x);
    extended.extend((1..=padlen).map(|i| 2.0 * x[last] - x[last - i]));
    let zi = lfilter_zi(coefficients);
    let start: Vec<f64> = zi.iter().map(|z| z * extended[0]).collect();
    let mut forward = lfilter_from(coefficients, &extended, &start);
    forward.reverse();
    let start: Vec<f64> = zi.iter().map(|z| z * forward[0]).collect();
    let mut backward = lfilter_from(coefficients, &forward, &start);
    backward.reverse();
    backward[padlen..backward.len() - padlen].to_vec()
}

pub fn welch_power(x: &[f64], segment: usize) -> Vec<f64> {
    let window = hann_periodic(segment);
    let step = segment - segment / 2;
    let plan = Fft::new(segment);
    let mut sums = vec![0.0; segment / 2 + 1];
    let mut count = 0;
    let mut start = 0;
    while start + segment <= x.len() {
        let piece = &x[start..start + segment];
        let mean = piece.iter().sum::<f64>() / segment as f64;
        let frame: Vec<f64> = piece
            .iter()
            .zip(&window)
            .map(|(value, w)| (value - mean) * w)
            .collect();
        for (sum, bin) in sums.iter_mut().zip(plan.real(&frame)) {
            *sum += bin.norm_sqr();
        }
        count += 1;
        start += step;
    }
    sums.iter().map(|sum| sum / count.max(1) as f64).collect()
}

pub fn voice_likeness(mono: &[f64], rate: u32) -> Option<f64> {
    let band = filtfilt(&butter_bandpass(4, 300.0, 3400.0, f64::from(rate)), mono);
    let hop = (rate / 100) as usize;
    let count = band.len() / hop;
    if count < 400 {
        return None;
    }
    let envelope: Vec<f64> = band[..count * hop]
        .chunks_exact(hop)
        .map(|chunk| (chunk.iter().map(|x| x * x).sum::<f64>() / hop as f64).sqrt())
        .collect();
    let mean = envelope.iter().sum::<f64>() / count as f64;
    let centred: Vec<f64> = envelope.iter().map(|value| value - mean).collect();
    let segment = count.min(512);
    let power = welch_power(&centred, segment);
    let hz = |k: usize| k as f64 * 100.0 / segment as f64;
    let sum = |low: f64, high: f64| -> f64 {
        power
            .iter()
            .enumerate()
            .filter(|(k, _)| hz(*k) >= low && hz(*k) <= high)
            .map(|(_, p)| p)
            .sum()
    };
    let syllable = sum(3.0, 7.0);
    let total = sum(0.5, 20.0) + 1e-20;
    Some(round_to(syllable / total, 2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_band_pass_matches_scipy_coefficients() {
        let first = butter_bandpass(1, 300.0, 3400.0, 48_000.0);
        let b = [0.17062367547157659, 0.0, -0.17062367547157659];
        let a = [1.0, -1.6440765027525206, 0.6587526490568468];
        for (got, want) in first.b.iter().zip(b).chain(first.a.iter().zip(a)) {
            assert!((got - want).abs() < 1e-12, "{got} against {want}");
        }
        let fourth = butter_bandpass(4, 300.0, 3400.0, 48_000.0);
        let b = [
            0.0010495703395201905,
            0.0,
            -0.004198281358080762,
            0.0,
            0.006297422037121143,
            0.0,
            -0.004198281358080762,
            0.0,
            0.0010495703395201905,
        ];
        let a = [
            1.0,
            -6.881508889936636,
            20.804107805434565,
            -36.10749875539666,
            39.36515802101331,
            -27.612137363857613,
            12.17065272505671,
            -3.082112548093927,
            0.3433390632319078,
        ];
        for (got, want) in fourth.b.iter().zip(b).chain(fourth.a.iter().zip(a)) {
            assert!((got - want).abs() < 1e-9, "{got} against {want}");
        }
        let zi = lfilter_zi(&fourth);
        for (got, want) in zi.iter().zip([-0.00104957, -0.00104957, 0.00314871]) {
            assert!((got - want).abs() < 1e-8, "{got} against {want}");
        }
    }

    #[test]
    fn filtfilt_passes_dc_through_a_low_pass_unchanged() {
        let low_pass = Coefficients {
            b: vec![0.5],
            a: vec![1.0, -0.5],
        };
        let out = filtfilt(&low_pass, &[2.0; 50]);
        assert!(out.iter().all(|value| (value - 2.0).abs() < 1e-12));
    }

    #[test]
    fn syllable_rate_bursts_score_high_and_a_steady_tone_low() {
        let burst = |i: usize| {
            let t = i as f64 / 48_000.0;
            let gate = if (t * 5.0).fract() < 0.5 { 1.0 } else { 0.0 };
            gate * (2.0 * PI * 1000.0 * t).sin()
        };
        let voiced: Vec<f64> = (0..48_000 * 5).map(burst).collect();
        assert!(voice_likeness(&voiced, 48_000).unwrap() >= 0.35);
        let steady: Vec<f64> = (0..48_000 * 5)
            .map(|i| (2.0 * PI * 1000.0 * i as f64 / 48_000.0).sin())
            .collect();
        assert!(voice_likeness(&steady, 48_000).unwrap() < 0.35);
        assert_eq!(voice_likeness(&steady[..48_000], 48_000), None);
    }
}
