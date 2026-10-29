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

`rotation`, `fov` and `gamma` are rejected for `top`, and `ortho_width` and `show_grid` for `perspective`.

Every result carries a camera string, for example `perspective -2500 0 170 0 0 0 90` (BugItGo order, X Y Z Pitch Yaw Roll, then fov) or `top 0 0 256`. Passing it back as `camera` gives the same view again, the way Unreal's BugItGo restores a view saved by BugIt. `camera` cannot be combined with `viewport`, `location`, `rotation`, `fov` or `ortho_width`.

## The arena in 3D

One arena unit is 25 cm. The arena x axis becomes Unreal Y and arena y becomes Unreal X, so the top view shows what a perspective camera looking straight down with yaw 0 sees. Each entity is a 1 m cube from `assets/gen/render_fixtures/meshes/cube.glb`, coloured by hp. The floor is `floor.glb` scaled to the 25 m spawn square, its top at Z 0, with the `checker.png` texture from `psx_bake`. Two point lights hang 3 m up at X -625 and Y plus or minus 625, intensity 30, attenuation radius 20 m, over an ambient of 0.15. Exposure is 1.0 and the white point 11.2.

## The lighting scene

Scenarios with `"host": "lighting"` run on a fixed room built for lighting work (`docs/design/phase-8-spot-lights.md`), reached through scenarios only; the live MCP simulation stays the arena. The room is 12 by 8 m and 3.5 m high, centred on the origin, with a doorway in the wall at X -600 and three 60 cm crates, all built by `interior_kit` from `specs/lighting/` into `assets/gen/lighting/`. The default camera stands in the doorway at eye height (170 cm) looking along +X. Ambient is 0.05, exposure 1.0 and the white point 11.2. The lights, each testing one thing:

- Two ceiling spots at Z 340 pointing down, fov 60, over X -350 and Y plus or minus 220: spot falloff power 1 at Y 220 and 4 at Y -220.
- A spot at X 250, Z 175 looking along +X at the far wall, fov 40 and aspect 2, with the static `bars` gobo (a window of 3 by 2 panes).
- Two ceiling spots over X 300 with the four-frame `flicker` gobo, 6 ticks a frame: Loop at Y 220, Oscillate at Y -220.
- Two point lights at Z 100 and Y plus or minus 250 with a 3 m radius: falloff power 2 at Y 250 and 1 at Y -250.

The gobos come from `tools/checker_maps` and `specs/lighting/gobos.json`, 64 by 64 pixels.

## Drawing

- `view_mode`, after Unreal's view modes:
  - `lit` (default for `perspective`) lights each surface with ambient plus the point and spot lights, Lambert diffuse with HPL3's distance falloff `pow(max(1 - d / radius, 0), falloff_pow)`, no specular or shadows. A spot is HPL3's frustum: a full vertical fov, an aspect and a near plane. Without a gobo it fades from its axis by `pow(1 - sqrt(t), spot_falloff_pow)`, with `t` the ratio of `1 - cos` of the angle off the axis to `1 - cos(fov / 2)`. With a gobo the point is projected through the frustum and the gobo texel there, nearest and decoded like a colour texture, replaces that fade; outside the frustum's rectangle or behind the light it adds nothing. Gobo frames change by tick in None, Loop or Oscillate mode.
  - `unlit` (default for `top`) shows base colour times texture.
- Perspective frames follow HPL3's linear pipeline (`docs/design/phase-8-linear-hdr.md`). Texture colour is decoded with `pow(c, 2.2)`, lighting goes into a 16-bit float buffer, and one pass at the end writes the 8-bit frame. In `lit` that pass multiplies by the scene's exposure and applies Hable's curve divided by its value at the scene's white point; `unlit` and `wireframe` skip both, so they show authored colour. Last comes the encode, `pow(c, 1 / gamma)`.
- `gamma` (perspective only, default 2.2) is the display gamma of that encode, a player's monitor setting rather than level data. Goldens are recorded at 2.2. The top view is never decoded or encoded.
  - `wireframe` draws edges: triangle edges in `perspective`, a one-pixel outline in `top`.
  - `lit` on `top` is an error.
- In the top view an entity is a 4-unit square, red at hp 0 and green at hp 100.
- `show_grid` (top only, default true) draws a line every 32 world units and brighter axes through 0. The grid is left out when its lines would be under 2 pixels apart.
- `width` and `height` run from 16 to 2048 pixels. Default 256.

In the top view shapes snap to whole pixels and there is no anti-aliasing, so the same state and camera give the same pixels on every adapter tested so far: an NVIDIA GeForce RTX 2050 and WARP on Windows, and Mesa lavapipe on Linux all produce byte-identical top frames. Perspective frames have sloped edges, texture lookups and per-pixel lighting, so adapters differ there by one unit of shading and a few edge pixels. The arena's 3D goldens are recorded on the CI software adapters and checked with the `low` preset; the top shots stay as the debug view.

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
| `exposure`, `white_point`, `gamma` | Perspective only: the scene's exposure and white point and the display gamma used. |
| `point_lights`, `spot_lights`, `gobo_frames` | Perspective only: how many point lights, spot lights and gobo frames the scene holds. |
| `off_screen` | Entities outside the view: for `perspective`, mesh instances whose bounds lie fully outside the view frustum. |
| `adapter` | `name`, `backend` and `device_type` of the GPU or software adapter. |
| `bucket` | `<platform>_<backend>`, for example `windows_dx12`, the golden bucket this frame belongs to. |
| `path` | Where the PNG was written, when `path` was given. |

## Golden screenshots

Scenario files check frames against golden images with the `screenshot` assertion; the format is in `docs/scenarios.md`. Goldens are recorded with `tickora-mcp scenario --record <file>` and alternatives with `--record-alternative`. The comparison follows Unreal's screenshot comparer: per-channel tolerance, a maximum share of differing pixels per 16 by 16 block and over the whole frame, and an optional search of the 8 neighbours for anti-aliasing shifts. The presets are `zero`, an exact match, and Unreal's `low`, `medium` and `high` (16, 24 and 32 per channel, block 0.10, frame 0.02, neighbour search on; Unreal's brightness window is left out); any other tolerance is a custom object. Top frames are identical on every adapter tested. Perspective frames differ by one unit of shading and a few edge pixels, and `low` covers every adapter pair measured, with room to spare: `docs/research/tolerance-presets.md`.

## Adapters

The renderer uses DX12 on Windows and Vulkan on Linux. `TICKORA_ADAPTER` picks the adapter: `auto` (default; a hardware GPU first, then the software one), `hardware` or `software`. CI uses `software`: WARP on windows-latest, and Mesa 26.1.3 lavapipe on ubuntu-latest.

The renderer starts on the first screenshot. On a machine with no adapter the screenshot tool returns an error and the other tools keep working.
