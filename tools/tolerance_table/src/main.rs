use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use tickora_render::{Comparison, Image, Tolerance, compare};

const USAGE: &str = "usage: tolerance-table [--shots <prefix>] [--sweep <channel>,...] <label>=<goldens dir> <label>=<goldens dir>...";

struct Options {
    prefix: String,
    sweep: Vec<u8>,
    sources: Vec<Source>,
}

struct Source {
    label: String,
    dir: PathBuf,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Measure {
    max_channel_delta: u8,
    global_error: f64,
    max_local_error: f64,
    global_error_aa: f64,
    max_local_error_aa: f64,
}

impl Measure {
    fn worst(self, other: Measure) -> Measure {
        Measure {
            max_channel_delta: self.max_channel_delta.max(other.max_channel_delta),
            global_error: self.global_error.max(other.global_error),
            max_local_error: self.max_local_error.max(other.max_local_error),
            global_error_aa: self.global_error_aa.max(other.global_error_aa),
            max_local_error_aa: self.max_local_error_aa.max(other.max_local_error_aa),
        }
    }

    fn row(&self, shot: &str, pair: &str) -> String {
        format!(
            "| {shot} | {pair} | {} | {} | {} | {} | {} |",
            self.max_channel_delta,
            percent(self.global_error),
            percent(self.max_local_error),
            percent(self.global_error_aa),
            percent(self.max_local_error_aa),
        )
    }
}

struct Row {
    channel: u8,
    shot: String,
    pair: String,
    measure: Measure,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match run(&args) {
        Ok(table) => {
            print!("{table}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("tolerance-table: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(args: &[String]) -> Result<String, String> {
    let Options {
        prefix,
        sweep,
        sources,
    } = parse_args(args)?;
    let shots = shared_shots(&sources, &prefix)?;
    let channels: Vec<u8> = std::iter::once(0).chain(sweep.iter().copied()).collect();
    let mut rows = Vec::new();
    for shot in &shots {
        let images = sources
            .iter()
            .map(|source| load_primary(&source.dir.join(shot)))
            .collect::<Result<Vec<_>, _>>()?;
        for i in 0..sources.len() {
            for j in i + 1..sources.len() {
                for &channel in &channels {
                    rows.push(Row {
                        channel,
                        shot: shot.clone(),
                        pair: format!("{} / {}", sources[i].label, sources[j].label),
                        measure: measure(&images[i], &images[j], channel)
                            .map_err(|e| format!("{shot}: {e}"))?,
                    });
                }
            }
        }
    }
    Ok(render(&rows, &sweep))
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut prefix = String::new();
    let mut sweep = Vec::new();
    let mut sources = Vec::new();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg == "--shots" {
            prefix = rest
                .next()
                .ok_or_else(|| format!("--shots needs a prefix; {USAGE}"))?
                .clone();
        } else if arg == "--sweep" {
            sweep = rest
                .next()
                .ok_or_else(|| format!("--sweep needs channel values; {USAGE}"))?
                .split(',')
                .map(|value| {
                    value
                        .parse::<u8>()
                        .ok()
                        .filter(|&channel| channel > 0)
                        .ok_or_else(|| {
                            format!("--sweep takes channel values 1 to 255, got {value:?}")
                        })
                })
                .collect::<Result<_, _>>()?;
        } else if arg.starts_with("--") {
            return Err(format!("unknown option {arg:?}; {USAGE}"));
        } else {
            let (label, dir) = arg
                .split_once('=')
                .filter(|(label, dir)| !label.is_empty() && !dir.is_empty())
                .ok_or_else(|| format!("expected <label>=<dir>, got {arg:?}; {USAGE}"))?;
            sources.push(Source {
                label: label.to_string(),
                dir: PathBuf::from(dir),
            });
        }
    }
    if sources.len() < 2 {
        return Err(format!("at least two sources are needed; {USAGE}"));
    }
    Ok(Options {
        prefix,
        sweep,
        sources,
    })
}

fn shared_shots(sources: &[Source], prefix: &str) -> Result<Vec<String>, String> {
    let mut shots = shot_names(&sources[0].dir, prefix)?;
    for source in &sources[1..] {
        let other = shot_names(&source.dir, prefix)?;
        if other != shots {
            return Err(format!(
                "{} holds shots {other:?} but {} holds {shots:?}",
                source.label, sources[0].label
            ));
        }
    }
    if shots.is_empty() {
        return Err(format!("no shots starting with {prefix:?}"));
    }
    shots.sort();
    Ok(shots)
}

fn shot_names(dir: &Path, prefix: &str) -> Result<Vec<String>, String> {
    let mut names = fs::read_dir(dir)
        .map_err(|e| format!("cannot list {}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with(prefix))
        .collect::<Vec<_>>();
    names.sort();
    Ok(names)
}

fn load_primary(dir: &Path) -> Result<Image, String> {
    let primaries = fs::read_dir(dir)
        .map_err(|e| format!("cannot list {}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".png") && !name.contains(".alt"))
        })
        .collect::<Vec<_>>();
    let [path] = primaries.as_slice() else {
        return Err(format!(
            "{} holds {} primary goldens, expected one",
            dir.display(),
            primaries.len()
        ));
    };
    let bytes = fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Image::from_png(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

fn measure(a: &Image, b: &Image, channel: u8) -> Result<Measure, String> {
    let exact = compare(a, b, &measuring(channel, false))?;
    let aa_forward = compare(a, b, &measuring(channel, true))?;
    let aa_backward = compare(b, a, &measuring(channel, true))?;
    Ok(Measure {
        max_channel_delta: exact.max_channel_delta,
        global_error: exact.global_error,
        max_local_error: exact.max_local_error,
        global_error_aa: worse(&aa_forward, &aa_backward).global_error,
        max_local_error_aa: worse(&aa_forward, &aa_backward).max_local_error,
    })
}

fn measuring(channel: u8, ignore_aa: bool) -> Tolerance {
    Tolerance {
        red: channel,
        green: channel,
        blue: channel,
        alpha: channel,
        max_local_error: 1.0,
        max_global_error: 1.0,
        ignore_aa,
    }
}

fn worse<'a>(a: &'a Comparison, b: &'a Comparison) -> &'a Comparison {
    if (b.global_error, b.max_local_error) > (a.global_error, a.max_local_error) {
        b
    } else {
        a
    }
}

fn percent(share: f64) -> String {
    format!("{:.2}%", share * 100.0)
}

const HEADER: &str = "| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |\n| --- | --- | --- | --- | --- | --- | --- |\n";

fn render(rows: &[Row], sweep: &[u8]) -> String {
    let mut out = String::from(HEADER);
    for row in rows.iter().filter(|row| row.channel == 0) {
        out.push_str(&row.measure.row(&row.shot, &row.pair));
        out.push('\n');
    }
    out.push_str("\nWorst over all shots, per pair:\n\n");
    out.push_str(&worst_per_pair(rows, 0));
    for &channel in sweep {
        out.push_str(&format!(
            "\nWorst over all shots, per pair, with a channel tolerance of {channel}:\n\n"
        ));
        out.push_str(&worst_per_pair(rows, channel));
    }
    out
}

fn worst_per_pair(rows: &[Row], channel: u8) -> String {
    let mut out = String::from(HEADER);
    let mut pairs: Vec<&str> = Vec::new();
    for row in rows {
        if !pairs.contains(&row.pair.as_str()) {
            pairs.push(&row.pair);
        }
    }
    for pair in pairs {
        let worst = rows
            .iter()
            .filter(|row| row.pair == pair && row.channel == channel)
            .fold(Measure::default(), |acc, row| acc.worst(row.measure));
        out.push_str(&worst.row("all", pair));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled(width: u32, height: u32, color: [u8; 4]) -> Image {
        Image::new(width, height, color.repeat((width * height) as usize)).unwrap()
    }

    fn set(image: &mut Image, x: u32, y: u32, color: [u8; 4]) {
        let at = ((y * image.width + x) * 4) as usize;
        image.pixels[at..at + 4].copy_from_slice(&color);
    }

    fn write_shot(root: &Path, shot: &str, bucket: &str, image: &Image) {
        let dir = root.join(shot);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("{bucket}.png")), image.to_png().unwrap()).unwrap();
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("tolerance-table-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn identical_frames_measure_zero() {
        let image = filled(16, 16, [10, 20, 30, 255]);
        assert_eq!(measure(&image, &image, 0).unwrap(), Measure::default());
    }

    #[test]
    fn a_shifted_edge_differs_until_the_neighbour_search() {
        let mut a = filled(32, 16, [0, 0, 0, 255]);
        let mut b = a.clone();
        for y in 0..16 {
            set(&mut a, 4, y, [200, 0, 0, 255]);
            set(&mut b, 5, y, [200, 0, 0, 255]);
        }
        let m = measure(&a, &b, 0).unwrap();
        assert_eq!(m.max_channel_delta, 200);
        assert_eq!(m.global_error, 32.0 / 512.0);
        assert_eq!(m.max_local_error, 32.0 / 256.0);
        assert_eq!(m.global_error_aa, 0.0);
        assert_eq!(m.max_local_error_aa, 0.0);
    }

    #[test]
    fn neighbour_search_is_measured_both_ways() {
        let a = filled(16, 16, [0, 0, 0, 255]);
        let mut b = a.clone();
        set(&mut b, 8, 8, [9, 0, 0, 255]);
        let m = measure(&a, &b, 0).unwrap();
        assert_eq!(m.global_error_aa, 1.0 / 256.0);
    }

    #[test]
    fn table_has_a_row_per_shot_and_pair_and_a_worst_row_per_pair() {
        let root = scratch("table");
        let black = filled(16, 16, [0, 0, 0, 255]);
        let mut dot = black.clone();
        set(&mut dot, 0, 0, [4, 0, 0, 255]);
        for (label, image) in [("x", &black), ("y", &black), ("z", &dot)] {
            write_shot(&root.join(label), "perspective-a", "windows_dx12", image);
            write_shot(&root.join(label), "perspective-b", "linux_vulkan", &black);
            write_shot(&root.join(label), "top-a", "windows_dx12", &black);
        }
        let arg = |label: &str| format!("{label}={}", root.join(label).display());
        let table = run(&[
            "--shots".to_string(),
            "perspective".to_string(),
            arg("x"),
            arg("y"),
            arg("z"),
        ])
        .unwrap();
        assert!(table.contains("| perspective-a | x / z | 4 | 0.39% | 0.39% | 0.39% | 0.39% |"));
        assert!(table.contains("| perspective-b | y / z | 0 | 0.00% | 0.00% | 0.00% | 0.00% |"));
        assert!(table.contains("| all | x / z | 4 | 0.39% | 0.39% | 0.39% | 0.39% |"));
        assert!(table.contains("| all | x / y | 0 |"));
        assert!(!table.contains("top-a"));
        assert!(!table.contains("channel tolerance"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn channel_tolerance_absorbs_small_deltas() {
        let a = filled(16, 16, [0, 0, 0, 255]);
        let mut b = a.clone();
        set(&mut b, 3, 3, [2, 0, 0, 255]);
        set(&mut b, 9, 9, [40, 0, 0, 255]);
        assert_eq!(measure(&a, &b, 0).unwrap().global_error, 2.0 / 256.0);
        assert_eq!(measure(&a, &b, 2).unwrap().global_error, 1.0 / 256.0);
        assert_eq!(measure(&a, &b, 2).unwrap().max_channel_delta, 40);
    }

    #[test]
    fn sweep_adds_a_worst_table_per_channel_value() {
        let root = scratch("sweep");
        let black = filled(16, 16, [0, 0, 0, 255]);
        let mut grey = black.clone();
        set(&mut grey, 0, 0, [2, 2, 2, 255]);
        write_shot(&root.join("x"), "s", "windows_dx12", &black);
        write_shot(&root.join("y"), "s", "linux_vulkan", &grey);
        let arg = |label: &str| format!("{label}={}", root.join(label).display());
        let table = run(&["--sweep".to_string(), "1,2".to_string(), arg("x"), arg("y")]).unwrap();
        let after = |marker: &str| table.split(marker).nth(1).unwrap().to_string();
        assert!(after("channel tolerance of 1:").contains("| all | x / y | 2 | 0.39% |"));
        assert!(after("channel tolerance of 2:").contains("| all | x / y | 2 | 0.00% |"));
        assert!(run(&["--sweep".to_string(), "0".to_string(), arg("x"), arg("y")]).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mismatched_shot_lists_are_an_error() {
        let root = scratch("mismatch");
        let black = filled(16, 16, [0, 0, 0, 255]);
        write_shot(&root.join("x"), "a", "windows_dx12", &black);
        write_shot(&root.join("y"), "b", "windows_dx12", &black);
        let arg = |label: &str| format!("{label}={}", root.join(label).display());
        assert!(run(&[arg("x"), arg("y")]).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn one_source_is_an_error() {
        assert!(run(&["x=somewhere".to_string()]).is_err());
    }
}
