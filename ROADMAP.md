# Tickora roadmap

Tickora is for Frictional-style first-person 3D games (ADR 0002).

Finished phases: `docs/ROADMAP-HISTORY.md`. Decisions: `docs/adr/`. Findings: `docs/research/`. Releases: `CHANGELOG.md`.

Each phase is one `0.MINOR.0` release; its CHANGELOG entry and git tag use the same version.

## Open items

- Phase 6 in progress: grey-box 3D renderer (0.6.0).
- Felipe decides: where the demo lives and the default host (phase 8).
- Felipe decides: what counts as proof and whether CI runs it (phase 8).
- Felipe decides: project name, license and release scope (phase 9).

## Phases 1 to 5: core, MCP, scenarios, screenshots, asset pipeline

Status: **Done** (0.1.0 to 0.5.0). Full text in the history.

## Phase 6: grey-box 3D renderer, 0.6.0

Status: **In Progress**. ADRs 0002 and 0003.

1. Done. GLB reader dependency, approved by Felipe: `gltf =1.4.1` (MIT OR Apache-2.0), default features off, only `utils` and `names`.
2. Not Started. `perspective` viewport (Unreal `ELevelViewportType`) and an eye-height camera string.
3. Not Started. Depth buffer and mesh drawing.
4. Not Started. Textures from the PSX bake.
5. Not Started. Point lights with distance falloff (lamp, generator).
6. Not Started. Tolerance presets `low`, `medium`, `high`, measured against the CI software adapters.
7. Not Started. 3D goldens in scenarios; the top view kept as a debug view.

## Phase 7: audio, 0.7.0

Status: **Not Started**.

1. Not Started. Noise as simulation state: events with position and loudness the monster can hear.
2. Not Started. Headless audio output a scenario can check; Keel for mixing and mastering.
3. Not Started. CC0 and generated sounds through the phase 5 tools.

## Phase 8: coal-mine demo, 0.8.0

Status: **Not Started**. PRD section 5.

1. Not Started. Where the demo lives and the default host (Felipe decides).
2. Not Started. Mine tile grid built into meshes by the generators.
3. Not Started. Miner: walk, run, lamp; running and the lamp make noise.
4. Not Started. Monster: investigates noise, hunts on sight in light.
5. Not Started. Pump generator burning limited fuel keeps tunnels lit.
6. Not Started. Shaft lift as the exit.
7. Not Started. Proof scenarios and goldens; whether CI runs them (Felipe decides).

## Phase 9: public release, 0.9.0

Status: **Not Started**. PRD sections 9 and 12.

1. Not Started. Project name (Felipe decides).
2. Not Started. License: Felipe says proprietary for now, the PRD says MIT OR Apache-2.0 (Felipe decides).
3. Not Started. What the public release covers (Felipe decides).

## Unscheduled

- MCP: one simulation per process; no `outputSchema`; no input-log export or replay tool.
- ECS: derive macro, systems and schedules, `query3` and above, trig tables (ADR 0001).
- A window a human plays the game in.
- Shadows, physics and 3D collision (ADR 0003).
- `front` and `side` viewports, reserved.
