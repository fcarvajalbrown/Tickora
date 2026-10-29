# Phase 8 design, item 3: spot lights with gobo textures

Status: approved by Felipe.

Today `Scene3` has point lights only, up to 16, dimmed with Unreal's Karis falloff. This item adds HPL3's spot light: a frustum with a field of view, an aspect and a near plane, an angular falloff with a power, and a gobo texture projected through the frustum, animated by ticks. It also moves point lights to HPL3's distance falloff, so both light types dim the same way before 8.4 adds shadows to both.

Under the CLAUDE.md rule, HPL3 comes first. Where Frictional's posts are silent, the evidence is the Bunker executable's decompile (`research-private/bunker/NOTES.md`, "Spot lights and light falloff") and the open source of HPL1 and HPL2, read for behaviour only. Unreal is listed for comparison.

## Sources checked

- Frictional Games, "Tech Feature: Light masking", 2010: the renderer draws "spheres for point lights and pyramids for spots", so a spot is a frustum, not a round cone.
- Frictional's HPL2 editor wiki, Lights page: a spot light has Radius, Near clip plane, FOV, Aspect ("FarClipPlane.width/FarClipPlane.height"), a Spot Falloff Map ("how the spot will attenuate over distance to the center"), a Falloff Map over distance, and a Gobo ("2D for SpotLights, CubeMap for PointLights").
- HPL2 source (`FrictionalGames/AmnesiaTheDarkDescent`, GPLv3, read only): FOV is the full vertical angle, default 60 degrees, aspect default 1; the frustum is `top = tan(FOV/2) * near`, `right = aspect * top`. The renderer binds the spot falloff map only when the light has no gobo, in the gobo's texture slot, and passes `1 - cos(FOV/2)` to the shader. The default distance falloff map is `core_falloff_linear`.
- HPL1 source (`FrictionalGames/HPL1Engine`, shaders under zlib): the spot's shape comes from one projected texture (`tex2Dproj` through the spot's view and projection), with a lookup that rejects points behind the light.
- Amnesia: The Bunker executable, names and decompile:
  - Spot light fields: FOV, Aspect, NearClipPlane, SpotFalloffPow, GoboTexture, GoboScale, GoboAnimMode, GoboAnimFrameTime, GoboAnimStartTime, GoboType (Diffuse or Specular), a cheap gobo flag. Animation modes are None, Loop and Oscillate.
  - Light defaults: falloff power 1.0. Spot defaults: FOV 60 degrees, aspect 1.0, near plane 0.1 m, radius 100 m, SpotFalloffPow 1.0.
  - Distance falloff in the CPU light-level code: `pow(max(1 - d / radius, 0), FalloffPow)`, linear at the default power, as HPL2's linear map.
  - Angular falloff, one of two CPU paths: `t = min((1 - cos theta) / (1 - cos(FOV/2)), 1)`, factor `pow(1 - sqrt(t), SpotFalloffPow)`, theta measured from the spot's forward axis. The other path measures the radius in the frustum's projected square. The GPU shaders are not in the executable, so which one the GPU runs is not seen; the cosine path uses the same quantity HPL2's shader receives.
- Unreal 5.5: spot lights are round cones with inner and outer cone angles; gobos come from light functions. Neither is used here.

## Choices

