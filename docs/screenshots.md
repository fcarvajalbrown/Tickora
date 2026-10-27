# Screenshots

Tickora renders the simulation offscreen with wgpu and returns PNG frames. No window opens, and taking a screenshot never steps the simulation.

## Camera

The camera follows Unreal's viewport model.

- `viewport`: only `top` exists for now, an orthographic view looking down on the arena plane, with +x to the right and +y up. `perspective`, `front` and `side` are reserved for 3D.
- `location`: `[x, y]` in world units, the centre of the image. Default `[0, 0]`.
- `ortho_width`: world units across the image width. Default 256. The height follows the image's aspect ratio.

Every result carries a camera string, for example `top 0 0 256`. Passing it back as `camera` gives the same view again, the way Unreal's BugItGo restores a view saved by BugIt. `camera` cannot be combined with `viewport`, `location` or `ortho_width`.

## Drawing

- `view_mode`: `unlit` (default) fills each entity, `wireframe` draws a one-pixel outline. In the demo arena an entity is a 4-unit square, red at hp 0 and green at hp 100.
- `show_grid` (default true) draws a line every 32 world units and brighter axes through 0. The grid is left out when its lines would be under 2 pixels apart.
- `width` and `height` run from 16 to 2048 pixels. Default 256.

Shapes snap to whole pixels and there is no anti-aliasing, so the same state and camera give the same pixels on every adapter tested so far: an NVIDIA GeForce RTX 2050 and WARP on Windows, and Mesa lavapipe on Linux all produce byte-identical arena frames.

## MCP tool

`tickora_screenshot` takes the camera and drawing arguments above, all optional, plus `path` to also write the PNG, relative to the server's working directory.

The result holds an image block (base64 PNG, `image/png`) followed by metadata:

| Key | Meaning |
| --- | --- |
| `tick`, `state_hash` | The simulation state the frame shows. |
| `width`, `height` | Frame size in pixels. |
| `pixel_hash` | FNV-1a over the size and the raw RGBA pixels, as a decimal string. It ignores PNG encoding. |
| `camera` | The camera string for this view. |
| `view_mode`, `show_grid` | The drawing settings used. |
| `off_screen` | Entities outside the view. |
| `adapter` | `name`, `backend` and `device_type` of the GPU or software adapter. |
| `bucket` | `<platform>_<backend>`, for example `windows_dx12`, the golden bucket this frame belongs to. |
| `path` | Where the PNG was written, when `path` was given. |

## Golden screenshots

Scenario files check frames against golden images with the `screenshot` assertion; the format is in `docs/scenarios.md`. Goldens are recorded with `tickora-mcp scenario --record <file>` and alternatives with `--record-alternative`. The comparison follows Unreal's screenshot comparer: per-channel tolerance, a maximum share of differing pixels per 16 by 16 block and over the whole frame, and an optional search of the 8 neighbours for anti-aliasing shifts. The one named preset is `zero`, an exact match; any other tolerance is a custom object. Unreal also has low, medium and high presets. Tickora adds them once the renderer has a feature, such as anti-aliasing or textures, that makes adapters differ, so their numbers can be measured. Today every adapter tested renders the arena identically.

## Adapters

The renderer uses DX12 on Windows and Vulkan on Linux. `TICKORA_ADAPTER` picks the adapter: `auto` (default; a hardware GPU first, then the software one), `hardware` or `software`. CI uses `software`: WARP on windows-latest, and Mesa 26.1.3 lavapipe on ubuntu-latest.

The renderer starts on the first screenshot. On a machine with no adapter the screenshot tool returns an error and the other tools keep working.
