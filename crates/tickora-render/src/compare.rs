use crate::image::Image;

pub const CHUNK: u32 = 16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerance {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
    pub max_local_error: f64,
    pub max_global_error: f64,
    pub ignore_aa: bool,
}

impl Tolerance {
    pub const ZERO: Tolerance = Tolerance {
        red: 0,
        green: 0,
        blue: 0,
        alpha: 0,
        max_local_error: 0.0,
        max_global_error: 0.0,
        ignore_aa: false,
    };

    pub const LOW: Tolerance = Tolerance::unreal(16);
    pub const MEDIUM: Tolerance = Tolerance::unreal(24);
    pub const HIGH: Tolerance = Tolerance::unreal(32);

    const fn unreal(channel: u8) -> Tolerance {
        Tolerance {
            red: channel,
            green: channel,
            blue: channel,
            alpha: channel,
            max_local_error: 0.10,
            max_global_error: 0.02,
            ignore_aa: true,
        }
    }

    pub fn preset(name: &str) -> Result<Tolerance, String> {
        match name {
            "zero" => Ok(Tolerance::ZERO),
            "low" => Ok(Tolerance::LOW),
            "medium" => Ok(Tolerance::MEDIUM),
            "high" => Ok(Tolerance::HIGH),
            other => Err(format!(
                "unknown tolerance preset {other:?}; expected \"zero\", \"low\", \"medium\" or \"high\""
            )),
        }
    }

