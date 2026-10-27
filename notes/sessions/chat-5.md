# Chat 5: screenshot capture with wgpu offscreen (PRD phase 4)

Done:
- Felipe's decisions through the UI: dependencies wgpu 30.0.1, png 0.18.1, pollster 1.0.1 (rust-version 1.85 to 1.87); a screenshot returns an inline image plus metadata, with an optional file; Unreal's viewport model as tool arguments; unlit and wireframe view modes with a grid flag; Unreal's screenshot comparer model; pinned software adapters in CI. He approved `docs/design/phase-4-screenshots.md` as written.
- New rule in CLAUDE.md from Felipe: copy from Unreal Engine anything that transports without colliding with the PRD's mission.
- Two follow-up decisions: the brightness window (Unreal's MinBrightness and MaxBrightness) left out, since no engine or study uses one beyond resemble.js; Corpofobia's luma tools studied first at his request. Tolerance presets ship as `zero` plus custom, because four adapters render identical frames.
- `crates/tickora-render`: camera (Unreal-style string), scene layout snapped to whole pixels, wgpu renderer with DX12 on Windows and Vulkan on Linux, PNG encode and decode, pixel hash, comparer (per-channel tolerance, local and global error, ignore-AA neighbour search), golden storage per bucket with alternatives.
- `crates/tickora-mcp`: `Host::scene`, arena drawn as hp-coloured squares, `tickora_screenshot` (tool 8), in-tree base64, screenshot assertions in scenarios, CLI `--record` and `--record-alternative`.
- CI renders on WARP and Mesa 26.1.3 lavapipe and uploads every scenario frame as an artifact. Linux needed `libllvm18`, since ubuntu-latest is now Ubuntu 26.04.
- Measured: RTX 2050, local WARP, CI WARP and CI lavapipe render the four arena shots to byte-identical PNGs. Goldens for windows_dx12 (local WARP) and linux_vulkan (CI artifact, run 40581739194) are committed.
- Docs: `docs/screenshots.md`, the screenshot section of `docs/scenarios.md`, README section.
- Tests: core 55 unit, 9 replay, 5 ECS; MCP 36 unit, 4 scenario, 5 screenshot scenario, 13 session; render 34 unit, 5 render. Green in debug and release locally on the RTX 2050 and on WARP, and in CI run 40582539472 on windows-latest and ubuntu-latest.

Mistake: f8559f6 (`ci:` commit) swept in half-finished `tools.rs` and `lib.rs` edits through `git commit -a`, and main did not build until 7507dc6.

Flags opened: low, medium and high presets wait for a renderer feature that makes adapters differ; only the `top` viewport exists; no perspective, front or side.
Flags closed: PRD phase 4; all phase 4 design decisions.
