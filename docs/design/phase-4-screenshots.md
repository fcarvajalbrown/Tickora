# Phase 4 design: screenshot capture

Status: approved by Felipe.

The decisions behind this design are recorded in `notes/session-notes.md`. Under the CLAUDE.md rule, anything with an Unreal Engine counterpart starts from how Unreal does it.

## Crates

- New crate `crates/tickora-render` holds wgpu 30.0.1 (default features off; dx12, vulkan, wgsl, std), png 0.18.1 and pollster 1.0.1. `tickora-core` keeps zero dependencies.
- `tickora-mcp` depends on `tickora-render`. The workspace `rust-version` goes from 1.85 to 1.87.
- The renderer knows nothing about the arena. It draws a `Scene`: a list of axis-aligned quads (filled or outline) and grid lines in world units, each with an RGBA colour. A host turns its world into a `Scene`.
- The `Host` trait gets `fn scene(&self, view_mode: ViewMode) -> Scene`. `ArenaHost` builds it from position and health.

## Renderer

- Created on the first screenshot call, not at startup, so an MCP server on a machine with no adapter still serves the other seven tools. Without an adapter the screenshot call is a tool error (`isError: true`) naming the cause.
- Adapter choice comes from `TICKORA_ADAPTER`: `auto` (default; hardware first, then the software fallback), `hardware` or `software`. CI sets `software`.
- Target format `Rgba8Unorm`, no sRGB conversion, no MSAA, one sample per pixel. Flat colours on axis-aligned quads sit inside the D3D12 and Vulkan rasterization rules, so the same scene should come out the same across adapters. The golden tests measure how close they actually get.
- Readback pads rows to wgpu's 256-byte alignment and strips the padding before encoding.
- Fixed-point positions convert to f64 only when the scene is built, which ADR 0001 allows for derived output. Nothing flows back into the simulation.

## Camera (Unreal viewport model)

- `viewport`: named after `ELevelViewportType`. Only `top` (orthographic, looking down onto the arena plane) is accepted for now. `perspective`, `front` and `side` are reserved for when there is 3D, and requesting them is an error until then.
- `location`: `[x, y]` in world units, the centre of the view. Default `[0, 0]`.
- `ortho_width`: world units across the image width, as in Unreal's ortho viewports. Default 256. The height follows the image aspect ratio.
- Arena axes on screen: +x right, +y up. Unreal is Z-up and left-handed, and I have not confirmed how its top viewport orients X and Y on screen. I will check that against Epic's docs during the build and match it if it transports cleanly.
- Camera string, like BugIt: every result carries `camera: "top <x> <y> <ortho_width>"`. Passing it back as the `camera` argument reproduces the same view, the way BugItGo does. `camera` cannot be combined with `viewport`, `location` or `ortho_width`.

## Drawing

- Background dark grey. With `show_grid` (default true), grid lines every 32 world units and brighter axis lines through 0.
- `view_mode: "unlit"` (default): each entity is a filled 4-unit square centred on its position. Colour runs from red at hp 0 to green at hp 100, computed in integers and clamped.
- `view_mode: "wireframe"`: the same squares, outline only, one pixel wide, same colour.
- Entities whose square lies fully outside the view are counted in `off_screen`.

## MCP tool `tickora_screenshot`

Arguments, all optional: `viewport`, `location`, `ortho_width`, `camera`, `width` and `height` (16 to 2048, default 256), `view_mode`, `show_grid`, `path`.

Result:

- An `image` content block: base64 PNG, `mimeType: image/png`. The base64 encoder is a small in-tree function, not a new dependency.
- A text block and `structuredContent` with `tick`, `state_hash`, `width`, `height`, `pixel_hash`, `camera`, `view_mode`, `off_screen`, `adapter` (name, backend, device type), `bucket` and `path` when one was given.
- `pixel_hash` is FNV-1a over the raw RGBA pixels, not over the PNG bytes, because a different encoder setting changes the PNG bytes without changing the image. It takes the place of the "PNG hash" in the earlier question's preview.
- `path`, relative to the server's working directory, writes the PNG there too.

The live simulation is only read, never stepped.

## Golden comparison (Unreal comparer model)

