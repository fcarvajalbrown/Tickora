# Tickora roadmap

Tickora is for Frictional-style first-person 3D games (ADR 0002).

Finished phases: `docs/ROADMAP-HISTORY.md`. Decisions: `docs/adr/`. Findings: `docs/research/`. Releases: `CHANGELOG.md`.

Each phase is one `0.MINOR.0` release; its CHANGELOG entry and git tag use the same version.

## Open items

- Felipe decides: where the demo lives and the default host (phase 9).
- Felipe decides: the demo's game design (phase 9).
- Felipe decides: what counts as proof and whether CI runs it (phase 9).
- Felipe decides: project name, license and release scope (phase 10).

## Phases 1 to 7: core, MCP, scenarios, screenshots, asset pipeline, 3D renderer, audio

Status: **Done** (0.1.0 to 0.7.0). Full text in the history.

## Phase 8: Bunker-level visuals, 0.8.0

Status: **In Progress**. Builds on phase 6; visuals like Amnesia: The Bunker (PRD section 4).

1. Done. Study Amnesia: The Bunker's look; findings in `docs/research/bunker-look.md` before the rest of the list is fixed.
2. Done. Linear space and HDR (design `docs/design/phase-8-linear-hdr.md`, CI run 40891832875): 2.2 power decode of colour textures, 16-bit float light buffer, Hable's filmic curve with a white point, manual exposure, one gamma encode at the end.
3. Done. Spot lights with gobo textures (design `docs/design/phase-8-spot-lights.md`, CI run 40918042503, goldens from 40917327743): HPL3 frustum spots, HPL3 distance falloff for spots and points, tick-driven gobos, the lighting scene; `low` still passes.
4. In Progress. Shadows from spot lights, and specular highlights (design `docs/design/phase-8-shadows-specular.md`): HPL3 shadow maps for casting spots, HPL2 jitter filter, Blinn-Phong specular, a second lighting room. Steps 1 to 3 of 8 done.
5. Not Started. Normal maps, mipmaps, filtered and anisotropic sampling.
6. Not Started. Exposure areas: per-area exposure in sim volumes that fade from one value to another over ticks, as in HPL3; fog reuses the volumes.
7. Not Started. Distance and height fog, with fog volumes.
8. Not Started. SSAO; its design settles how screenshots stay deterministic.
9. Not Started. Bloom and film grain seeded per tick.
10. Not Started. Sky and ground ambient, then irradiance probes.

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
- Anti-aliasing, after the coal-mine demo (Felipe), FXAA included; the tolerance table is measured again when it lands.
- Parallax mapping, decals and colour grading (`docs/research/bunker-look.md`).
- `front` and `side` viewports, reserved.