    fn channels_match(&self, a: [u8; 4], b: [u8; 4]) -> bool {
        let limits = [self.red, self.green, self.blue, self.alpha];
        (0..4).all(|i| a[i].abs_diff(b[i]) <= limits[i])
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    pub passed: bool,
    pub differing_pixels: u64,
    pub global_error: f64,
    pub max_local_error: f64,
    pub max_channel_delta: u8,
}

pub fn compare(
    actual: &Image,
    golden: &Image,
    tolerance: &Tolerance,
) -> Result<Comparison, String> {
    if (actual.width, actual.height) != (golden.width, golden.height) {
        return Err(format!(
            "image is {}x{} but the golden is {}x{}",
            actual.width, actual.height, golden.width, golden.height
        ));
    }
    let (width, height) = (actual.width, actual.height);
    let chunks_x = width.div_ceil(CHUNK);
    let chunks_y = height.div_ceil(CHUNK);
    let mut chunk_diffs = vec![0u32; (chunks_x * chunks_y) as usize];
    let mut differing_pixels = 0u64;
    let mut max_channel_delta = 0u8;
    for y in 0..height {
        for x in 0..width {
            let (a, b) = (actual.pixel(x, y), golden.pixel(x, y));
            for i in 0..4 {
                max_channel_delta = max_channel_delta.max(a[i].abs_diff(b[i]));
            }
            if tolerance.channels_match(a, b)
                || (tolerance.ignore_aa && neighbour_matches(a, golden, x, y, tolerance))
            {
                continue;
            }
            differing_pixels += 1;
            chunk_diffs[((y / CHUNK) * chunks_x + x / CHUNK) as usize] += 1;
        }
    }
    let mut max_local_error = 0.0f64;
    for cy in 0..chunks_y {
        for cx in 0..chunks_x {
            let w = CHUNK.min(width - cx * CHUNK);
            let h = CHUNK.min(height - cy * CHUNK);
            let share = f64::from(chunk_diffs[(cy * chunks_x + cx) as usize]) / f64::from(w * h);
            max_local_error = max_local_error.max(share);
        }
    }
    let global_error = differing_pixels as f64 / (f64::from(width) * f64::from(height));
    Ok(Comparison {
        passed: global_error <= tolerance.max_global_error
            && max_local_error <= tolerance.max_local_error,
        differing_pixels,
        global_error,
        max_local_error,
        max_channel_delta,
    })
}

fn neighbour_matches(
    pixel: [u8; 4],
    golden: &Image,
    x: u32,
    y: u32,
    tolerance: &Tolerance,
) -> bool {
    for dy in -1i64..=1 {
        for dx in -1i64..=1 {
            let (nx, ny) = (i64::from(x) + dx, i64::from(y) + dy);
            let inside = (0..i64::from(golden.width)).contains(&nx)
                && (0..i64::from(golden.height)).contains(&ny);
            if (dx, dy) != (0, 0)
                && inside
                && tolerance.channels_match(pixel, golden.pixel(nx as u32, ny as u32))
            {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLACK: [u8; 4] = [0, 0, 0, 255];
    const WHITE: [u8; 4] = [255, 255, 255, 255];

    fn filled(width: u32, height: u32, color: [u8; 4]) -> Image {
        Image::new(width, height, color.repeat((width * height) as usize)).unwrap()
    }

    fn set(image: &mut Image, x: u32, y: u32, color: [u8; 4]) {
        let at = ((y * image.width + x) * 4) as usize;
        image.pixels[at..at + 4].copy_from_slice(&color);
    }

    fn custom(channel: u8, local: f64, global: f64, ignore_aa: bool) -> Tolerance {
        Tolerance {
            red: channel,
            green: channel,
            blue: channel,
            alpha: channel,
            max_local_error: local,
            max_global_error: global,
            ignore_aa,
        }
    }

    #[test]
    fn identical_images_pass_at_zero() {
        let image = filled(32, 32, BLACK);
        let result = compare(&image, &image, &Tolerance::ZERO).unwrap();
        assert!(result.passed);
        assert_eq!(result.differing_pixels, 0);
        assert_eq!(result.max_channel_delta, 0);
    }

    #[test]
    fn one_pixel_fails_zero_and_reports_its_share() {
        let golden = filled(32, 32, BLACK);
        let mut actual = golden.clone();
        set(&mut actual, 5, 5, [3, 0, 0, 255]);
        let result = compare(&actual, &golden, &Tolerance::ZERO).unwrap();
        assert!(!result.passed);
        assert_eq!(result.differing_pixels, 1);
        assert_eq!(result.global_error, 1.0 / 1024.0);
        assert_eq!(result.max_local_error, 1.0 / 256.0);
        assert_eq!(result.max_channel_delta, 3);
    }

    #[test]
    fn channel_tolerance_absorbs_small_deltas() {
        let golden = filled(16, 16, BLACK);
        let mut actual = golden.clone();
        set(&mut actual, 0, 0, [3, 3, 3, 255]);
        assert!(
            compare(&actual, &golden, &custom(3, 0.0, 0.0, false))
                .unwrap()
                .passed
        );
        assert!(
            !compare(&actual, &golden, &custom(2, 0.0, 0.0, false))
                .unwrap()
                .passed
        );
    }

    #[test]
    fn local_error_catches_a_hot_spot_the_global_share_hides() {
        let golden = filled(64, 64, BLACK);
        let mut actual = golden.clone();
        for y in 0..8 {
            for x in 0..8 {
                set(&mut actual, x, y, WHITE);
            }
        }
        let result = compare(&actual, &golden, &custom(0, 0.2, 0.02, false)).unwrap();
        assert_eq!(result.global_error, 64.0 / 4096.0);
        assert_eq!(result.max_local_error, 0.25);
        assert!(!result.passed);
        assert!(
            compare(&actual, &golden, &custom(0, 0.25, 0.02, false))
                .unwrap()
                .passed
        );
    }

    #[test]
    fn edge_chunks_use_their_real_size() {
        let golden = filled(17, 16, BLACK);
        let mut actual = golden.clone();
        set(&mut actual, 16, 0, WHITE);
        let result = compare(&actual, &golden, &Tolerance::ZERO).unwrap();
        assert_eq!(result.max_local_error, 1.0 / 16.0);
    }

    #[test]
    fn ignore_aa_accepts_a_one_pixel_shift() {
        let mut golden = filled(16, 16, BLACK);
        let mut actual = golden.clone();
        for y in 4..8 {
            set(&mut golden, 4, y, WHITE);
            set(&mut actual, 5, y, WHITE);
        }
        assert!(
            !compare(&actual, &golden, &custom(0, 0.0, 0.0, false))
                .unwrap()
                .passed
        );
        assert!(
            compare(&actual, &golden, &custom(0, 0.0, 0.0, true))
                .unwrap()
                .passed
        );
    }

    #[test]
    fn ignore_aa_does_not_accept_a_new_colour() {
        let golden = filled(16, 16, BLACK);
        let mut actual = golden.clone();
        set(&mut actual, 8, 8, WHITE);
        assert!(
            !compare(&actual, &golden, &custom(0, 0.0, 0.0, true))
                .unwrap()
                .passed
        );
    }

    #[test]
    fn size_mismatch_is_an_error() {
        assert!(
            compare(
                &filled(16, 16, BLACK),
                &filled(16, 17, BLACK),
                &Tolerance::ZERO
            )
            .is_err()
        );
    }

    #[test]
    fn presets_parse() {
        assert_eq!(Tolerance::preset("zero").unwrap(), Tolerance::ZERO);
        assert_eq!(Tolerance::preset("low").unwrap(), Tolerance::LOW);
        assert_eq!(Tolerance::preset("medium").unwrap(), Tolerance::MEDIUM);
        assert_eq!(Tolerance::preset("high").unwrap(), Tolerance::HIGH);
        assert!(Tolerance::preset("huge").is_err());
    }

    #[test]
    fn presets_follow_unreal_automation_screenshot_options() {
        for (preset, channel) in [
            (Tolerance::LOW, 16),
            (Tolerance::MEDIUM, 24),
            (Tolerance::HIGH, 32),
        ] {
            assert_eq!(preset, custom(channel, 0.10, 0.02, true));
        }
    }

    #[test]
    fn low_absorbs_shading_rounding_and_rejects_a_wrong_colour() {
        let golden = filled(32, 32, [100, 100, 100, 255]);
        let mut rounded = golden.clone();
        for x in 0..32 {
            set(&mut rounded, x, 3, [101, 99, 101, 255]);
        }
        assert!(compare(&rounded, &golden, &Tolerance::LOW).unwrap().passed);
        let mut wrong = golden.clone();
        for y in 0..32 {
            for x in 0..32 {
                set(&mut wrong, x, y, [140, 100, 100, 255]);
            }
        }
        assert!(!compare(&wrong, &golden, &Tolerance::HIGH).unwrap().passed);
    }
}
