use serde_json::{Value, json};
use tickora_core::input::InputLog;
use tickora_core::rng::Rng;
use tickora_core::sim::Runner;
use tickora_mcp::Server;
use tickora_mcp::demo::{Arena, ArenaHost, Command};
use tickora_mcp::jsonrpc::INVALID_PARAMS;

#[derive(Clone, Copy)]
enum Era {
    Modern,
    Legacy,
}

struct Script {
    era: Era,
    lines: Vec<String>,
}

impl Script {
    fn new(era: Era) -> Self {
        let mut script = Script {
            era,
            lines: Vec::new(),
        };
        match era {
            Era::Modern => script.request("server/discover", json!({})),
            Era::Legacy => {
                script.request(
                    "initialize",
                    json!({ "protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } }),
                );
                script.lines.push(
                    json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }).to_string(),
                );
            }
        }
        script
    }

    fn request(&mut self, method: &str, mut params: Value) {
        if let Era::Modern = self.era {
            params["_meta"] = json!({
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientInfo": { "name": "test", "version": "0" },
                "io.modelcontextprotocol/clientCapabilities": {},
            });
        }
        let id = self.lines.len();
        self.lines.push(
            json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string(),
        );
    }

    fn call(&mut self, tool: &str, arguments: Value) {
        self.request(
            "tools/call",
            json!({ "name": tool, "arguments": arguments }),
        );
    }

    fn run(&self, seed: u64) -> Vec<Value> {
        let input = self.lines.join("\n") + "\n";
        let mut output = Vec::new();
        Server::new(ArenaHost::new(seed))
            .serve(input.as_bytes(), &mut output)
            .unwrap();
        String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}

fn structured(reply: &Value) -> &Value {
    assert_eq!(reply["result"]["isError"], false, "{reply}");
    &reply["result"]["structuredContent"]
}

fn last_hashes(replies: &[Value]) -> (String, String) {
    let hashes = structured(replies.last().unwrap());
    (
        hashes["state_hash"].as_str().unwrap().to_string(),
        hashes["replay_hash"].as_str().unwrap().to_string(),
    )
}

fn random_inputs(rng: &mut Rng) -> Vec<Command> {
    let mut inputs = Vec::new();
    if rng.chance(1, 2) {
        inputs.push(Command::Spawn);
    }
    if rng.chance(1, 3) {
        inputs.push(Command::Damage(rng.next_u32()));
    }
    inputs
}

fn command_json(command: Command) -> Value {
    match command {
        Command::Spawn => json!({ "type": "spawn" }),
        Command::Damage(amount) => json!({ "type": "damage", "amount": amount }),
    }
}

fn agent_session(era: Era, script_seed: u64, calls: usize) -> (Script, InputLog<Command>, u64) {
    let mut rng = Rng::from_seed(script_seed);
    let mut script = Script::new(era);
    let mut log = InputLog::new();
    let mut tick = 0;
    for _ in 0..calls {
        match rng.below(5) {
            0 | 1 => {
                let ticks = u64::from(rng.below(8)) + 1;
                let inputs = random_inputs(&mut rng);
                for &command in &inputs {
                    log.push(tick, command);
                }
                let inputs: Vec<Value> = inputs.into_iter().map(command_json).collect();
                script.call("tickora_step", json!({ "ticks": ticks, "inputs": inputs }));
                tick += ticks;
            }
            2 => script.call(
                "tickora_query",
                json!({ "components": ["position", "health"], "limit": 5 }),
            ),
            3 => script.call(
                "tickora_get_entity",
                json!({ "index": rng.below(6), "generation": 0 }),
            ),
            _ => script.call("tickora_list_components", json!({})),
        }
    }
    script.call("tickora_state_hash", json!({}));
    (script, log, tick)
}

#[test]
fn the_same_seed_and_call_sequence_give_identical_transcripts_and_hashes() {
    let (script, _, _) = agent_session(Era::Modern, 17, 300);
    let first = script.run(0xC0FFEE);
    let second = script.run(0xC0FFEE);
    assert_eq!(first, second);
    assert_eq!(last_hashes(&first), last_hashes(&second));
}

#[test]
fn both_protocol_eras_drive_the_simulation_to_the_same_hashes() {
    let (modern, _, _) = agent_session(Era::Modern, 23, 200);
    let (legacy, _, _) = agent_session(Era::Legacy, 23, 200);
    let modern = modern.run(5);
    let legacy = legacy.run(5);
    assert_eq!(last_hashes(&modern), last_hashes(&legacy));
    for (m, l) in modern.iter().zip(&legacy).skip(1) {
        assert_eq!(
            m["result"]["structuredContent"],
            l["result"]["structuredContent"]
        );
    }
    assert!(
        legacy
            .iter()
            .all(|reply| reply["result"].get("resultType").is_none())
    );
    assert!(
        modern
            .iter()
            .all(|reply| reply["result"]["resultType"] == "complete")
    );
}

#[test]
fn different_seeds_give_different_hashes() {
    let (script, _, _) = agent_session(Era::Modern, 31, 100);
    assert_ne!(last_hashes(&script.run(1)), last_hashes(&script.run(2)));
}

#[test]
fn hashes_over_mcp_match_a_direct_runner_fed_the_same_inputs() {
    let (script, log, ticks) = agent_session(Era::Modern, 41, 250);
    let mut runner = Runner::new(Arena::new(), 77);
    let replay = runner.run(&log, ticks);
    let final_state = structured(script.run(77).last().unwrap()).clone();
    assert_eq!(final_state["tick"], ticks);
    assert_eq!(final_state["state_hash"], runner.state_hash().to_string());
    assert_eq!(final_state["replay_hash"], replay.hash.to_string());
}

#[test]
fn mcp_session_hash_is_pinned() {
    let (script, _, _) = agent_session(Era::Modern, 0xC0FFEE, 400);
    let (state, replay) = last_hashes(&script.run(0xC0FFEE));
    assert_eq!((state.as_str(), replay.as_str()), PINNED);
}

const PINNED: (&str, &str) = ("5353186769580955786", "8908456892389666548");

#[test]
fn reset_returns_to_a_fresh_run() {
    let mut script = Script::new(Era::Modern);
    script.call(
        "tickora_step",
        json!({ "ticks": 10, "inputs": [{ "type": "spawn" }] }),
    );
    script.call("tickora_reset", json!({ "seed": "18446744073709551615" }));
    script.call(
        "tickora_step",
        json!({ "ticks": 3, "inputs": [{ "type": "spawn" }] }),
    );
    let replies = script.run(9);

    let mut fresh = Script::new(Era::Modern);
    fresh.call(
        "tickora_step",
        json!({ "ticks": 3, "inputs": [{ "type": "spawn" }] }),
    );
    let fresh = fresh.run(u64::MAX);

    assert_eq!(structured(&replies[2])["seed"], u64::MAX.to_string());
    assert_eq!(structured(&replies[2])["tick"], 0);
    assert_eq!(last_hashes(&replies), last_hashes(&fresh));
}

#[test]
fn tools_list_exposes_the_six_tools_with_object_schemas() {
    let mut script = Script::new(Era::Modern);
    script.request("tools/list", json!({}));
    let replies = script.run(0);
    let tools = replies[1]["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        [
            "tickora_step",
            "tickora_state_hash",
            "tickora_list_components",
            "tickora_query",
            "tickora_get_entity",
            "tickora_reset",
        ]
    );
    for tool in tools {
        assert_eq!(tool["inputSchema"]["type"], "object");
        let name = tool["name"].as_str().unwrap();
        assert!(
            name.chars()
                .all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c))
        );
    }
}

