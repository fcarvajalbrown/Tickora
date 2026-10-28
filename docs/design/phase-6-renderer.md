# Phase 6 design: grey-box 3D renderer

Status: approved by Felipe, eye height included.

ADRs 0002 and 0003 set the target: a first-person 3D view with a depth buffer, meshes, textures and point lights, drawn from generator output. Under the CLAUDE.md rule, every piece starts from how Unreal does it. Where Unreal's choice does not transport, the reason is given.

## Sources checked

- `ELevelViewportType` (Epic API docs): `LVT_OrthoXY`, `LVT_OrthoXZ`, `LVT_OrthoYZ`, `LVT_Perspective`, plus free-look and negative ortho views. The 4.26 docs name the first three Top, Front and Left; newer docs give other display names, so Tickora keeps its own names `top`, `front`, `side` for the ortho views and `perspective` for `LVT_Perspective`.
- `UCheatManager::BugItGo(float X, float Y, float Z, float Pitch, float Yaw, float Roll)` (Epic API docs): moves the player and sets the rotation. The perspective camera string follows this order.
- `UCameraComponent` field of view (Epic Python API docs): the horizontal angle in degrees, used in perspective mode only. A forum thread gives 90 as the default for new viewports.
- `UPointLightComponent` (Epic API docs): with inverse squared falloff, `AttenuationRadius` only clamps the light's contribution.
- Karis, "Real Shading in Unreal Engine 4", SIGGRAPH 2013 course notes, equation (9): `falloff = saturate(1 - (distance / lightRadius)^4)^2 / (distance^2 + 1)`.

Not verified, so not used as fact: Unreal's default `BaseEyeHeight` and the source of `GetLocalLightAttenuation`. Eye height is a value Felipe sets (see "Open values").

## World convention

- Tickora's 3D world uses Unreal's axes and units: centimetres, Z up, X forward, Y right (left-handed). `interior_kit` already writes `layout.csv` in this convention, so a placement list goes into a scene without conversion.
- GLB files are metres, Y up, right-handed. Meshkit builds in metres, Z up, right-handed `(x, y, z)` and writes `(x, z, -y)`. The reader maps a glTF position `(gx, gy, gz)` to Unreal `(gx, gz, gy) * 100`: back to meshkit's frame, then Y negated for the handedness change. The swap is a reflection, so triangle winding flips; the pipeline's front face is set to match. A test builds an asymmetric meshkit piece and checks that its known corners land where the conversion says.
- Simulation state stays a fixed-point grid (ADR 0003). Hosts convert to f64 only when building a scene, as the top view does today (ADR 0001).

## Crates

- `tickora-render` gains `gltf = { version = "=1.4.1", default-features = false, features = ["utils", "names"] }` (item 1, approved). No other new dependency: PNG textures are read with the existing `png` 0.18.1, matrices are a small in-tree module in f64.
- `tickora-core` keeps zero dependencies.

## Camera (Unreal viewport model)

- `Viewport` gains `Perspective`. `front` and `side` stay reserved.
- `Camera` becomes two shapes: the existing top camera, and a perspective camera with `location [x, y, z]` in centimetres, `rotation [pitch, yaw, roll]` in degrees (Unreal `FRotator` order and meaning: yaw about Z from +X towards +Y, pitch up positive, roll about the view axis), and `fov`, the horizontal field of view in degrees, default 90, accepted from 5 to 170.
- Camera string, BugItGo order plus the field of view: `perspective <x> <y> <z> <pitch> <yaw> <roll> <fov>`, for example `perspective 0 0 <eye height> 0 0 0 90`. It round-trips exactly, as the top string does. The top string `top <x> <y> <ortho_width>` is unchanged, so every phase 4 golden and scenario stays valid.
- Eye height: a scene carries the default perspective camera its host proposes, standing at the player start with Z at the floor plus the host's eye height. Without `camera` or `location`, a perspective request uses that default. The renderer itself has no eye height constant.
- Projection: reversed Z with an infinite far plane, as Unreal's perspective views, near plane 10 cm. `Depth32Float`, compare `Greater`, cleared to 0.

## Scene and meshes

- A new `Scene3` beside today's `Scene`: background colour, ambient light colour, mesh instances (mesh, location, rotation, uniform scale, optional material colour override), point lights, and the default camera. `Scene` and the top view stay as they are and remain the debug view (ADR 0002).
- `MeshLibrary` loads GLB files once and caches them by path. A mesh holds one primitive per material: positions, normals, UVs, u32 indices, material name and `baseColorFactor`. Anything meshkit does not write (skins, morph targets, sparse accessors, more than one buffer, external URIs, non-triangle modes) is rejected with a message naming the file and the feature.
- Draw order is scene order; with a depth buffer the order does not change the image except for exact depth ties, which the coplanar check in `tools/` keeps out of generated meshes.

## Textures

- A material whose name has a texture in the scene's texture set is drawn with it; the set maps material names to PNG files from `psx_bake`, the way Corpofobia's `texture_glb.py` applies baked textures by material name.
- Textures upload as `Rgba8Unorm` with no mipmaps, sampled nearest-neighbour with repeat: the PSX look, and fewer places for adapters to differ than filtered sampling.
- The texture colour multiplies `baseColorFactor`. No sRGB conversion, matching the phase 4 target format.

