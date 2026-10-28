use std::collections::BTreeMap;

use super::Audio;
use super::loudness::block_loudness;
use super::spectrum::{SEGMENT, centroids, spectrogram};

pub const DEFAULT_WIDTH: u32 = 1000;
pub const DEFAULT_HEIGHT: u32 = 500;
pub const MIN_SIZE: [u32; 2] = [100, 50];
pub const MAX_SIZE: u32 = 4096;

const LOW_HZ: f64 = 20.0;
const HIGH_HZ: f64 = 20_000.0;

const MAGMA: [[u8; 3]; 17] = [
    [0, 0, 4],
    [10, 8, 34],
    [29, 17, 71],
    [54, 16, 107],
    [81, 18, 124],
    [106, 28, 129],
    [131, 38, 129],
    [156, 46, 127],
    [183, 55, 121],
    [208, 65, 111],
    [231, 82, 99],
    [245, 107, 92],
    [252, 137, 97],
    [254, 167, 114],
    [254, 196, 136],
    [253, 226, 163],
    [252, 253, 191],
];

const TAB20: [[u8; 3]; 20] = [
    [0x1f, 0x77, 0xb4],
    [0xae, 0xc7, 0xe8],
    [0xff, 0x7f, 0x0e],
    [0xff, 0xbb, 0x78],
    [0x2c, 0xa0, 0x2c],
    [0x98, 0xdf, 0x8a],
    [0xd6, 0x27, 0x28],
    [0xff, 0x98, 0x96],
    [0x94, 0x67, 0xbd],
    [0xc5, 0xb0, 0xd5],
    [0x8c, 0x56, 0x4b],
    [0xc4, 0x9c, 0x94],
    [0xe3, 0x77, 0xc2],
    [0xf7, 0xb6, 0xd2],
    [0x7f, 0x7f, 0x7f],
    [0xc7, 0xc7, 0xc7],
    [0xbc, 0xbd, 0x22],
    [0xdb, 0xdb, 0x8d],
    [0x17, 0xbe, 0xcf],
    [0x9e, 0xda, 0xe5],
];

const LOUDNESS: [u8; 3] = [0x1f, 0x77, 0xb4];
const CENTROID: [u8; 3] = [0x2c, 0xa0, 0x2c];
const ONSET: [u8; 3] = [0x00, 0xff, 0xff];
const STRIP: [u8; 3] = [16, 16, 16];
const DIVIDER: [u8; 3] = [96, 96, 96];
const DASH: u32 = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub time_s: f64,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Raster {
    fn new(width: u32, height: u32, fill: [u8; 3]) -> Self {
        let pixels = (0..width * height)
            .flat_map(|_| [fill[0], fill[1], fill[2], 255])
            .collect();
        Raster {
            width,
            height,
            pixels,
        }
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 3] {
        let at = ((y * self.width + x) * 4) as usize;
        [self.pixels[at], self.pixels[at + 1], self.pixels[at + 2]]
    }

    fn set(&mut self, x: u32, y: u32, colour: [u8; 3]) {
        if x < self.width && y < self.height {
            let at = ((y * self.width + x) * 4) as usize;
            self.pixels[at..at + 3].copy_from_slice(&colour);
        }
    }

    fn span(&mut self, x: u32, from: u32, to: u32, colour: [u8; 3]) {
        for y in from.min(to)..=from.max(to) {
            self.set(x, y, colour);
        }
    }

    fn row(&mut self, y: u32, colour: [u8; 3]) {
        for x in 0..self.width {
            self.set(x, y, colour);
        }
    }
}

pub fn ramp(t: f64) -> [u8; 3] {
    let scaled = t.clamp(0.0, 1.0) * (MAGMA.len() - 1) as f64;
    let index = (scaled.floor() as usize).min(MAGMA.len() - 2);
    let frac = scaled - index as f64;
    let [low, high] = [MAGMA[index], MAGMA[index + 1]];
    std::array::from_fn(|c| {
        (f64::from(low[c]) + (f64::from(high[c]) - f64::from(low[c])) * frac).round() as u8
    })
}