#[test]
fn a_bad_input_is_a_tool_error_and_changes_nothing() {
    let mut script = Script::new(Era::Modern);
    script.call("tickora_step", json!({ "inputs": [{ "type": "spawn" }] }));
    script.call(
        "tickora_step",
        json!({ "inputs": [{ "type": "spawn" }, { "type": "explode" }] }),
    );
    script.call("tickora_step", json!({ "ticks": 0 }));
    script.call("tickora_step", json!({ "tick": 1 }));
    script.call("tickora_state_hash", json!({}));
    let replies = script.run(3);
    for reply in &replies[2..5] {
        assert_eq!(reply["result"]["isError"], true, "{reply}");
    }
    assert_eq!(structured(&replies[5])["tick"], 1);
    assert_eq!(
        structured(&replies[5])["state_hash"],
        structured(&replies[1])["state_hash"]
    );
}

#[test]
fn entity_and_component_lookups_report_misses_as_tool_errors() {
    let mut script = Script::new(Era::Legacy);
    script.call("tickora_step", json!({ "inputs": [{ "type": "spawn" }] }));
    script.call("tickora_get_entity", json!({ "index": 0, "generation": 0 }));
    script.call("tickora_get_entity", json!({ "index": 0, "generation": 1 }));
    script.call("tickora_query", json!({ "components": ["mana"] }));
    script.call("tickora_query", json!({ "components": [] }));
    script.call("tickora_list_components", json!({}));
    script.call("tickora_teleport", json!({}));
    let replies = script.run(0);

    let entity = structured(&replies[2]);
    let names: Vec<&str> = entity["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["position", "velocity", "health"]);
    assert_eq!(entity["components"][2]["fields"]["hp"], 100);
    assert_eq!(replies[3]["result"]["isError"], true);
    assert_eq!(replies[4]["result"]["isError"], true);
    assert_eq!(structured(&replies[5])["total"], 1);
    let listed = structured(&replies[6]);
    assert_eq!(listed["entities"], 1);
    assert_eq!(
        listed["components"][0],
        json!({ "name": "position", "count": 1 })
    );
    assert_eq!(replies[7]["error"]["code"], INVALID_PARAMS);
}