## View modes (Unreal names)

- `lit` (default for `perspective`): diffuse only. Colour = base colour times texture times (ambient plus the sum over point lights of light colour times intensity times `max(0, N·L)` times falloff). Specular, shadows and exposure are out (ADR 0003).
- `unlit`: base colour times texture, no lights.
- `wireframe`: triangle edges as a line list built on the CPU and depth-tested. wgpu's line polygon mode needs a feature the downlevel limits and software adapters do not promise, so the edges are separate line primitives instead.
- The top view keeps `unlit` and `wireframe` only; `lit` on `top` is an error.

## Point lights

- A point light has location, colour, intensity and attenuation radius, after `UPointLightComponent` with inverse squared falloff.
- Falloff is Karis equation (9) with distance in metres inside the formula, so the `+ 1` in the denominator is 1 m² and a light at 1 m gives one half of its intensity. Unreal's lumen and candela units come with exposure, which this renderer does not have, so intensity is a plain multiplier.
- Up to 16 point lights per frame in one uniform buffer, enough for the miner's lamp and the generators; more is an error naming the count.
- Lighting is per pixel in the fragment shader.

## Tolerance presets

- Perspective edges, texture lookups and per-pixel lighting can differ by a few units per channel between adapters, so the 3D goldens need tolerance, as ADR 0002 says.
- Measurement: render every 3D golden shot on the four adapters (RTX 2050 and WARP locally, WARP and Mesa lavapipe in CI), compare every pair, and record per pair the largest channel difference, the global share of differing pixels and the worst 16 by 16 block, with and without the neighbour search. The table goes into `docs/research/tolerance-presets.md`.
- How `low`, `medium` and `high` are derived from that table is a criterion Felipe picks before any number is written. `zero` and custom tolerances stay as they are.

## MCP tool and scenarios

- `tickora_screenshot` accepts `viewport: "perspective"` with `location` `[x, y, z]`, `rotation` `[pitch, yaw, roll]` and `fov`, or a perspective `camera` string; `ortho_width` is rejected for perspective and `rotation` and `fov` for top. `view_mode` accepts `lit`. The result's `camera` is the string for the view drawn, and `off_screen` counts instances whose bounds lie fully outside the view frustum.
- The `Host` trait gains `fn scene3(&self, ...) -> Scene3`. The arena builds it as a test fixture: a floor and one meshkit cube per entity coloured by hp, with one arena unit as 25 cm so the 4-unit entity is a 1 m cube, and two point lights. These meshes and textures come from `tools/meshkit` and `psx_bake` specs under `specs/`, with the baked output committed under `assets/gen/` (ADR 0004).
- Scenario `screenshot` assertions take the same keys. `scenarios/arena.json` gains perspective shots in `lit`, `unlit` and `wireframe`; the existing top shots stay as the debug view.

## Tests

- Unit: camera string round trip and errors for both shapes; rotator to view matrix against hand-computed axes; reversed-Z projection of near, mid and far points; GLB reading of a committed meshkit fixture and the axis conversion; falloff values at 0, 1 m, half radius and the radius; argument errors in the MCP tool.
- Render: exact checks that hold on any adapter, at pixel centres well inside faces: a near cube hides a far one (depth), a textured quad shows the texel colour under `unlit`, and a surface lit by a near light is brighter than by a far one. Edge pixels are left to the goldens.
- Goldens: the arena scenario in debug and release, every 3D shot on both CI buckets.
- Every test runs locally in debug and in release with `CARGO_TARGET_DIR=target/verify`, then in CI.

## Order of commits

Each step is committed, pushed and checked in CI before the next; ROADMAP items are marked Done with evidence and CHANGELOG Unreleased is updated as each lands.

1. Camera: `Perspective`, rotator, the perspective string, projection and view matrices (item 2).
2. `gltf` dependency, GLB reader, axis conversion, mesh fixture made with meshkit (items 1 and 3).
3. Depth buffer and mesh drawing in `unlit` and `wireframe`; render tests (item 3).
4. Textures from a `psx_bake` fixture (item 4).
5. `lit` view mode and point lights (item 5).
6. `Host::scene3`, the arena fixture, MCP tool and scenario keys, `docs/screenshots.md` and `docs/scenarios.md` (item 7, first part).
7. Adapter measurement, Felipe's criterion, the `low`, `medium` and `high` presets (item 6).
8. 3D goldens recorded from CI and committed (item 7).

## Open values

- Eye height for the arena fixture's default camera: 170 cm, chosen by Felipe. Source: Epic's Lyra sample, capsule half-height 90 (`LyraCharacter.cpp:41`) plus `BaseEyeHeight` 80 (`LyraCharacter.cpp:79`), with `BaseEyeHeight` measured from the collision center (`Pawn.h`, UE 5.5). The coal-mine miner's eye height is set in phase 8.

## Not in this phase

Shadows, specular, normal maps, mipmaps and filtered sampling, anti-aliasing, skinned or animated meshes, `front` and `side` viewports, a window to play in, 3D collision (ADR 0003).