pub fn mark_colours(marks: &[Mark]) -> BTreeMap<String, [u8; 3]> {
    let mut names: Vec<&str> = marks.iter().map(|mark| mark.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    names
        .into_iter()
        .enumerate()
        .map(|(index, name)| (name.to_string(), TAB20[index % TAB20.len()]))
        .collect()
}

pub fn hex(colour: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", colour[0], colour[1], colour[2])
}

fn nearest(times: &[f64], t: f64) -> usize {
    let after = times.partition_point(|&time| time < t);
    if after == 0 {
        0
    } else if after == times.len() || t - times[after - 1] <= times[after] - t {
        after - 1
    } else {
        after
    }
}

fn interpolate(times: &[f64], values: &[f64], t: f64) -> Option<f64> {
    let (first, last) = (*times.first()?, *times.last()?);
    if t < first || t > last {
        return None;
    }
    let after = times.partition_point(|&time| time < t);
    if after == 0 || times[after] == t {
        return Some(values[after]);
    }
    let (t0, t1) = (times[after - 1], times[after]);
    let frac = (t - t0) / (t1 - t0);
    Some(values[after - 1] + (values[after] - values[after - 1]) * frac)
}

fn plot(raster: &mut Raster, top: u32, rows: u32, values: &[Option<f64>], colour: [u8; 3]) {
    let (low, high) = values
        .iter()
        .flatten()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), &v| {
            (low.min(v), high.max(v))
        });
    if !low.is_finite() {
        return;
    }
    let (low, high) = if high - low < 1e-9 {
        (low - 1.0, high + 1.0)
    } else {
        (low, high)
    };
    let usable = f64::from(rows.saturating_sub(3));
    let mut previous = None;
    for (x, value) in values.iter().enumerate() {
        let Some(value) = value else {
            previous = None;
            continue;
        };
        let y = top + 1 + ((1.0 - (value - low) / (high - low)) * usable).round() as u32;
        raster.span(x as u32, previous.unwrap_or(y), y, colour);
        previous = Some(y);
    }
}

