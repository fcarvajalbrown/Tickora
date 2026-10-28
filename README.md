# Tickora

A deterministic game engine in Rust with a built-in MCP server, for building games with coding agents and no editor. The agent writes the game, the engine runs it headless and checks it against scenario tests, and then a person plays it.

![Status](https://img.shields.io/badge/status-planning-orange)
![Language](https://img.shields.io/badge/language-Rust-blue)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux-blue)

## Why

I work in Unreal and Godot. When an agent works on those engines it goes through the GUI or a bridge plugin, not the engine's own data, and every round trip is slow. Tickora is built for agents from the start, so the agent reads the game state directly and decides when the simulation steps.

## Planned for v1

- A deterministic simulation core with a fixed tick, a seeded RNG and input-log replay. The same seed and inputs give the same state on Windows and Linux, and replay hashes prove it.
- An MCP server built in, talking over stdio, registered with one command.
- Scenario files with assertions, so a game can be tested without anyone playing it.
- Headless screenshots through wgpu offscreen rendering.

The data model is an ECS and game logic is written as Rust crates. There is no visual editor in v1.

## Scenario files

A scenario is a JSON file with a seed, inputs at given ticks, a final tick and a list of assertions. An assertion can check the state or replay hash, the number of live entities, how many entities hold a component or a set of components, one field of one entity, a screenshot, or the audio of a tick range. `scenarios/arena.json` runs the demo arena for 600 ticks with 42 assertions: 14 screenshots and 12 audio checks.

```
cargo run -p tickora-mcp -- scenario scenarios/arena.json
```

The command exits 0 when every assertion passes, 1 when one fails and 2 when a file can't be read or parsed. `--json` prints the full report. Over MCP, `tickora_run_scenario` takes a path or the scenario inline and runs it on its own simulation, so the agent's live one is left alone. The format is described in `docs/scenarios.md`.

## Screenshots

The engine renders the simulation offscreen with wgpu (DX12 on Windows, Vulkan on Linux) and no window opens. Over MCP, `tickora_screenshot` returns the frame as an image the agent can look at, plus the tick, state hash and a hash of the pixels. The camera copies Unreal's viewport model: a top orthographic view with a location and an ortho width, and a camera string that brings back the same view later, as BugItGo does in Unreal.

Scenario files can check a frame against golden images, with the tolerance model of Unreal's screenshot comparer. Shapes snap to whole pixels, and an NVIDIA RTX 2050, WARP and Mesa lavapipe all render the demo arena to identical files, so the goldens match exactly. Details are in `docs/screenshots.md`.

## Hearing

An agent can't listen to a WAV, so Tickora describes the audio it renders. Over MCP, `tickora_listen` replays a range of ticks and returns which sounds started and stopped at which tick, how loud the mix is, its peaks, transients, tones and stereo width, a few lines of plain text, and a spectrogram with every sound start drawn on it. The measures and the wording come from `listen.py`, the audio description script from Corpofobia, ported to Rust. A test runs both on the same WAV and checks that the numbers and the text match.

Scenario files can check loudness, how many transients a range has and how many times a named sound started. On the CLI, `--listen` prints the text and `--sheet <path>` writes the image.

## Inspired by HPL3

Tickora takes ideas from HPL3, the Frictional Games engine behind SOMA, Amnesia: Rebirth and Amnesia: The Bunker, for example how its main loop schedules fixed steps and drives Newton physics.

`tools/decompile-all` is a batch client for a pyghidra-mcp (Ghidra) server. It saves checkpoints and resumes when rerun.

## Roadmap

Tickora is built for first-person 3D games in the style of Frictional Games. Phases 1 to 4 (core, MCP server, scenarios, screenshots) are done. Next come the asset pipeline, a grey-box 3D renderer, audio, a demo set in an abandoned coal mine, and a public release. Live status is in [ROADMAP.md](ROADMAP.md) and every release is in [CHANGELOG.md](CHANGELOG.md).

Platforms for v1 are Windows and Linux.