| Piece | HPL3 | Unreal 5.5 | Tickora (Felipe's choice) |
| --- | --- | --- | --- |
| Shape | frustum: FOV, aspect, near plane, radius | round cone, inner and outer angle | HPL3 frustum |
| FOV | full vertical angle, default 60 | half angles | full vertical angle, default 60 |
| Distance falloff | `pow(1 - d/r, FalloffPow)`, default 1 | inverse square with a window | HPL3's, for spots and point lights |
| Angular falloff | `SpotFalloffPow` on a cosine ratio | smooth step between cones | HPL3's cosine path, default power 1 |
| Gobo | projected 2D texture; replaces the angular falloff | light function material | projected texture, replaces the angular falloff |
| Gobo animation | None, Loop, Oscillate, frame time, start time | material time | the same three modes, in ticks |
| Test scene | n/a | n/a | a new lighting scene; the arena keeps its layout |
| Assets | n/a | n/a | generated only |

The FOV follows HPL rather than Tickora's camera, which takes a horizontal FOV from Unreal. The two never meet: the camera FOV frames the screen, the spot FOV shapes the light.

## Light model

All distances are Unreal centimetres, like the rest of `Scene3`. HPL's 0.1 m near plane is 10 cm, the same as the camera's near plane.

For a lit surface point `p` with normal `n`, each light adds `colour * intensity * lambert * distance * spot`:

- `lambert = max(dot(n, l), 0)` as today, `l` the unit vector from `p` to the light.
- `distance = pow(max(1 - d / attenuation_radius, 0), falloff_pow)`, `d` the distance in cm. This replaces Karis for point lights too.
- `spot` is 1 for a point light. For a spot light:
  - Without a gobo: `t = min((1 - cos theta) / (1 - cos(fov / 2)), 1)`, `spot = pow(1 - sqrt(t), spot_falloff_pow)`. Aspect plays no part, as in HPL2, where the renderer notes that aspect "might be wonky if there is no gobo".
  - With a gobo: `p` goes through the spot's view and projection. Behind the light, or outside the frustum's rectangle, `spot = 0`. Inside, `spot` is the gobo texel at the projected position, decoded with `pow(c, 2.2)` as a colour texture, per channel, so a gobo can tint the light. Sampling is nearest with clamp, until 8.5 adds filtering.
- The near plane builds the projection matrix, and 8.4 will reuse it for the spot's shadow map. It does not cut the light, since HPL draws the spot's pyramid from the light's position.

Both falloffs and the projection get an f64 CPU reference in a new `light` module in `tickora-render`, beside `tone`, so tests predict pixels. The shader holds the same formulas in f32.

## API changes

- `PointLight` gains `falloff_pow: f32`, default 1.0. `attenuation_radius` keeps its name, as Unreal naming already built stays.
- New `SpotLight { location, rotation: Rotator, color, intensity, attenuation_radius, falloff_pow, fov, aspect, near_plane, spot_falloff_pow, gobo: Option<GoboRef> }`. Rotation uses the existing `Rotator` (pitch, yaw, roll, Unreal order); the spot points along its forward axis. Defaults follow HPL3: fov 60, aspect 1, near plane 10 cm, both powers 1.0.
- `GoboRef { name, anim: GoboAnim, frame_ticks: u32, start_tick: u64 }` with `GoboAnim::{None, Loop, Oscillate}`. HPL3 times frames in seconds; Tickora counts ticks, at 60 per second, so a frame shows for a whole number of ticks.
- `Scene3` gains `spots: Vec<SpotLight>` (at most 16, as for point lights), `gobos: BTreeMap<String, Vec<Image>>` (one image per frame), and `tick: u64`, which picks the frame. Frame for tick `t`, with `n` frames and `k = (t - start_tick) / frame_ticks` (0 before the start tick):
  - None: frame 0.
  - Loop: `k mod n`.
  - Oscillate: `0, 1, ..., n-1, n-2, ..., 1`, then again; a period of `2n - 2` steps, or frame 0 when `n` is 1.
- Gobo frames go into one 2D texture array. Every frame of every gobo has the same size, a power of two from 16 to 256 per side, and the array holds at most 64 frames. `render3` rejects a missing gobo name, frames of mixed size, a fov outside 1 to 179, an aspect, near plane, radius or power that is not finite or not above 0, and more than 16 spots.
- MCP tool 8 `tickora_screenshot` metadata reports the counts of point lights, spots and gobo frames. Arguments are unchanged.

## The lighting scene

Felipe's choice: a new scene for lighting, so lighting items have their own room, and only generated assets.

- Host: `LightingHost` in `crates/tickora-mcp`, beside the arena. Its only state is the tick, which drives gobo animation; it takes no inputs and hashes the tick. Its scene is fixed: a closed room built from meshkit pieces, a few cubes to catch light at different distances, the lights below, and a default camera at eye height (170 cm) in a doorway.
- Scenario files gain an optional top-level `host` key, `"arena"` by default or `"lighting"`. The CLI and `tickora_run_scenario` both pick the host from it. The live MCP simulation stays the arena; the lighting scene is reached through scenarios only.
- Lights, each one testing one thing:
  1. A spot with no gobo, fov 60, spot falloff power 1: the round pool and its soft edge.
  2. The same spot with spot falloff power 4: a tighter pool, so the power is seen to act.
  3. A spot with aspect 2 and a static "window bars" gobo, thrown on a wall: projection, aspect and the frustum's hard rectangle.
  4. A spot with a four-frame "flicker" gobo on Loop and a twin on Oscillate, `frame_ticks` 6: animation.
  5. One point light with falloff power 2 beside one with power 1: the new distance falloff.
- Gobos: generated by extending `tools/checker_maps` with gobo patterns (window bars, a lamp cage grid, a flicker set of four brightness frames), written as PNGs into `assets/gen/lighting/` from a spec in `specs/lighting/`, as the generator rules ask. Room pieces come from meshkit and `interior_kit` the same way. Exposure 1.0 and white point 11.2 as in the arena; Felipe sets the light intensities after seeing the frames.
- `scenarios/lighting.json`: lit shots of each spot alone (aimed by camera string), the gobo spot on its wall, the animated gobo at ticks covering every frame of both modes, one overview, and one unlit and one wireframe overview for the geometry. All perspective, at `low`.

## The arena

The arena keeps its layout. Its two point lights switch from Karis to HPL3's falloff, so its lit frames change:

- Karis at intensity 30 and a 20 m radius gives about 1.1 at 5 m; the linear falloff gives 22.5 there, far past the white point. The arena's `LIGHT_INTENSITY` is set again, by Felipe, from frames rendered at a few candidate values, as exposure was in 8.2.
- The four lit goldens move. The three unlit, three wireframe and four top goldens must stay byte-identical, since none of them is lit.

## Goldens, recorded once

1. With the renderer, the lighting scene and the gobos in place, render the arena's four lit shots and every lighting shot locally, headless, into scratch copies of `scenarios/` (`CARGO_TARGET_DIR=target/verify` while the MCP server holds the release binary). Open them in paint.net for Felipe.
2. Felipe sets the arena's intensity and the lighting scene's intensities. Step 1 runs again until he is satisfied. Nothing is committed as a golden before that.
3. CI's "Render scenario screenshots" step records `scenarios/lighting.json` as well as `arena.json`.
4. Commit and push the code with the old arena goldens and no lighting goldens. CI fails on that commit, as expected, and still uploads `scenario-shots-Windows` and `scenario-shots-Linux`.
5. Download both, copy `windows_dx12.png` and `linux_vulkan.png` into the four lit arena folders and every lighting folder. No `.altN` files. Check the arena's unlit, wireframe and top shots in the artifacts against the committed files, byte for byte; if any differ, stop and tell Felipe.
6. Commit the goldens on their own with the run ID, push, and confirm CI is green on both platforms. Record the run IDs in the ROADMAP and session notes.

## Measuring the tolerance table again

Yes, once. The projection, the cosine ratio and two `pow` calls per light are new f32 maths that can round differently per adapter, and the gobo's nearest lookup can pick a neighbouring texel on a projected edge.

1. Record every perspective shot of both scenarios on both local adapters, `TICKORA_ADAPTER=hardware` (RTX 2050) and `software` (WARP).
2. CI frames are the artifacts from goldens step 5 (WARP on `windows-latest`, lavapipe on `ubuntu-latest`).
3. Run `tolerance-table` with `--shots perspective --sweep 1,2,4,8,16` over the four adapters, the same command as `docs/research/tolerance-presets.md`, once per scenario.
4. Results go in a new section, "Spot lights and gobos (phase 8 item 3)", with the CI run ID. Earlier tables stay.
5. The check is unchanged: at channel tolerance 16 with the neighbour search, every pair within block 0.10 and frame 0.02. If a pair fails, most likely on a gobo edge, stop and ask Felipe; presets are not changed without him.

## Tests

- `light` unit tests: distance falloff is 1 at the light and 0 at the radius, linear at power 1, and power 2 gives the square; the spot factor is 1 on the axis, 0 at and past fov/2, and power 4 gives the fourth power of power 1; the projection maps the axis to the gobo's centre and the frustum corners to its corners, rejects points behind the light; the frame index for None, Loop and Oscillate over a run of ticks, including before the start tick and a one-frame gobo.
- `render3`: a spot on a floor lights the centre and leaves a point past fov/2 at ambient; a two-texel gobo lights one half of the pool red and the other blue as the CPU reference predicts, within one unit; a gobo changes frame on the tick the reference says; a point light's pixel follows the new falloff. Every rejected input listed under API changes is an error.
- MCP and scenarios: the `host` key parses, an unknown host is a malformed scenario, unknown keys are still rejected; the lighting host is deterministic (same ticks, same hashes); screenshot metadata carries the light counts.
- Python: the gobo patterns are byte-stable, and `tests/tools` checks the committed PNGs.
- Goldens: both scenarios on both CI buckets; CI on Windows and Linux.

## Out of scope

- Shadows from spots and points, and the specular gobo type (8.4).
- Gobo scale and the cheap gobo flag: the executable names them, but nothing found says what they do.
- Cube-map gobos for point lights (HPL2's wiki), box lights, directional lights and light mask boxes.
- Filtered gobo sampling and mipmaps (8.5).
- A light-level query for the sim (the Bunker's CPU paths, for a monster seeing the player in light). That belongs to phase 9 and must stay in fixed point.

## Steps

Each step is committed and pushed before the next.

1. `light` module with its unit tests.
2. Renderer: HPL3 distance falloff for point lights, `SpotLight`, gobo texture array and frame choice, `Scene3` fields; `render3` tests.
3. Gobo patterns and room pieces from the generators, with specs and Python tests.
4. `LightingHost`, the scenario `host` key, `scenarios/lighting.json`, screenshot metadata, docs (`docs/scenarios.md`, `docs/screenshots.md`).
5. Felipe judges the frames and sets the intensities.
6. Goldens recorded from CI as above.
7. Tolerance table measured again and written up.
8. ROADMAP item 3 marked done with the run IDs; session notes updated.

## Sources

- Frictional Games, "Tech Feature: Light masking", 2010: https://frictionalgames.blogspot.com/2010/12/tech-feature-light-masking.html
- Frictional Games wiki, HPL2 level editor lights: https://oldwiki.frictionalgames.com/hpl2/tools/editors/lights
- HPL2 source: https://github.com/FrictionalGames/AmnesiaTheDarkDescent (`HPL2/core/sources/scene/LightSpot.cpp`, `HPL2/core/sources/graphics/RendererDeferred.cpp`)
- HPL1 source: https://github.com/FrictionalGames/HPL1Engine (`assets/core/programs/Diffuse_Light_Spot_fp.cg`, `Diffuse_Light_Spot_vp.cg`)
- Amnesia: The Bunker executable, decompile notes in `research-private/bunker/NOTES.md` (git-ignored)