- Bucket: `<platform>_<backend>`, for example `windows_dx12` or `linux_vulkan`, after Unreal's `Platform_RHI_ShaderModel`. The device type is left out of the bucket on purpose, so a hardware GPU is compared against the software golden and the tolerance has to cover that gap.
- Goldens live in a directory per shot: `<bucket>.png`, plus `<bucket>.alt<N>.png` for alternatives (Unreal's Add As Alternative). An image passes if it matches any file in its bucket, as Unreal picks the closest ground truth.
- Tolerance presets `zero`, `low`, `medium`, `high`, `custom`. `custom` takes per-channel tolerances (red, green, blue, alpha). Unreal's custom tolerance also has min and max brightness; they are left out, by Felipe's rule, because no other engine and no study uses a brightness window (see `notes/session-notes.md`).
- Checks: a pixel differs when any channel is past its tolerance. Max local error is the share of differing pixels in any 16 by 16 chunk; max global error is the share across the image. `ignore_aa` (default true) lets a pixel match its expected colour in any of its 8 neighbours.
- Preset numbers: Unreal's public docs do not give them, so `zero` is exact and `low`, `medium` and `high` get numbers measured on this hardware against the CI software adapters. The measured numbers go in `notes/session-notes.md` and the user doc.
- Missing ground truth: Unreal warns on the first run. Here it is a failed assertion that says how to record it, because a warning would let CI pass with no golden at all. This is the one departure from Unreal, and the PRD's test strategy (golden screenshots with tolerance) is the reason for it.
- Recording: only through an explicit CLI flag. `--record` writes or replaces `<bucket>.png` (Unreal's Add New and Replace); `--record-alternative` adds the next `alt<N>`. The MCP tool never writes goldens.

## Scenario assertion

A seventh assertion kind for `docs/scenarios.md`:

```json
{ "at": 600, "screenshot": "goldens/arena/top-600", "camera": "top 0 0 256",
  "view_mode": "unlit", "width": 256, "height": 256, "tolerance": "low" }
```

- `screenshot` is the golden directory, relative to the scenario file (or the server's working directory for an inline scenario over MCP).
- A missing adapter makes the run a scenario error (exit 2), not a failed assertion, because nothing was compared.
- `scenarios/arena.json` gains screenshot assertions at ticks 0 and 600, unlit and wireframe, with goldens for `windows_dx12` and `linux_vulkan` recorded from the CI software adapters.

## CI

- ubuntu-latest: download `mesa-26.1.3-linux-x86_64.tar.xz` from gfx-rs/ci-build release build29, write an `icd.json` pointing at `libvulkan_lvp.so`, and set `VK_DRIVER_FILES` and `LD_LIBRARY_PATH`, as wgpu's own `install-mesa` action does.
- windows-latest: the built-in DX12 WARP through wgpu's fallback adapter. If the runner lacks it or it is too old, I stop and bring you the NuGet WARP route.
- Both set `TICKORA_ADAPTER=software`. The existing fmt, clippy and debug and release test steps stay, and the screenshot tests run inside them.
- The Linux goldens come from a CI run: the first CI run uploads its rendered images as a workflow artifact, I check them, and they get committed as the `linux_vulkan` goldens.

## Tests

- Unit: base64 against RFC 4648 vectors, camera string round trip and parse errors, hp colour ramp, scene building from a small arena, comparer maths on hand-built images (each preset, local versus global error, the neighbour search).
- Render: a known scene rendered at small size; check exact pixels at quad centres and at background points, which should hold on any adapter.
- Golden: the arena scenario with its screenshot assertions, in debug and release.
- MCP session: `tickora_screenshot` returns an image block plus metadata, `camera` round trip, argument errors.

## Order of commits

1. Workspace `rust-version` 1.87 and the `tickora-render` crate with the renderer and render tests.
2. Comparer and golden storage, with unit tests.
3. `Host::scene` and the arena scene.
4. `tickora_screenshot` MCP tool, base64, session tests.
5. Scenario screenshot assertion, the CLI record flags, `docs/scenarios.md`.
6. CI adapters, then goldens recorded from CI.
7. User doc `docs/screenshots.md`, README, session notes with the measured tolerances.

## Not in this phase

Perspective, front and side viewports; text or labels in frames; an interactive viewer window; recording goldens from MCP; a lit view mode.
