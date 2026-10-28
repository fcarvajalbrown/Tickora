# Tickora roadmap

Tickora is for Frictional-style first-person 3D games (ADR 0002).

Finished phases: `docs/ROADMAP-HISTORY.md`. Decisions: `docs/adr/`. Findings: `docs/research/`. Releases: `CHANGELOG.md`.

Each phase is one `0.MINOR.0` release; its CHANGELOG entry and git tag use the same version.

## Open items

- Phase 7 is in progress: audio (0.7.0); item 2 is next.
- Felipe decides: where the demo lives and the default host (phase 9).
- Felipe decides: the demo's game design (phase 9).
- Felipe decides: what counts as proof and whether CI runs it (phase 9).
- Felipe decides: project name, license and release scope (phase 10).

## Phases 1 to 6: core, MCP, scenarios, screenshots, asset pipeline, 3D renderer

Status: **Done** (0.1.0 to 0.6.0). Full text in the history.

## Phase 7: audio, 0.7.0

Status: **In Progress**.

1. Done. Noise as simulation state: events with position and loudness the monster can hear (`docs/design/phase-7-noise.md`, CI run 40697705009).
2. Not Started. Headless audio output a scenario can check; Keel for mixing and mastering.
3. Not Started. CC0 and generated sounds through the phase 5 tools.

## Phase 8: Bunker-level visuals, 0.8.0

Status: **Not Started**. Builds on phase 6; visuals like Amnesia: The Bunker (PRD section 4).

1. Not Started. Study Amnesia: The Bunker's look; findings in `docs/research/` before the rest of the list is fixed.
2. Not Started. Shadows from the point lights and specular highlights.
3. Not Started. Normal maps, mipmaps and filtered texture sampling.

## Phase 9: coal-mine demo, 0.9.0

Status: **Not Started**. PRD section 5.

1. Not Started. Where the demo lives and the default host (Felipe decides).
2. Not Started. Demo game design (Felipe decides); layout from real mine plans with branching paths and lights, not one linear or spiral tunnel.
3. Not Started. Mine tile grid built into meshes by the generators.
4. Not Started. Miner: walk, run, lamp; running and the lamp make noise.
5. Not Started. Monster: investigates noise, hunts on sight in light.
6. Not Started. Pump generator burning limited fuel keeps tunnels lit.
7. Not Started. Shaft lift as the exit.
8. Not Started. Proof scenarios and goldens; whether CI runs them (Felipe decides).

## Phase 10: public release, 0.10.0

Status: **Not Started**. PRD sections 9 and 12.

1. Not Started. Project name (Felipe decides).
2. Not Started. License: Felipe says proprietary for now, the PRD says MIT OR Apache-2.0 (Felipe decides).
3. Not Started. What the public release covers (Felipe decides).

## Unscheduled

- MCP: one simulation per process; no `outputSchema`; no input-log export or replay tool.
- ECS: derive macro, systems and schedules, `query3` and above, trig tables (ADR 0001).
- A window a human plays the game in.
- After v1: a scripting layer and a node editor for teammates who don't use agents, with graphs stored as text an agent can edit too (`docs/research/scripting-language.md`).
- Physics and 3D collision (ADR 0003).
- Anti-aliasing, after the coal-mine demo (Felipe); the tolerance table is measured again when it lands.
- `front` and `side` viewports, reserved.
