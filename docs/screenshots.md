# Screenshots

Tickora renders the simulation offscreen with wgpu and returns PNG frames. No window opens, and taking a screenshot never steps the simulation.

## Camera

The camera follows Unreal's viewport model. `viewport` is `top` (default) or `perspective`; `front` and `side` are reserved.

`perspective` is the first-person 3D view, in Unreal's axes and units: centimetres, X forward, Y right, Z up.

- `location`: `[x, y, z]` in centimetres.
- `rotation`: `[pitch, yaw, roll]` in degrees, as Unreal's `FRotator`: yaw turns from +X towards +Y, positive pitch looks up.
- `fov`: the horizontal field of view in degrees, 5 to 170.

Any of the three left out comes from the host's default camera. The arena's stands at `[-2500, 0, 170]` looking along +X with fov 90: 170 cm is Felipe's eye height, taken from Epic's Lyra sample, and -2500 puts the whole 25 m spawn square inside the 90 degree view.

`top` is the orthographic debug view, looking down with +x to the right and +y up.

- `location`: `[x, y]` in world units, the centre of the image. Default `[0, 0]`.
- `ortho_width`: world units across the image width. Default 256. The height follows the image's aspect ratio.

`rotation` and `fov` are rejected for `top`, and `ortho_width` and `show_grid` for `perspective`.

Every result carries a camera string, for example `perspective -2500 0 170 0 0 0 90` (BugItGo order, X Y Z Pitch Yaw Roll, then fov) or `top 0 0 256`. Passing it back as `camera` gives the same view again, the way Unreal's BugItGo restores a view saved by BugIt. `camera` cannot be combined with `viewport`, `location`, `rotation`, `fov` or `ortho_width`.

## The arena in 3D

One arena unit is 25 cm. The arena x axis becomes Unreal Y and arena y becomes Unreal X, so the top view shows what a perspective camera looking straight down with yaw 0 sees. Each entity is a 1 m cube from `assets/gen/render_fixtures/meshes/cube.glb`, coloured by hp. The floor is `floor.glb` scaled to the 25 m spawn square, its top at Z 0, with the `checker.png` texture from `psx_bake`. Two point lights hang 3 m up at X -625 and Y plus or minus 625, intensity 30, attenuation radius 20 m, over an ambient of 0.15.

## Drawing

- `view_mode`, after Unreal's view modes:
  - `lit` (default for `perspective`) lights each surface with ambient plus the point lights: Lambert diffuse with Unreal's windowed inverse-square falloff, no specular or shadows.
  - `unlit` (default for `top`) shows base colour times texture.
  - `wireframe` draws edges: triangle edges in `perspective`, a one-pixel outline in `top`.
  - `lit` on `top` is an error.
- In the top view an entity is a 4-unit square, red at hp 0 and green at hp 100.
- `show_grid` (top only, default true) draws a line every 32 world units and brighter axes through 0. The grid is left out when its lines would be under 2 pixels apart.
- `width` and `height` run from 16 to 2048 pixels. Default 256.

In the top view shapes snap to whole pixels and there is no anti-aliasing, so the same state and camera give the same pixels on every adapter tested so far: an NVIDIA GeForce RTX 2050 and WARP on Windows, and Mesa lavapipe on Linux all produce byte-identical top frames. Perspective frames have sloped edges, texture lookups and per-pixel lighting, so adapters may differ there; that is measured before the 3D goldens are recorded.

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
| `off_screen` | Entities outside the view: for `perspective`, mesh instances whose bounds lie fully outside the view frustum. |
| `adapter` | `name`, `backend` and `device_type` of the GPU or software adapter. |
| `bucket` | `<platform>_<backend>`, for example `windows_dx12`, the golden bucket this frame belongs to. |
| `path` | Where the PNG was written, when `path` was given. |

## Golden screenshots

Scenario files check frames against golden images with the `screenshot` assertion; the format is in `docs/scenarios.md`. Goldens are recorded with `tickora-mcp scenario --record <file>` and alternatives with `--record-alternative`. The comparison follows Unreal's screenshot comparer: per-channel tolerance, a maximum share of differing pixels per 16 by 16 block and over the whole frame, and an optional search of the 8 neighbours for anti-aliasing shifts. The one named preset is `zero`, an exact match; any other tolerance is a custom object. Unreal also has low, medium and high presets. Tickora adds them once the renderer has a feature, such as anti-aliasing or textures, that makes adapters differ, so their numbers can be measured. Today every adapter tested renders the arena identically.

## Adapters

The renderer uses DX12 on Windows and Vulkan on Linux. `TICKORA_ADAPTER` picks the adapter: `auto` (default; a hardware GPU first, then the software one), `hardware` or `software`. CI uses `software`: WARP on windows-latest, and Mesa 26.1.3 lavapipe on ubuntu-latest.

The renderer starts on the first screenshot. On a machine with no adapter the screenshot tool returns an error and the other tools keep working.
