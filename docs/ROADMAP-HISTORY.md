# Tickora roadmap history

Finished phases, moved from `ROADMAP.md`, newest first.

## Phase 4: screenshot capture, 0.4.0

Status: **Done**. `tickora-render` with wgpu offscreen (DX12, Vulkan), Unreal-style top camera, golden comparer and per-bucket goldens, the `tickora_screenshot` tool, screenshot assertions; four adapters render identical frames (`docs/screenshots.md`).

## Phase 3: scenario files and assertions, 0.3.0

Status: **Done**. JSON scenarios with hash, count, query and field assertions (`docs/scenarios.md`), the `tickora-mcp scenario` CLI with exit codes, the `tickora_run_scenario` tool on a fresh host.

## Phase 2: MCP server over stdio, 0.2.0

Status: **Done**. `tickora-mcp` serving both MCP eras (2026-07-28 and legacy `initialize`), `Host` trait, demo arena, six tools; registered in Claude Code with one command in `.mcp.json`.

## Phase 1: repository and simulation core with replay, 0.1.0

Status: **Done**. Cargo workspace; `tickora-core` with PCG32 RNG, FNV-1a state hash, tick-indexed input log (`TKIL` v1), `Runner` with chained replay hash, `Fx` Q32.32 fixed point (ADR 0001), sparse-set ECS with field introspection; CI on Windows and Linux in debug and release.
