use crate::light::ShadowQuality;

pub const CONTENT_GAMMA: f64 = 2.2;
pub const DEFAULT_GAMMA: f64 = 2.2;
pub const DEFAULT_EXPOSURE: f64 = 1.0;
pub const DEFAULT_WHITE_POINT: f64 = 11.2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Display {
    pub gamma: f32,
    pub shadow_quality: ShadowQuality,
}

impl Default for Display {
    fn default() -> Self {
        Display {
            gamma: DEFAULT_GAMMA as f32,
            shadow_quality: ShadowQuality::default(),
        }
    }
}

const A: f64 = 0.15;
const B: f64 = 0.50;
const C: f64 = 0.10;
const D: f64 = 0.20;
const E: f64 = 0.02;
const F: f64 = 0.30;

pub fn decode(encoded: f64) -> f64 {
    encoded.max(0.0).powf(CONTENT_GAMMA)
}

pub fn decode_byte(byte: u8) -> f64 {
    decode(f64::from(byte) / 255.0)
}

pub fn hable(x: f64) -> f64 {
    (x * (A * x + C * B) + D * E) / (x * (A * x + B) + D * F) - E / F
}

pub fn tone_map(linear: f64, exposure: f64, white_point: f64) -> f64 {
    (hable(linear * exposure) / hable(white_point)).clamp(0.0, 1.0)
}

pub fn encode(linear: f64, gamma: f64) -> f64 {
    linear.clamp(0.0, 1.0).powf(1.0 / gamma)
}

pub fn encode_byte(linear: f64, gamma: f64) -> u8 {
    (encode(linear, gamma) * 255.0).round() as u8
}

pub fn check_positive(name: &str, value: f64) -> Result<(), String> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(format!("{name} must be finite and above 0, got {value}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hable_starts_at_zero() {
        assert!(hable(0.0).abs() < 1e-15);
    }

    #[test]
    fn white_point_maps_to_one() {
        assert_eq!(tone_map(DEFAULT_WHITE_POINT, 1.0, DEFAULT_WHITE_POINT), 1.0);
        assert_eq!(tone_map(4.0, 1.0, 4.0), 1.0);
    }

    #[test]
    fn curve_rises_up_to_the_white_point() {
        let mut previous = tone_map(0.0, 1.0, DEFAULT_WHITE_POINT);
        for step in 1..=6400 {
            let next = tone_map(f64::from(step) / 100.0, 1.0, DEFAULT_WHITE_POINT);
            assert!(next >= previous, "{step}");
            previous = next;
        }
        assert_eq!(previous, 1.0);
    }

    #[test]
    fn exposure_scales_the_input() {
        for x in [0.01, 0.25, 1.0, 3.0] {
            assert_eq!(
                tone_map(x, 2.0, DEFAULT_WHITE_POINT),
                tone_map(x * 2.0, 1.0, DEFAULT_WHITE_POINT)
            );
        }
    }

    #[test]
    fn every_byte_round_trips_at_the_default_gamma() {
        for byte in 0..=255u8 {
            assert_eq!(encode_byte(decode_byte(byte), DEFAULT_GAMMA), byte);
        }
    }

    #[test]
    fn defaults_give_the_design_values() {
        let byte = |x: f64| encode_byte(tone_map(x, 1.0, DEFAULT_WHITE_POINT), DEFAULT_GAMMA);
        assert_eq!(byte(0.05), 42);
        assert_eq!(byte(0.25), 86);
        assert_eq!(byte(0.5), 115);
        assert_eq!(byte(1.0), 148);
        assert_eq!(byte(4.0), 219);
    }

    #[test]
    fn encode_clamps_out_of_range_values() {
        assert_eq!(encode_byte(-1.0, DEFAULT_GAMMA), 0);
        assert_eq!(encode_byte(7.0, DEFAULT_GAMMA), 255);
    }

    #[test]
    fn only_finite_positive_values_pass() {
        assert!(check_positive("gamma", 2.2).is_ok());
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(check_positive("gamma", bad).is_err());
        }
    }
}
