# Changelog

All notable changes to Tickora, an agent-native game engine in Rust for first-person 3D games, with a deterministic simulation core, a built-in MCP server, scenario tests and headless screenshots. The format follows Keep a Changelog and versions follow Semantic Versioning; each version is one phase of `ROADMAP.md`.

## [Unreleased]

### Added

- `perspective` viewport after Unreal's `LVT_Perspective`: camera string `perspective <x> <y> <z> <pitch> <yaw> <roll> <fov>` in BugItGo order, Unreal axes (centimetres, Z up, X forward), horizontal field of view, reversed-Z projection with an infinite far plane and a 10 cm near plane.
- GLB mesh reader on `gltf` 1.4.1: meshkit's metres, Y up, map to Unreal centimetres, Z up; skins, morph targets, sparse accessors and external buffers are rejected by name.
- `specs/render_fixtures/` and its meshes under `assets/gen/render_fixtures/`, built with `interior_kit` and passing `mesh_gate`.

## [0.5.0]

Asset pipeline: CC0 sourcing with SHA-256 manifests, seeded mesh generators and checks, PSX texture bake, credits.

### Added

- ADR 0002: every Tickora game targets first-person 3D, in the style of Frictional Games.
- ADR 0003: grey-box 3D first, with meshes and textures from generators.
- `ROADMAP.md`, `docs/ROADMAP-HISTORY.md` and this changelog.
- `docs/research/demo-toolkit.md`: the tools available for the coal-mine demo.
- ADR 0004: assets in plain git without LFS; `.gitattributes` with binary, diff and generated attributes.
- Pinned Python dependencies for the asset tools in `tools/requirements.txt`.
- CC0 texture fetchers for ambientCG, Poly Haven and Texture.ninja, `fetch_missing` and `fetch_cc0`, with a `--manifest` option that writes the SHA-256 record under `specs/`.
- CC0 sound search across BigSoundBank, Freesound CC0, the Red Library, Wikimedia Commons and OpenGameArt, per-source fetchers with manifests under `specs/audio/`, and `find_blows` for cutting single impacts.
- `meshkit` and `interior_kit`: seeded GLB pieces from JSON specs and a `layout.csv` in Unreal's convention; mesh checks `check_mesh`, `mesh_gate`, `uv_check`, `coplanar_check` and `canonical_glb`.
- `psx_bake` (CC0 maps to PSX-size albedo with occlusion and height shading) and `contact_sheet`.
- `docs/licences/` for records of assets that are not CC0, and `build_credits`, which builds credits from the asset manifests.

### Changed

- PRD: genre focus and phase order (asset pipeline, 3D renderer, audio, demo, release).

### Fixed

- `build_credits` no longer fails on a source record that has a title and no name, such as a Freesound sound.

## [0.4.0]

Screenshot capture without a window.

### Added

- `tickora-render`: wgpu offscreen renderer, DX12 on Windows and Vulkan on Linux, with an Unreal-style top orthographic camera and camera string.
- Golden comparer with Unreal's tolerance model, and goldens per platform and backend.
- MCP tool `tickora_screenshot` and scenario `screenshot` assertions, with `--record` on the CLI.
- CI renders on pinned software adapters (WARP, Mesa lavapipe).

## [0.3.0]

Scenario files with assertions.

### Added

- JSON scenario format with hash, entity and component count, query and field assertions.
- `tickora-mcp scenario` CLI with exit codes 0, 1 and 2, and `--json`.
- MCP tool `tickora_run_scenario` on a fresh host.

## [0.2.0]

MCP server over stdio.

### Added

- `tickora-mcp` serving both MCP eras (2026-07-28 and legacy `initialize`).
- `Host` trait, demo arena, and tools `tickora_step`, `tickora_state_hash`, `tickora_list_components`, `tickora_query`, `tickora_get_entity`, `tickora_reset`.
- Registration in Claude Code with one command.

## [0.1.0]

Deterministic simulation core with replay.

### Added

- `tickora-core`: PCG32 RNG, FNV-1a state hash, tick-indexed input log, `Runner` with chained replay hash.
- `Fx` Q32.32 fixed point (ADR 0001) and a sparse-set ECS with field introspection.
- CI on Windows and Linux in debug and release.

[Unreleased]: https://github.com/fcarvajalbrown/Tickora/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/fcarvajalbrown/Tickora/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/fcarvajalbrown/Tickora/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/fcarvajalbrown/Tickora/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/fcarvajalbrown/Tickora/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/fcarvajalbrown/Tickora/releases/tag/v0.1.0
