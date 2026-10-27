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

A scenario is a JSON file with a seed, inputs at given ticks, a final tick and a list of assertions. An assertion can check the state or replay hash, the number of live entities, how many entities hold a component or a set of components, or one field of one entity. `scenarios/arena.json` runs the demo arena for 600 ticks with 16 assertions.

```
cargo run -p tickora-mcp -- scenario scenarios/arena.json
```

The command exits 0 when every assertion passes, 1 when one fails and 2 when a file can't be read or parsed. `--json` prints the full report. Over MCP, `tickora_run_scenario` takes a path or the scenario inline and runs it on its own simulation, so the agent's live one is left alone. The format is described in `docs/scenarios.md`.

## Inspired by HPL3

Tickora takes ideas from HPL3, the Frictional Games engine behind SOMA, Amnesia: Rebirth and Amnesia: The Bunker, for example how its main loop schedules fixed steps and drives Newton physics.

`tools/decompile-all` is a batch client for a pyghidra-mcp (Ghidra) server. It saves checkpoints and resumes when rerun.

## Roadmap

Planned order:

1. Repository and simulation core with replay.
2. MCP server over stdio.
3. Scenario files and assertions.
4. Screenshot capture.
5. A demo game that an agent builds and proves.

Platforms for v1 are Windows and Linux.
