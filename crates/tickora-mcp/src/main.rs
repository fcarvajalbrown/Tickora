use std::env;
use std::io;
use std::path::Path;
use std::process::ExitCode;

use serde_json::{Value, json};
use tickora_mcp::Server;
use tickora_mcp::demo::ArenaHost;
use tickora_mcp::scenario::{self, Report};

const USAGE: &str = "usage: tickora-mcp [scenario [--json] <file>...]";
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
    let as_json = args.iter().any(|arg| arg == "--json");
    let files: Vec<&String> = args.iter().filter(|arg| *arg != "--json").collect();
    if files.is_empty() {
        eprintln!("no scenario files given; {USAGE}");
        return ExitCode::from(MALFORMED);
    }
    let mut worst = 0;
    let mut reports = Vec::new();
    for file in files {
        let outcome = scenario::load(Path::new(file))
            .and_then(|scenario| scenario::run(&mut ArenaHost::new(0), &scenario));
        let code = match &outcome {
            Ok(report) if report.passed => 0,
            Ok(_) => 1,
            Err(_) => MALFORMED,
        };
        worst = worst.max(code);
        if as_json {
            reports.push(match outcome {
                Ok(report) => json!({ "file": file, "report": report.to_json() }),
                Err(error) => json!({ "file": file, "error": error }),
            });
        } else {
            match outcome {
                Ok(report) => print_report(file, &report),
                Err(error) => println!("ERROR {file}: {error}"),
            }
        }
    }
    if as_json {
        println!("{}", Value::Array(reports));
    }
    ExitCode::from(worst)
}

fn print_report(file: &str, report: &Report) {
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
