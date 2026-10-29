# Tickora roadmap history

Finished phases, moved from `ROADMAP.md`, newest first.

## Phase 7: audio, 0.7.0

Status: **Done** (0.7.0, CI green on Windows and Linux, run 40764387479). Designs in `docs/design/phase-7-noise.md`, `phase-7-audio-output.md`, `phase-7-sounds.md` and `phase-7-hearing.md`; prior art in `docs/research/agent-hearing.md`.

1. Done. Noise as simulation state: events with position and loudness the monster can hear (`docs/design/phase-7-noise.md`, CI run 40697705009).
2. Done. Headless audio output a scenario can check; Keel for mixing and mastering (`docs/design/phase-7-audio-output.md`, CI run 40717357273).
3. Done. CC0 and generated sounds through the phase 5 tools (`docs/design/phase-7-sounds.md`, CI run 40731326989).
4. Done. Agent hearing: Corpofobia's `audio_listen` describes rendered audio in text through `tickora_listen` and the scenario CLI (`docs/design/phase-7-hearing.md`, CI run 40764387479).

## Phase 6: grey-box 3D renderer, 0.6.0

Status: **Done** (0.6.0, CI green on WARP and lavapipe, run 40619161517). ADRs 0002 and 0003; design in `docs/design/phase-6-renderer.md`; tolerance measurement in `docs/research/tolerance-presets.md`.

1. Done. GLB reader dependency, approved by Felipe: `gltf =1.4.1` (MIT OR Apache-2.0), default features off, only `utils` and `names`.
2. Done. `perspective` viewport (Unreal `ELevelViewportType`) and an eye-height camera string. Arena default `perspective -2500 0 170 0 0 0 90`, 170 cm from Lyra (Felipe); CI run 40615790132.
3. Done. Depth buffer and mesh drawing: reversed-Z `Depth32Float`, `Scene3` instances from meshkit GLBs, `unlit` and `wireframe`; 6 render tests pass on RTX 2050 and WARP locally and in CI run 40599315260 (WARP, lavapipe).
4. Done. Textures from the PSX bake: by material name, nearest and repeat, no mipmaps; a checker through `checker_maps` and `psx_bake` renders its exact texels on RTX 2050 and WARP locally and in CI run 40599885889 (WARP, lavapipe).
5. Done. Point lights with distance falloff (lamp, generator): `lit` view mode, up to 16 lights, Karis 2013 equation (9); 5 lighting render tests pass on RTX 2050 and WARP locally and in CI run 40602838231 (WARP, lavapipe).
6. Done. Tolerance presets `low`, `medium`, `high`: Unreal's 16, 24, 32 per channel, block 0.10, frame 0.02 (Felipe); `low` passes all four adapters in `docs/research/tolerance-presets.md`; CI run 40618435179.
7. Done. 3D goldens in scenarios: ten arena perspective shots (lit, unlit, wireframe) with `low`, goldens from CI (WARP, lavapipe); the top shots kept as the debug view; CI run 40619161517.

## Phase 5: asset pipeline, 0.5.0

Status: **Done** (0.5.0, CI green with 67 tool tests on Windows and Linux). Tools listed in `docs/research/demo-toolkit.md`; ADR 0003.

1. Done. CC0 texture fetchers (ambientCG, Poly Haven, Texture.ninja, `fetch_missing`, `fetch_cc0`) in `tools/`, manifest committed under `specs/`; 10 tests, live ambientCG fetch checked.
2. Done. CC0 sound search and fetchers (BigSoundBank, Freesound CC0, Red Library, Commons, OpenGameArt) in `tools/cc0_audio/`, manifests under `specs/audio/`; live search and Freesound fetch checked.
3. Done. `meshkit`, `interior_kit` and the mesh checks (`check_mesh`, `mesh_gate`, `uv_check`, `coplanar_check`, `canonical_glb`) in `tools/`; 45 tool tests; a probe tunnel piece built, gated and rendered with F3D.
4. Done. `psx_bake` and `contact_sheet` in `tools/cc0_textures/`; 47 tool tests; the fetched ambientCG rock baked to a 64 px PSX texture and sheeted.
5. Done. `docs/licences/` for non-CC0 records and `build_credits` (with `song_licences`); 67 tool tests; credits built from real ambientCG and Freesound manifests.
6. Done. Python dependencies approved by Felipe and pinned in `tools/requirements.txt` (numpy 2.5.4, Pillow 12.3.0, soundfile 0.14.0, trimesh 5.1.1); repo `.venv` imports all four.

## Phase 4: screenshot capture, 0.4.0

Status: **Done**. `tickora-render` with wgpu offscreen (DX12, Vulkan), Unreal-style top camera, golden comparer and per-bucket goldens, the `tickora_screenshot` tool, screenshot assertions; four adapters render identical frames (`docs/screenshots.md`).

## Phase 3: scenario files and assertions, 0.3.0

Status: **Done**. JSON scenarios with hash, count, query and field assertions (`docs/scenarios.md`), the `tickora-mcp scenario` CLI with exit codes, the `tickora_run_scenario` tool on a fresh host.

## Phase 2: MCP server over stdio, 0.2.0

Status: **Done**. `tickora-mcp` serving both MCP eras (2026-07-28 and legacy `initialize`), `Host` trait, demo arena, six tools; registered in Claude Code with one command in `.mcp.json`.

## Phase 1: repository and simulation core with replay, 0.1.0

Status: **Done**. Cargo workspace; `tickora-core` with PCG32 RNG, FNV-1a state hash, tick-indexed input log (`TKIL` v1), `Runner` with chained replay hash, `Fx` Q32.32 fixed point (ADR 0001), sparse-set ECS with field introspection; CI on Windows and Linux in debug and release.
