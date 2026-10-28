use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as Process;

use serde_json::{Value, json};
use tickora_mcp::demo::ArenaHost;
use tickora_mcp::{scenario, shot, tools};

fn workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tickora-shots-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn bucket() -> String {
    shot::renderer().unwrap().info().bucket()
}

fn write_scenario(dir: &Path, camera: &str) -> PathBuf {
    let file = dir.join("shots.json");
    let text = json!({
        "seed": 9,
        "inputs": [{ "at": 0, "inputs": [{"type": "spawn"}, {"type": "spawn"}, {"type": "spawn"}] }],
        "run_to": 30,
        "expect": [
            { "at": 30, "entities": { "eq": 4 } },
            { "at": 30, "screenshot": "goldens/arena-30", "camera": camera, "width": 64, "height": 48 },
        ],
    });
    fs::write(&file, text.to_string()).unwrap();
    file
}

fn cli(args: &[&str]) -> (i32, String) {
    let output = Process::new(env!("CARGO_BIN_EXE_tickora-mcp"))
        .arg("scenario")
        .args(args)
        .output()
        .unwrap();
    (
        output.status.code().unwrap(),
        String::from_utf8(output.stdout).unwrap() + &String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn record_writes_the_bucket_golden_and_a_rerun_passes() {
    let dir = workspace("record");
    let file = write_scenario(&dir, "top 0 0 128");
    let file = file.to_str().unwrap();
    let golden = dir
        .join("goldens/arena-30")
        .join(format!("{}.png", bucket()));

    let (code, out) = cli(&[file]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("no golden for bucket"), "{out}");
    assert!(!golden.exists());

    let (code, out) = cli(&["--record", file]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("RECORDED"), "{out}");
    assert!(golden.is_file());

    let (code, out) = cli(&[file]);
    assert_eq!(code, 0, "{out}");

    write_scenario(&dir, "top 30 0 128");
    let (code, out) = cli(&[file]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("differs from"), "{out}");

    let (code, out) = cli(&["--record-alternative", file]);
    assert_eq!(code, 0, "{out}");
    assert!(
        dir.join("goldens/arena-30")
            .join(format!("{}.alt1.png", bucket()))
            .is_file()
    );
    let (code, out) = cli(&[file]);
    assert_eq!(code, 0, "{out}");

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn record_flags_are_exclusive_and_unknown_options_are_malformed() {
    let dir = workspace("flags");
    let file = write_scenario(&dir, "top 0 0 128");
    let file = file.to_str().unwrap();
    assert_eq!(cli(&["--record", "--record-alternative", file]).0, 2);
    assert_eq!(cli(&["--recrod", file]).0, 2);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_json_report_carries_the_comparison() {
    let dir = workspace("json");
    let file = write_scenario(&dir, "top 0 0 128");
    let file = file.to_str().unwrap();
    assert_eq!(cli(&["--record", file]).0, 0);
    let (code, out) = cli(&["--json", file]);
    assert_eq!(code, 0, "{out}");
    let reports: Value = serde_json::from_str(&out).unwrap();
    let actual = &reports[0]["report"]["results"][1]["actual"];
    assert_eq!(actual["bucket"], bucket());
    assert_eq!(actual["differing_pixels"], 0);
    assert!(actual["pixel_hash"].as_str().is_some());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn malformed_screenshot_assertions_are_rejected() {
    let base = |extra: Value| {
        let mut assertion = json!({ "at": 0, "screenshot": "goldens/x" });
        for (key, value) in extra.as_object().unwrap() {
            assertion[key] = value.clone();
        }
        json!({ "seed": 0, "run_to": 0, "expect": [assertion] })
    };
    for extra in [
        json!({ "tolerance": "loose" }),
        json!({ "tolerance": { "red": 300 } }),
        json!({ "tolerance": { "max_global_error": 2 } }),
        json!({ "tolerance": { "ignore_aa": 1 } }),
        json!({ "tolerance": { "blur": 1 } }),
        json!({ "view_mode": "lit" }),
        json!({ "viewport": "side" }),
        json!({ "zoom": 2 }),
        json!({ "screenshot": "" }),
        json!({ "camera": "top 0 0 1", "ortho_width": 2 }),
    ] {
        assert!(
            scenario::from_json(&base(extra.clone())).is_err(),
            "{extra} was accepted"
        );
    }
    let ok = scenario::from_json(&base(json!({
        "tolerance": { "red": 2, "max_local_error": 0.1, "ignore_aa": false }
    })));
    assert!(ok.is_ok(), "{ok:?}");
}

#[test]
fn an_inline_scenario_over_mcp_reports_a_missing_golden_as_a_failed_assertion() {
    let mut host = ArenaHost::new(0);
    let params = json!({
        "name": "tickora_run_scenario",
        "arguments": { "scenario": {
            "seed": 1,
            "run_to": 0,
            "expect": [{ "at": 0, "screenshot": "target/no-such-golden-dir", "width": 32, "height": 32 }],
        } },
    });
    let reply = tools::call(&mut host, params.as_object().unwrap()).unwrap();
    assert_eq!(reply["isError"], false, "{reply}");
    let report = &reply["structuredContent"];
    assert_eq!(report["passed"], false);
    assert!(
        report["results"][0]["message"]
            .as_str()
            .unwrap()
            .contains("no golden for bucket")
    );
}