pub fn sheet(
    audio: &Audio,
    onset_times: &[f64],
    marks: &[Mark],
    width: u32,
    height: u32,
) -> Result<Raster, String> {
    if width < MIN_SIZE[0] || height < MIN_SIZE[1] || width > MAX_SIZE || height > MAX_SIZE {
        return Err(format!(
            "the sheet must be from {} by {} to {MAX_SIZE} by {MAX_SIZE} pixels, got {width} by {height}",
            MIN_SIZE[0], MIN_SIZE[1]
        ));
    }
    let rate = f64::from(audio.rate);
    let spectrum = spectrogram(&audio.mono(), audio.rate)?;
    let duration = audio.duration();
    let spectrum_rows = height * 3 / 5;
    let strip_rows = height / 5;
    let centroid_top = spectrum_rows + strip_rows;
    let time_at = |x: u32| (f64::from(x) + 0.5) / f64::from(width) * duration;
    let x_of = |t: f64| ((t / duration * f64::from(width)).floor().max(0.0) as u32).min(width - 1);

    let bins: Vec<usize> = (0..spectrum_rows)
        .map(|y| {
            let hz = LOW_HZ
                * (HIGH_HZ / LOW_HZ).powf(1.0 - (f64::from(y) + 0.5) / f64::from(spectrum_rows));
            ((hz * SEGMENT as f64 / rate).round() as usize).min(spectrum.freqs.len() - 1)
        })
        .collect();
    let frames: Vec<usize> = (0..width)
        .map(|x| nearest(&spectrum.times, time_at(x)))
        .collect();
    let decibels: Vec<Vec<f64>> = frames
        .iter()
        .map(|&frame| {
            bins.iter()
                .map(|&bin| 10.0 * (spectrum.power[frame][bin] + 1e-12).log10())
                .collect()
        })
        .collect();
    let (low, high) = decibels
        .iter()
        .flatten()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), &v| {
            (low.min(v), high.max(v))
        });

    let mut raster = Raster::new(width, height, STRIP);
    for (x, column) in decibels.iter().enumerate() {
        for (y, value) in column.iter().enumerate() {
            let t = if high - low > 1e-9 {
                (value - low) / (high - low)
            } else {
                0.0
            };
            raster.set(x as u32, y as u32, ramp(t));
        }
    }

    let blocks = block_loudness(&audio.channels, audio.rate, 3.0, 0.1);
    let block_times: Vec<f64> = blocks.times.iter().map(|t| t + 1.5).collect();
    let loudness: Vec<Option<f64>> = (0..width)
        .map(|x| interpolate(&block_times, &blocks.levels, time_at(x)))
        .collect();
    plot(&mut raster, spectrum_rows, strip_rows, &loudness, LOUDNESS);

    let centres = centroids(&spectrum);
    let centroid_rows = height - centroid_top;
    let centroid: Vec<Option<f64>> = frames
        .iter()
        .map(|&frame| Some(centres[frame].clamp(LOW_HZ, HIGH_HZ).log10()))
        .collect();
    plot(
        &mut raster,
        centroid_top,
        centroid_rows,
        &centroid,
        CENTROID,
    );

    raster.row(spectrum_rows, DIVIDER);
    raster.row(centroid_top, DIVIDER);

    for &time in onset_times {
        raster.span(x_of(time), 0, spectrum_rows / 10, ONSET);
    }
    let colours = mark_colours(marks);
    for mark in marks {
        let x = x_of(mark.time_s);
        let colour = colours[&mark.name];
        for y in (0..height).filter(|y| (y / DASH).is_multiple_of(2)) {
            raster.set(x, y, colour);
            raster.set(x + 1, y, colour);
        }
    }
    Ok(raster)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn tone(hz: f64, seconds: f64) -> Audio {
        let mono: Vec<f64> = (0..(seconds * 48_000.0) as usize)
            .map(|i| 0.1 * (2.0 * PI * hz * i as f64 / 48_000.0).sin())
            .collect();
        Audio::new(48_000, [mono.clone(), mono])
    }

    fn brightness(colour: [u8; 3]) -> u32 {
        colour.iter().map(|&c| u32::from(c)).sum()
    }

    #[test]
    fn colours_follow_listen_py_by_sorted_name() {
        let marks = [
            Mark {
                time_s: 1.0,
                name: "spawn".to_string(),
            },
            Mark {
                time_s: 2.0,
                name: "hit".to_string(),
            },
            Mark {
                time_s: 3.0,
                name: "spawn".to_string(),
            },
        ];
        let colours = mark_colours(&marks);
        assert_eq!(hex(colours["hit"]), "#1f77b4");
        assert_eq!(hex(colours["spawn"]), "#aec7e8");
        assert_eq!(ramp(0.0), [0, 0, 4]);
        assert_eq!(ramp(1.0), [252, 253, 191]);
        assert_eq!(ramp(0.5), [183, 55, 121]);
    }

    #[test]
    fn a_tone_is_brightest_on_its_row_and_marks_carry_their_colour() {
        let marks = [Mark {
            time_s: 1.0,
            name: "spawn".to_string(),
        }];
        let raster = sheet(&tone(1000.0, 5.0), &[2.0], &marks, 300, 100).unwrap();
        assert_eq!(raster.pixels.len(), 300 * 100 * 4);
        let x = 50;
        let brightest = (0..60)
            .max_by_key(|&y| brightness(raster.pixel(x, y)))
            .unwrap();
        let expected = 60.0 * (1.0 - (1000f64 / LOW_HZ).log10() / (HIGH_HZ / LOW_HZ).log10());
        assert!((f64::from(brightest) - expected).abs() <= 1.5);
        let spawn = [0x1f, 0x77, 0xb4];
        assert_eq!(raster.pixel(60, 0), spawn);
        assert_eq!(raster.pixel(61, 10), spawn);
        assert_ne!(raster.pixel(60, 5), spawn);
        assert_eq!(raster.pixel(120, 0), ONSET);
        assert_eq!(raster.pixel(0, 60), DIVIDER);
        assert_eq!(raster.pixel(0, 80), DIVIDER);
        assert!((61..80).any(|y| raster.pixel(150, y) == LOUDNESS));
        assert!((81..100).any(|y| raster.pixel(x, y) == CENTROID));
    }

    #[test]
    fn silence_draws_and_bad_sizes_are_refused() {
        let silence = Audio::new(48_000, [vec![0.0; 48_000], vec![0.0; 48_000]]);
        let raster = sheet(&silence, &[], &[], 200, 100).unwrap();
        assert_eq!(raster.pixel(0, 0), MAGMA[0]);
        assert!(sheet(&silence, &[], &[], 99, 100).is_err());
        assert!(sheet(&silence, &[], &[], 200, 5000).is_err());
    }
}
