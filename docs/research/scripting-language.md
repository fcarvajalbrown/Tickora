# Scripting language and a Blueprint-style editor

Felipe asked three things: what the research says about a scripting language for an agent-first engine, which languages are candidates, and whether a Blueprint-style node editor for people who do not use agents makes sense later. Nothing here is decided. Tickora has no scripting language today; the PRD says game logic is written as Rust crates.

## 1. Agent-first and scripting

The evidence is thin. No study I found compares a compile-then-test loop (Rust, C++, C#) with an interpreted script loop for coding agents.

- An EMNLP 2025 paper ("Real-time world crafting") does not let the LLM write general-purpose code. The LLM writes a constrained DSL that a deterministic engine interprets, because arbitrary generated code risks stability and security. That is the closest published support for a narrow script layer.
- Unreal is going the other way: making its existing visual scripts reachable by agents. Secondary sources say UE 5.8 (June 2026) ships an experimental MCP plugin that exposes Blueprints, assets, levels and materials. I did not confirm this on Epic's site. Community MCP servers (mcp-unreal, ziggymar's Unreal Engine MCP, LOOMLE) edit Blueprint graphs, which mcp-unreal calls "normally opaque binary assets".
- One opinion piece, a multi-model poll, argues text-based project formats suit LLMs better than inspector-driven ones. It is an opinion, not a measurement.

What this means for Tickora, as my reading and not a finding: agents already write the Rust gameplay well, and the costs of Rust here are the compile time and the need to rebuild `tickora-mcp` for every new game. A script layer would buy hot reload, games loaded by the MCP server at runtime, and a sandbox. It must keep the core's rules: integer or Fx math only, no clock, no unordered iteration.

## 2. Candidates

Every one is a new dependency and needs Felipe's approval; none would enter `tickora-core`. A community benchmark (`script-bench-rs`) compares mlua (Lua 5.4 and Luau), Rhai, Rune and wasmtime on speed and Rust interop.

| Candidate | What it is | Fit with determinism | Agents writing it |
| --- | --- | --- | --- |
| Rhai | Pure Rust, JavaScript-like syntax, best docs and playground per users | Features `no_float` and `only_i64` remove floats; `set_max_operations` caps runaway scripts (0, unlimited, by default). Determinism itself is not documented | No benchmark found; a niche language |
| Luau through mlua | Roblox's typed, sandboxed Lua | Numbers are doubles, against the integer rule unless the API hands scripts Fx only. Pulls in a C/C++ build | MultiPL-E (2022, older models) found "remarkably good" Lua results |
| Rune | Rust-native dynamic language | Not checked | Young project, little training data likely |
| WebAssembly (wasmtime or wasmi) | Run modules compiled from Rust or other languages | WASM float NaN bit patterns are not fully fixed by the spec; from memory, wasmtime can canonicalize NaNs (not checked this session). Integer-only modules avoid it | Agents write Rust and compile it; not live scripting |
| A JSON behaviour format | Data, not a language: states, triggers, actions interpreted by Rust | Deterministic by construction | Closest to the EMNLP paper; also editable by a node editor |

## 3. A Blueprint-style editor later

Doable, but it needs things Tickora does not have yet: a window (Unscheduled in the ROADMAP) and a UI stack.

- egui-snarl (0.12.0) is a node-graph widget for egui. The graph is typed data with nodes and wires, and it serializes, so the editor and the agent can work on the same file.
- egui_node_graph is archived. Leat does node graphs for DSP, with a Bevy integration. Oxygengine's script-flow last released in December 2021. I found nothing on built-in visual scripting in Fyrox or Bevy.
- The lesson from Unreal: Blueprints are binary, so agents needed MCP bridges built afterwards. If Tickora ever has node graphs, store them as text (the JSON behaviour format above) from day one, so a designer in the editor and an agent over MCP edit the same thing.

## Sources

- EMNLP 2025, "Real-time world crafting: generating structured game behaviors from natural language with large language models": https://underline.io/lecture/137717-real-time-world-crafting-generating-structured-game-behaviors-from-natural-language-with-large-language-models
- Puget Systems, "Unreal Engine MCP hands-on" (2026-07-09): https://www.pugetsystems.com/blog/2026/07/09/unreal-engine-mcp-hands-on-testing-ai-inside-the-editor/
- mcp-unreal: https://mcpservers.org/servers/remiphilippe/mcp-unreal
- Unreal Engine MCP (ziggymar): https://mcpservers.org/servers/ziggymar/unreal-mcp
- LOOMLE: https://awesome.ecosyste.ms/projects/github.com%2Floomle%2Floomle
- AI Roundtable poll: https://opper.ai/ai-roundtable/questions/best-game-engine-framework-to-use-with-llm-coding-agents-3cb2c917
- script-bench-rs: https://awesome.ecosyste.ms/projects/github.com%2Fkhvzak%2Fscript-bench-rs
- Rhai max operations: https://rhai.rs/book/safety/max-operations.html
- Rhai numbers: https://rhai.rs/book/language/numbers.html
- Luau: https://www.github.com/luau-lang
- MultiPL-E: https://arxiv.org/pdf/2208.08227
- egui-snarl: https://docs.rs/egui-snarl/latest/egui_snarl/
- egui_node_graph: https://awesome.ecosyste.ms/projects/github.com%2Fsetzer22%2Fegui_node_graph
- Leat: https://codeberg.org/Azorlogh/leat
- oxygengine-script-flow: https://lib.rs/crates/oxygengine-script-flow
