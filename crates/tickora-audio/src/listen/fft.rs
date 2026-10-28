use std::f64::consts::PI;
use std::ops::{Add, Mul, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub const ZERO: Complex = Complex { re: 0.0, im: 0.0 };

    pub fn new(re: f64, im: f64) -> Self {
        Complex { re, im }
    }

    pub fn norm_sqr(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
}

impl Add for Complex {
    type Output = Complex;

    fn add(self, other: Complex) -> Complex {
        Complex::new(self.re + other.re, self.im + other.im)
    }
}

impl Sub for Complex {
    type Output = Complex;

    fn sub(self, other: Complex) -> Complex {
        Complex::new(self.re - other.re, self.im - other.im)
    }
}

impl Mul for Complex {
    type Output = Complex;

    fn mul(self, other: Complex) -> Complex {
        Complex::new(
            self.re * other.re - self.im * other.im,
            self.re * other.im + self.im * other.re,
        )
    }
}

pub struct Fft {
    size: usize,
    twiddles: Vec<Complex>,
}

impl Fft {
    pub fn new(size: usize) -> Self {
        let twiddles = (0..size)
            .map(|j| {
                let angle = -2.0 * PI * j as f64 / size as f64;
                Complex::new(angle.cos(), angle.sin())
            })
            .collect();
        Fft { size, twiddles }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn transform(&self, input: &[Complex]) -> Vec<Complex> {
        assert_eq!(input.len(), self.size, "input length must match the plan");
        if self.size == 0 {
            return Vec::new();
        }
        self.recurse(input, 1)
    }

    pub fn real(&self, input: &[f64]) -> Vec<Complex> {
        let complex: Vec<Complex> = input.iter().map(|&x| Complex::new(x, 0.0)).collect();
        let mut out = self.transform(&complex);
        out.truncate(self.size / 2 + 1);
        out
    }

    fn recurse(&self, input: &[Complex], stride: usize) -> Vec<Complex> {
        let n = input.len();
        if n == 1 {
            return vec![input[0]];
        }
        let radix = smallest_factor(n);
        let part = n / radix;
        let parts: Vec<Vec<Complex>> = (0..radix)
            .map(|r| {
                let sub: Vec<Complex> = (0..part).map(|m| input[m * radix + r]).collect();
                self.recurse(&sub, stride * radix)
            })
            .collect();
        (0..n)
            .map(|k| {
                parts
                    .iter()
                    .enumerate()
                    .fold(Complex::ZERO, |sum, (r, values)| {
                        sum + values[k % part] * self.twiddles[(r * k % n) * stride]
                    })
            })
            .collect()
    }
}

fn smallest_factor(n: usize) -> usize {
    if n.is_multiple_of(2) {
        return 2;
    }
    let mut factor = 3;
    while factor * factor <= n {
        if n.is_multiple_of(factor) {
            return factor;
        }
        factor += 2;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    fn direct(input: &[Complex]) -> Vec<Complex> {
        let n = input.len();
        (0..n)
            .map(|k| {
                input
                    .iter()
                    .enumerate()
                    .fold(Complex::ZERO, |sum, (j, &x)| {
                        let angle = -2.0 * PI * (j * k % n) as f64 / n as f64;
                        sum + x * Complex::new(angle.cos(), angle.sin())
                    })
            })
            .collect()
    }

    #[test]
    fn matches_the_direct_transform_for_mixed_and_prime_sizes() {
        for size in [1, 2, 8, 12, 75, 97, 128, 360] {
            let input: Vec<Complex> = (0..size)
                .map(|i| Complex::new((i as f64 * 0.37).sin(), (i as f64 * 0.11).cos()))
                .collect();
            let fast = Fft::new(size).transform(&input);
            for (a, b) in fast.iter().zip(direct(&input)) {
                assert!(
                    (a.re - b.re).abs() < 1e-9 && (a.im - b.im).abs() < 1e-9,
                    "size {size}"
                );
            }
        }
    }

    #[test]
    fn a_cosine_lands_in_its_bin() {
        let size = 64;
        let input: Vec<f64> = (0..size)
            .map(|i| (2.0 * PI * 5.0 * i as f64 / size as f64).cos())
            .collect();
        let out = Fft::new(size).real(&input);
        assert_eq!(out.len(), 33);
        assert!((out[5].re - 32.0).abs() < 1e-9);
        assert!(
            out.iter()
                .enumerate()
                .all(|(k, c)| k == 5 || c.norm_sqr() < 1e-18)
        );
    }
}
