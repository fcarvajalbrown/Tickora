# Tickora roadmap history

Finished phases, moved from `ROADMAP.md`, newest first.

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
