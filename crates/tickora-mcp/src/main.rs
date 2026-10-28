use std::env;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::process::ExitCode;

use serde_json::{Value, json};
use tickora_mcp::demo::ArenaHost;
use tickora_mcp::hearing::{self, Hearing};
use tickora_mcp::scenario::{self, Record, Report};
use tickora_mcp::{Host, Server};

const USAGE: &str = "usage: tickora-mcp [scenario [--json] [--record | --record-alternative] [--wav <path>] [--listen] [--sheet <path>] <file>...]";
const MALFORMED: u8 = 2;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.split_first() {
        None => serve(),
        Some((command, rest)) if command == "scenario" => scenarios(rest),
        Some((other, _)) => {
            eprintln!("unknown command {other:?}; {USAGE}");
            ExitCode::from(MALFORMED)
        }
    }
}

fn serve() -> ExitCode {
    match Server::new(ArenaHost::new(0)).serve(io::stdin().lock(), io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tickora-mcp: {e}");
            ExitCode::FAILURE
        }
    }
}

fn scenarios(args: &[String]) -> ExitCode {
    let flags = ["--json", "--record", "--record-alternative", "--listen"];
    let split = split_value(args, "--wav").and_then(|(wav, rest)| {
        split_value(&rest, "--sheet").map(|(sheet, rest)| (wav, sheet, rest))
    });
    let (wav, sheet, args) = match split {
        Ok(split) => split,
        Err(message) => {
            eprintln!("{message}; {USAGE}");
            return ExitCode::from(MALFORMED);
        }
    };
    let args = &args[..];
    let has = |flag: &str| args.iter().any(|arg| arg == flag);
    let as_json = has("--json");
    let extras = Extras {
        wav,
        sheet,
        listen: has("--listen"),
    };
    let record = match (has("--record"), has("--record-alternative")) {
        (false, false) => Record::Off,
        (true, false) => Record::Replace,
        (false, true) => Record::Alternative,
        (true, true) => {
            eprintln!("--record and --record-alternative cannot be combined; {USAGE}");
            return ExitCode::from(MALFORMED);
        }
    };
    if let Some(unknown) = args
        .iter()
        .find(|arg| arg.starts_with("--") && !flags.contains(&arg.as_str()))
    {
        eprintln!("unknown option {unknown:?}; {USAGE}");
        return ExitCode::from(MALFORMED);
    }
    let files: Vec<&String> = args
        .iter()
        .filter(|arg| !flags.contains(&arg.as_str()))
        .collect();
    if files.is_empty() {
        eprintln!("no scenario files given; {USAGE}");
        return ExitCode::from(MALFORMED);
    }
    if extras.any() && files.len() > 1 {
        eprintln!("--wav, --listen and --sheet take one scenario file; {USAGE}");
        return ExitCode::from(MALFORMED);
    }
    let mut worst = 0;
    let mut reports = Vec::new();
    for file in files {
        let mut host = ArenaHost::new(0);
        let outcome = scenario::load(Path::new(file))
            .and_then(|scenario| scenario::run_recording(&mut host, &scenario, record))
            .and_then(|report| extras.finish(&host, file).map(|heard| (report, heard)));
        let code = match &outcome {
            Ok((report, _)) if report.passed => 0,
            Ok(_) => 1,
            Err(_) => MALFORMED,
        };
        worst = worst.max(code);
        if as_json {
            reports.push(match outcome {
                Ok((report, heard)) => {
                    let mut entry = json!({ "file": file, "report": report.to_json() });
                    if let Some(path) = &extras.wav {
                        entry["wav"] = json!(path);
                    }
                    if let Some(path) = &extras.sheet {
                        entry["sheet"] = json!(path);
                    }
                    if let Some(heard) = heard {
                        entry["hearing"] = heard.to_json();
                    }
                    entry
                }
                Err(error) => json!({ "file": file, "error": error }),
            });
        } else {
            match outcome {
                Ok((report, heard)) => {
                    for path in [&extras.wav, &extras.sheet].into_iter().flatten() {
                        println!("WROTE {path}");
                    }
                    print_report(file, &report);
                    if let Some(heard) = heard.filter(|_| extras.listen) {
                        println!("{}", heard.text());
                    }
                }
                Err(error) => println!("ERROR {file}: {error}"),
            }
        }
    }
    if as_json {
        println!("{}", Value::Array(reports));
    }
    ExitCode::from(worst)
}

fn split_value(args: &[String], flag: &str) -> Result<(Option<String>, Vec<String>), String> {
    let mut value = None;
    let mut rest = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg != flag {
            rest.push(arg.clone());
            continue;
        }
        let path = args
            .next()
            .filter(|path| !path.starts_with("--"))
            .ok_or(format!("{flag} needs a path"))?;
        if value.replace(path.clone()).is_some() {
            return Err(format!("{flag} is given twice"));
        }
    }
    Ok((value, rest))
}

struct Extras {
    wav: Option<String>,
    sheet: Option<String>,
    listen: bool,
}

impl Extras {
    fn any(&self) -> bool {
        self.wav.is_some() || self.sheet.is_some() || self.listen
    }

    fn finish(&self, host: &ArenaHost, file: &str) -> Result<Option<Hearing>, String> {
        let mut wav = match &self.wav {
            Some(path) => Some(BufWriter::new(create(path)?)),
            None => None,
        };
        let heard = if self.listen || self.sheet.is_some() {
            let out = wav.as_mut().map(|out| out as &mut dyn Write);
            Some(hearing::hear(host, 0, host.tick(), file, out)?)
        } else {
            if let Some(out) = wav.as_mut() {
                tickora_mcp::host::write_wav(host, 0, host.tick(), out)?;
            }
            None
        };
        if let (Some(out), Some(path)) = (wav.as_mut(), &self.wav) {
            out.flush()
                .map_err(|e| format!("cannot write {path}: {e}"))?;
        }
        if let (Some(path), Some(heard)) = (&self.sheet, &heard) {
            let image = heard.sheet.as_ref().ok_or_else(|| heard.reading.clone())?;
            create(path)?
                .write_all(&image.to_png()?)
                .map_err(|e| format!("cannot write {path}: {e}"))?;
        }
        Ok(heard)
    }
}

fn create(path: &str) -> Result<File, String> {
    let path = Path::new(path);
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    File::create(path).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn print_report(file: &str, report: &Report) {
    for outcome in &report.outcomes {
        if let Some(path) = outcome.actual.get("recorded").and_then(Value::as_str) {
            println!("RECORDED {path}");
        }
    }
    let total = report.outcomes.len();
    let failed = report.failures().count();
    if failed == 0 {
        println!(
            "PASS {file}: {total} assertions, tick {}, state hash {}",
            report.ticks_run, report.state_hash
        );
        return;
    }
    println!("FAIL {file}: {failed} of {total} assertions failed");
    for outcome in report.failures() {
        let detail = match &outcome.message {
            Some(message) => message.clone(),
            None => format!("got {} for {}", outcome.actual, outcome.check),
        };
        println!("  expect[{}] at {}: {detail}", outcome.index, outcome.at);
    }
}
