use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as Process;

use serde_json::{Value, json};
use tickora_core::hash::StateHasher;
use tickora_core::input::InputLog;
use tickora_core::sim::Runner;
use tickora_mcp::Host;
use tickora_mcp::demo::{Arena, ArenaHost, Command};
use tickora_mcp::scenario;
use tickora_mcp::tools;

const EXPECTED_TO_FAIL: &str = ".fail.json";

fn scenarios_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios")
}

fn arena_file() -> PathBuf {
    scenarios_dir().join("arena.json")
}

fn scenario_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(scenarios_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    files
}

fn cli(args: &[&Path]) -> (i32, String) {
    let output = Process::new(env!("CARGO_BIN_EXE_tickora-mcp"))
        .arg("scenario")
        .args(args)
        .output()
        .unwrap();
    (
        output.status.code().unwrap(),
        String::from_utf8(output.stdout).unwrap(),
    )
}

#[test]
fn every_scenario_in_the_repo_has_its_expected_outcome() {
    let files = scenario_files();
    assert!(files.len() >= 2, "{files:?}");
    for file in files {
        let name = file.file_name().unwrap().to_str().unwrap();
        let parsed = scenario::load(&file).unwrap();
        let report = scenario::run(&mut ArenaHost::new(0), &parsed).unwrap();
        assert_eq!(
            report.passed,
            !name.ends_with(EXPECTED_TO_FAIL),
            "{name}: {}",
            report.to_json()
        );
    }
}

#[test]
fn a_scenario_replays_to_the_hashes_of_a_runner_fed_the_same_input_log() {
    let parsed = scenario::load(&arena_file()).unwrap();
    let mut log = InputLog::new();
    for batch in &parsed.inputs {
        for input in &batch.inputs {
            log.push(batch.at, Command::from_json(input).unwrap());
        }
    }
    let mut runner = Runner::new(Arena::new(parsed.seed), parsed.seed);
    let replay = runner.run(&log, parsed.run_to);
    let report = scenario::run(&mut ArenaHost::new(0), &parsed).unwrap();
    assert_eq!(report.replay_hash, replay.hash);
    assert_eq!(report.state_hash, runner.state_hash());
    assert_eq!(report.ticks_run, 600);
}

#[test]
fn cli_exit_codes_separate_pass_fail_and_malformed() {
    let failing = scenarios_dir().join("arena-wrong-expectations.fail.json");
    let malformed =
        std::env::temp_dir().join(format!("tickora-malformed-{}.json", std::process::id()));
    fs::write(&malformed, r#"{ "seed": 1 }"#).unwrap();

    let (code, out) = cli(&[&arena_file()]);
    assert_eq!(code, 0, "{out}");
    assert!(out.starts_with("PASS "));
    let (code, out) = cli(&[&arena_file(), &failing]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("FAIL ") && out.contains("expect[2] at 10"));
    let (code, out) = cli(&[&failing, &malformed]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("missing key \"run_to\""));
    let (code, _) = cli(&[]);
    assert_eq!(code, 2);
    fs::remove_file(&malformed).unwrap();

    let (code, out) = cli(&[Path::new("--json"), &arena_file(), &failing]);
    assert_eq!(code, 1);
    let reports: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(reports[0]["report"]["passed"], true);
    assert_eq!(reports[1]["report"]["failed"], 2);
}

#[test]
fn wav_writes_the_whole_run_and_replays_to_the_pinned_audio_hash() {
    let wav = std::env::temp_dir().join(format!("tickora-arena-{}.wav", std::process::id()));
    let (code, out) = cli(&[Path::new("--wav"), &wav, &arena_file()]);
    assert_eq!(code, 0, "{out}");
    assert!(out.starts_with("WROTE "), "{out}");
    let bytes = fs::read(&wav).unwrap();
    fs::remove_file(&wav).unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(bytes.len(), 44 + 600 * 800 * 6);
    let mut chain = StateHasher::new();
    for block in bytes[44..].chunks(800 * 6) {
        let mut hasher = StateHasher::new();
        hasher.write_bytes(block);
        chain.write_u64(hasher.finish());
    }
    let pinned = scenario::load(&arena_file())
        .unwrap()
        .expect
        .iter()
        .find_map(|assertion| match assertion.check {
            scenario::Check::Audio {
                from: 0,
                to: 600,
                check: scenario::AudioCheck::Hash(hash),
            } => Some(hash),
            _ => None,
        })
        .unwrap();
    assert_eq!(chain.finish(), pinned);

    let failing = scenarios_dir().join("arena-wrong-expectations.fail.json");
    let (code, out) = cli(&[Path::new("--wav"), &wav, &arena_file(), &failing]);
    assert_eq!(code, 2, "{out}");
    let (code, _) = cli(&[&arena_file(), Path::new("--wav")]);
    assert_eq!(code, 2);
    assert!(!wav.exists());
}

#[test]
fn the_mcp_tool_runs_a_scenario_file_by_path() {
    let mut host = ArenaHost::new(0);
    let params = json!({
        "name": "tickora_run_scenario",
        "arguments": { "path": arena_file().to_str().unwrap() },
    });
    let reply = tools::call(&mut host, params.as_object().unwrap()).unwrap();
    assert_eq!(reply["isError"], false, "{reply}");
    let report = &reply["structuredContent"];
    assert_eq!(report["passed"], true);
    assert_eq!(report["state_hash"], "8903675243730010471");
    assert_eq!(report["results"].as_array().unwrap().len(), 39);
    assert_eq!(host.tick(), 0);
}
