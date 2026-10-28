# PRD: Agent-native game engine (working title, name not chosen yet)

## 1. Summary

A new game engine written in Rust, built so coding agents can create and verify games without an editor. It has a deterministic ECS simulation core, a built-in MCP server, scenario files with assertions, and headless screenshots. Claude Code is the first-class agent. The project is open source under MIT OR Apache-2.0. Its primary goal is making horror games fast, the way Corpofobia was made, with visuals like Amnesia: The Bunker, built with an agent; GitHub stars and discoverability are secondary.

## 2. Problem

Felipe works with Unreal Engine and Godot today. Three pains:

- Agents work through GUIs and bridges instead of the engine's own data.
- The iteration loop is slow.
- Closed or aging engines are hard to learn from and extend, which motivated rebuilding.

Evidence that the gap draws attention, from snapshots taken during research (counts vary between sources):

- Unity MCP (CoplayDev): about 13.6k stars.
- Godot MCP (Coding-Solo): about 4k to 4.6k stars.
- Unity-MCP (IvanMurzak): about 3.5k stars.
- Bevy: 42.9k stars. A Bevy discussion argues its ECS already makes it agent-friendly, and others in the thread disagree.
- Godogen drives Godot, Bevy and Babylon.js with Claude Code and Codex, and showed 0 stars when checked.

Why now: agent bridges are drawing stars, coding agents are Felipe's main workflow, and several old engines are open or abandoned.

## 3. Users

Primary users are solo and indie developers who build with coding agents, plus hobbyists and modders. Claude Code is the first-class agent.

Main journey: the user describes a game, the agent builds it, the engine proves it works through scenario tests, and a human plays it.

## 4. Goals and Non-goals

Goal: make horror games fast, the way Corpofobia was made, with visuals like Amnesia: The Bunker, built with an agent.

Secondary: GitHub stars and discoverability, public games built on the engine, and contributors and forks.

Non-goal for v1: a visual editor or GUI.

## 5. Scope and Features

All four MVP features were chosen as Must.

Must:
- Deterministic simulation core with fixed tick, seeded RNG and input-log replay.
- Built-in MCP server.
- Scenario files with assertions.
- Screenshot capture.

Should and Could: not yet prioritized.

Demo hook: an agent builds a small game and proves it, with no editor.

Genre focus: games in the style of Frictional Games (Amnesia, SOMA): systemic horror without combat, where light, noise and scarce resources drive the tension. Frictional games are first-person 3D, so Tickora targets first-person 3D games and the demo is 3D, never a 2D or top-down substitute. The phase 5 demo is a first-person survival game in that style, set in a fictional abandoned coal mine inspired by Chile's closed southern mines.

## 6. Technical Architecture

Decisions:
- Language: Rust.
- Data model: ECS.
- Dependencies: own core, reuse small libraries (math, image, windowing). Felipe approves each dependency before it is added.
- Screenshots: wgpu offscreen. Pixel output may vary across GPUs.
- Agent connection: MCP over stdio.
- Game logic: written as Rust crates.
- License: MIT OR Apache-2.0.
- Platforms for v1: Windows and Linux.

Components: simulation core (ECS, tick, RNG, replay), MCP server, scenario runner, offscreen renderer.

Data flow: the agent calls MCP tools over stdio. The engine steps the simulation, evaluates scenarios, and returns state, diffs and frames.

External services: none. Everything is free and open source.

## 7. UX

Not applicable as a GUI, since a visual editor is a non-goal. The surfaces are the command line and MCP tools. Command and tool names are not decided.

## 8. Business case

None. Open source only. Success is measured by stars, public games built on the engine, and contributors and forks.

## 9. Risks and Mitigations

- Unproven demand for a new engine. No evidence was found that from-scratch engines gather stars quickly. The bet rests on the agent-native angle. Mitigation: ship the demo early and measure.
- MCP spec and client changes. Mitigation: keep the MCP layer thin and isolated.
- Scope creep into a full engine. Mitigation: hold the MVP to the four Must features.
- Solo maintainer burnout. Mitigation: keep the repo small and well tested so contributors can join.
- Closed engine study. Policy: study open sources and public documentation, and also allow REA on closed engines for behavior notes only, with clean-room separation, conditional on a lawyer's review. A solo developer working with an AI agent will find clean-room separation hard, so the lawyer should address how to handle it.
- GPL code. id Tech 3 and the HPL engines are GPL. Learn from the architecture. Copying code would make this engine GPL.
- Assets. Ship only original or permissively licensed assets. Take-Two sent a DMCA notice against the re3 and reVC projects and later sued their developers, which shows the exposure.
- Legal advice. This document flags legal risk and does not give legal advice. Consult a qualified lawyer before any public release.

## 10. Testing and Acceptance Criteria

The MVP is done when:
- The same seed and inputs produce identical state, with replay hashes matching on Windows and Linux.
- A headless screenshot works on Windows and Linux.
- One command registers the MCP server in Claude Code.

Test strategy:
- Replay-hash tests.
- Golden screenshots with tolerance.
- Fuzzed input logs.
- CI on Windows and Linux.

## 11. Phases and Milestones

Order follows dependencies, confirmed by Felipe; each phase is one `0.MINOR.0` release. Live status is in `ROADMAP.md`.

1. Repository and simulation core with replay (0.1.0).
2. MCP server over stdio (0.2.0).
3. Scenario files and assertions (0.3.0).
4. Screenshot capture with wgpu offscreen (0.4.0).
5. Asset pipeline: mesh and texture generators and CC0 sourcing (0.5.0).
6. Grey-box 3D renderer (0.6.0).
7. Audio (0.7.0).
8. Coal-mine demo game with proof scenarios (0.8.0).
9. Public release, after legal review (0.9.0).

## 12. Open Questions

- Project name.
- Star target, and the timeframe for it.
- Priorities beyond the MVP (Should and Could).
- Whether advanced 3D rendering, console and mobile ports, and an asset marketplace are non-goals.
- Whether "an agent builds and proves a game from one prompt in CI" is an acceptance criterion. It is the demo hook but was not chosen as acceptance.
- How the lawyer should handle clean-room separation for a solo developer, and when to engage one.
- Command and MCP tool names.
- The list of reused libraries and their license compatibility.
- Where the repository is hosted.
