use std::fs;
use std::path::{Path, PathBuf};

use crate::compare::{Comparison, Tolerance, compare};
use crate::image::Image;

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Missing {
        expected: PathBuf,
    },
    Compared {
        golden: PathBuf,
        comparison: Comparison,
    },
}

impl Outcome {
    pub fn passed(&self) -> bool {
        matches!(self, Outcome::Compared { comparison, .. } if comparison.passed)
    }
}

pub fn primary_path(dir: &Path, bucket: &str) -> PathBuf {
    dir.join(format!("{bucket}.png"))
}

pub fn candidates(dir: &Path, bucket: &str) -> Result<Vec<PathBuf>, String> {
    let mut found = Vec::new();
    let primary = primary_path(dir, bucket);
    if primary.is_file() {
        found.push(primary);
    }
    let mut alternatives: Vec<(u32, PathBuf)> = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let number = name
                    .strip_prefix(&format!("{bucket}.alt"))?
                    .strip_suffix(".png")?
                    .parse()
                    .ok()?;
                Some((number, entry.path()))
            })
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(format!("cannot list {}: {e}", dir.display())),
    };
    alternatives.sort();
    found.extend(alternatives.into_iter().map(|(_, path)| path));
    Ok(found)
}

pub fn check(
    image: &Image,
    dir: &Path,
    bucket: &str,
    tolerance: &Tolerance,
) -> Result<Outcome, String> {
    let mut best: Option<(PathBuf, Comparison)> = None;
    for path in candidates(dir, bucket)? {
        let bytes = fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let golden = Image::from_png(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        let comparison =
            compare(image, &golden, tolerance).map_err(|e| format!("{}: {e}", path.display()))?;
        if comparison.passed {
            return Ok(Outcome::Compared {
                golden: path,
                comparison,
            });
        }
        if best
            .as_ref()
            .is_none_or(|(_, kept)| comparison.global_error < kept.global_error)
        {
            best = Some((path, comparison));
        }
    }
    Ok(match best {
        Some((golden, comparison)) => Outcome::Compared { golden, comparison },
        None => Outcome::Missing {
            expected: primary_path(dir, bucket),
        },
    })
}

pub fn record(image: &Image, dir: &Path, bucket: &str) -> Result<PathBuf, String> {
    write(image, dir, primary_path(dir, bucket))
}

pub fn record_alternative(image: &Image, dir: &Path, bucket: &str) -> Result<PathBuf, String> {
    let taken = candidates(dir, bucket)?.len() as u32;
    let mut number = taken.max(1);
    let mut path = dir.join(format!("{bucket}.alt{number}.png"));
    while path.exists() {
        number += 1;
        path = dir.join(format!("{bucket}.alt{number}.png"));
    }
    write(image, dir, path)
}

fn write(image: &Image, dir: &Path, path: PathBuf) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    fs::write(&path, image.to_png()?)
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tickora-golden-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn solid(value: u8) -> Image {
        Image::new(16, 16, [value, value, value, 255].repeat(256)).unwrap()
    }

    #[test]
    fn missing_golden_names_the_file_to_record() {
        let dir = scratch("missing");
        let outcome = check(&solid(0), &dir, "linux_vulkan", &Tolerance::ZERO).unwrap();
        assert_eq!(
            outcome,
            Outcome::Missing {
                expected: dir.join("linux_vulkan.png")
            }
        );
        assert!(!outcome.passed());
    }

    #[test]
    fn record_then_check_passes_and_other_buckets_stay_missing() {
        let dir = scratch("record");
        let path = record(&solid(10), &dir, "windows_dx12").unwrap();
        assert_eq!(path, dir.join("windows_dx12.png"));
        assert!(
            check(&solid(10), &dir, "windows_dx12", &Tolerance::ZERO)
                .unwrap()
                .passed()
        );
        assert!(matches!(
            check(&solid(10), &dir, "linux_vulkan", &Tolerance::ZERO).unwrap(),
            Outcome::Missing { .. }
        ));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_alternative_can_pass_when_the_primary_fails() {
        let dir = scratch("alternative");
        record(&solid(10), &dir, "windows_dx12").unwrap();
        let alt = record_alternative(&solid(20), &dir, "windows_dx12").unwrap();
        assert_eq!(alt, dir.join("windows_dx12.alt1.png"));
        let outcome = check(&solid(20), &dir, "windows_dx12", &Tolerance::ZERO).unwrap();
        assert!(outcome.passed());
        assert!(matches!(outcome, Outcome::Compared { golden, .. } if golden == alt));
        let second = record_alternative(&solid(30), &dir, "windows_dx12").unwrap();
        assert_eq!(second, dir.join("windows_dx12.alt2.png"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_failure_reports_the_closest_golden() {
        let dir = scratch("closest");
        record(&solid(10), &dir, "windows_dx12").unwrap();
        let mut near = solid(25);
        near.pixels[..4].copy_from_slice(&[24, 24, 24, 255]);
        let alt = record_alternative(&near, &dir, "windows_dx12").unwrap();
        let outcome = check(&solid(24), &dir, "windows_dx12", &Tolerance::ZERO).unwrap();
        assert!(!outcome.passed());
        assert!(matches!(outcome, Outcome::Compared { golden, .. } if golden == alt));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn replace_overwrites_the_primary() {
        let dir = scratch("replace");
        record(&solid(10), &dir, "windows_dx12").unwrap();
        record(&solid(40), &dir, "windows_dx12").unwrap();
        assert!(
            check(&solid(40), &dir, "windows_dx12", &Tolerance::ZERO)
                .unwrap()
                .passed()
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
