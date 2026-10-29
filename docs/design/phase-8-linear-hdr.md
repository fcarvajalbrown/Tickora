# Phase 8 design, item 2: linear space and HDR

Status: approved by Felipe.

Today the phase 6 renderer lights in gamma space. It multiplies 8-bit texture bytes by light values and writes the result straight to an `Rgba8Unorm` target, with no decode, no HDR buffer and no tone mapping (`docs/research/bunker-look.md`, "Tickora today"). This item switches 3D rendering to HPL3's pipeline: decode colour to linear, light into a 16-bit float buffer, apply exposure and Hable's curve with a white point, and encode once at the end.

Under the CLAUDE.md rule (HPL3 first, Unreal as a would-like and the reference where HPL3 is silent), every choice below starts from HPL3. Unreal's choices are listed for comparison.

## Sources checked

- Frictional Games, "Tech Feature: HDR Lightning", 2012. Code: `Uncharted2Tonemap(scene_color * exposure) / Uncharted2Tonemap(white_point)` with A 0.15, B 0.50, C 0.10, D 0.20, E 0.02, F 0.30. Exposure is set by hand, "dividing the level into areas that have different exposures". Bloom comes before tone mapping, with a bright pass at the white point. The post gives no number for the white point or the exposure.
- Frictional Games, "Tech Feature: Linear-space lighting", 2013. Textures are decoded either through the sRGB texture format or with `pow(texture, 2.2)` in the shader. Diffuse, specular and ambient occlusion textures are decoded. Normal, displacement and alpha maps and the alpha channel are not. The output is encoded last, "after doing all calculations and post-processing", as `pow(final_linear_color, 1.0 / monitor_gamma)`. The post says "for most monitors a gamma of 2.2 would work fine", that the player should pick gamma from a calibration chart, and that monitor gamma is separate from the 2.2 used to decode textures.
- The white point. Four web searches and both posts give no Frictional value. 11.2 is the default in third-party implementations of Hable's curve (colour-hdri's `tonemapping_operator_filmic`, rayimage's `render_tonemap`). Those are secondary sources, not Hable's own text.
- Unreal 5.5, read from the install at `D:/EpicGames/UE_5.5`:
  - `Texture.h`: a per-texture `SRGB` flag, "Whether Texture and its source are in SRGB Gamma color space", for 8-bit and compressed formats.
  - `BaseScalability.ini`: `r.SceneColorFormat=3` at the lower effects levels and `4` at `EffectsQuality@3`. What each number means lives in engine source that the install doesn't ship, so it wasn't checked.
  - `TonemapCommon.ush`: the filmic curve with `FilmSlope`, `FilmToe`, `FilmShoulder`, `FilmBlackClip` and `FilmWhiteClip`. Unreal 5.0's Python API lists the defaults as 0.88, 0.55, 0.26, 0.0 and 0.04 (`FilmStockSettings`).
  - `Scene.h`: `EAutoExposureMethod` with Histogram, Basic and Manual ("Uses camera settings"). `AutoExposureBias` is a "Logarithmic adjustment for the exposure", where -1 is twice as dark. The engine's default method is set in source that wasn't available, so it is unverified.
  - `GammaCorrectionCommon.ush` and `PostProcessTonemap.usf`: sRGB output runs the exact piecewise curve (`LinearToSrgb`, 12.92 below 0.00313067, then `1.055 * pow(x, 1/2.4) - 0.055`). A comment says iOS output is "raw gamma 2.2 not sRGB".

## Choices

| Piece | HPL3 | Unreal 5.5 | Tickora (Felipe's choice) |
| --- | --- | --- | --- |
| Colour decode | sRGB texture format or `pow(c, 2.2)` | per-texture `SRGB` flag, hardware decode | `pow(c, 2.2)` in the shader, a fixed content gamma |
| Light buffer | 16-bit float RGBA, alpha holds SSAO | `r.SceneColorFormat` per quality level | `Rgba16Float`, alpha kept for 8.7's SSAO |
| Tone curve | Hable, divided by the curve at a white point | ACES-based filmic | Hable with HPL3's constants |
| White point | a value, number not published | none (white clip) | scene field, default 11.2 |
| Exposure | linear multiplier set by hand per area | auto by default, or manual in stops | linear multiplier per scene, default 1.0 |
| Encode | `pow(c, 1/monitor_gamma)`, picked by the player | exact sRGB curve | `pow(c, 1/gamma)`, a display setting, default 2.2 |

The decode is `pow(c, 2.2)` rather than the sRGB format because the encode is a 2.2 power. sRGB decode followed by a 2.2 encode lifts the darks (byte 5 comes out 13, 10 comes out 18, 20 comes out 27), and unlit would no longer show authored colours. With 2.2 on both sides, an unlit texel comes back within one unit at the default gamma. The cost: once 8.5 adds filtered sampling, filtering runs on encoded values. Sampling is nearest today, so nothing changes yet.

## Pipeline

3D frames only. The 2D `Scene` and the top view stay as they are: `Rgba8Unorm`, no decode, no encode. They remain the debug view, and their four `zero` goldens must stay byte-identical.

1. Geometry pass into an `Rgba16Float` colour target the size of the frame, plus the same `Depth32Float` depth. The clear colour is the scene background decoded with `pow(c, 2.2)`.
2. Decode in the mesh shader. The texel's RGB goes through `pow(texel, 2.2)`. Its alpha is not decoded. Everything else is linear already: light colours, intensity, ambient, glTF `baseColorFactor` and vertex colours (glTF 2.0 defines both as linear), and `MeshInstance::color`.
3. Lighting is unchanged in form (Lambert plus ambient, Karis falloff), now on linear values. `fs_lit` writes linear RGB with alpha 1 into the float buffer. `fs_unlit`, used by unlit and wireframe, writes the decoded linear base colour.
4. Tone-map pass: one full-screen triangle reads the float buffer with `textureLoad` (no filtering, no sampler) and writes the existing `Rgba8Unorm` target, which is read back as today.
   - Lit: `c = hable(colour * exposure) / hable(white_point)`, then `saturate(c)`, then `pow(c, 1 / gamma)`.
   - Unlit and wireframe: `saturate(colour)` then `pow(c, 1 / gamma)`. No exposure and no curve, so these views still check authored colour.
   - `hable(x) = (x * (A * x + C * B) + D * E) / (x * (A * x + B) + D * F) - E / F` with HPL3's constants.
5. Later phase 8 items slot into this order: bloom (8.8) between the geometry pass and the curve, as HPL3 requires; film grain and other post effects before the encode.

What the curve does to the numbers, at exposure 1.0, white point 11.2 and gamma 2.2 (from a Python check of the formulas above): linear 0.05 now shows as byte 42 where the old pipeline wrote 13, 0.25 as 86 (old 64), 0.5 as 115 (old 128), 1.0 as 148 (old 255), 4.0 as 219, and 11.2 reaches 255. Darks come up and highlights roll off instead of clipping. In the lit view the arena background (24, 24, 28) goes through the curve and becomes about 16.

## API changes

- `Scene3` gains `exposure: f32` (linear multiplier, default 1.0) and `white_point: f32` (default 11.2). `render3` rejects either if not finite or not above 0.
- A new `Display { gamma: f32 }`, `Default` gamma 2.2, passed to `render3` beside the view mode. Gamma must be finite and above 0. It is a display setting, never level data: a player's calibration, later from a settings menu.
- A `tone` module in `tickora-render` holds the CPU reference in f64: `decode`, `hable`, `tone_map`, `encode`. The tests use it to predict pixels. The shader holds the same formulas in f32.
- MCP tool 8 `tickora_screenshot` gains an optional `gamma` (default 2.2), and its metadata reports `gamma`, `exposure` and `white_point`. The scenario `screenshot` assertion gains the same optional `gamma` key. `docs/screenshots.md` and `docs/scenarios.md` are updated.
- Arena: new constants `EXPOSURE` (1.0) and `WHITE_POINT` (11.2) in `demo.rs`, and `hp_color_linear` decodes its bytes with `pow(c, 2.2)` so a cube reads as the same colour as its top-view quad. Light values stay as they are (ambient 0.15, two lights at intensity 30, radius 20 m), by Felipe's choice.

Simulation state, replay, ECS, noise and audio hashes don't change: rendering reads the sim and never writes it.

## Re-recording the goldens, once

All ten perspective shots in `scenarios/arena.json` move: the four lit ones through the curve, and the three unlit and three wireframe ones through decode and encode. The four top shots must not move. Each golden is recorded once, after Felipe has seen the lit frames.

1. With the renderer change in place, render the four lit shots locally, headless, into a scratch copy of `scenarios/` (`tickora-mcp scenario --record <copy>/arena.json`, with `CARGO_TARGET_DIR=target/verify` while the MCP server holds the release binary). Give Felipe the PNG paths.
2. Felipe looks at them. If they read too dark or too bright, he sets `EXPOSURE` and step 1 runs again. No golden is committed before he is satisfied.
3. Commit and push the code with the old goldens. CI's golden check fails on that commit, as expected. The "Render scenario screenshots" step runs `if: always()`, so it still uploads `scenario-shots-Windows` (WARP, dx12) and `scenario-shots-Linux` (lavapipe, Vulkan) from the new renderer.
4. `gh run download <run id>` both artifacts and copy `windows_dx12.png` and `linux_vulkan.png` over the ten perspective folders. No `.altN` files are added. Check that the four top goldens in the artifacts are byte-identical to the committed ones; if any differ, stop and tell Felipe.
5. Commit the goldens as their own commit with the run ID in the message, push, and confirm CI goes green on both platforms with `low` on every perspective shot. Record the run IDs in the ROADMAP and session notes.

## Measuring the tolerance table again

The `low` preset (16 per channel, block 0.10, frame 0.02, neighbour search) was justified by the phase 6 measurement. The float buffer, the f32 curve and the `pow` encode can round differently on each adapter, so the table is measured again before 8.2 is done:

1. Record every shot on both local adapters into copies of `scenarios/`: `TICKORA_ADAPTER=hardware` (RTX 2050) and `software` (WARP).
2. CI frames are the artifacts from step 5 above (WARP on `windows-latest`, lavapipe on `ubuntu-latest`).
3. `cargo run --release -p tolerance-table -- --shots perspective --sweep 1,2,4,8,16 rtx2050=<dir> warp-local=<dir> warp-ci=<dir> lavapipe-ci=<dir>`, the same command and the same four adapters as in `docs/research/tolerance-presets.md`.
4. The results go in a new section of `docs/research/tolerance-presets.md`, "Linear space and HDR (phase 8 item 2)", with the CI run ID. The phase 6 tables stay as they are.
5. The check is the existing criterion: at channel tolerance 16 with the neighbour search, every pair stays within block 0.10 and frame 0.02. If any pair fails, stop and ask Felipe. Presets are not changed without him.

## Tests

- `tone` unit tests: `hable(0) = 0`; `tone_map(white_point) = 1`; monotonic over 0 to 64; every byte 0 to 255 decodes and encodes back to itself at gamma 2.2; an exposure of 2 equals doubling the input.
- `render3`: `lit_ambient_alone_scales_the_colour` now expects the value from `tone::tone_map` for ambient 0.25 (byte 86 at the defaults), within one unit. The unlit texel checks (`[200, 40, 40]`, `[40, 40, 200]`, white, background) keep their values, within one unit. New: exposure 2 brightens a lit face; a different gamma changes unlit pixels as `tone::encode` predicts; a light far brighter than the white point saturates at 255 instead of wrapping. Exposure, white point or gamma that is zero, negative or not finite is an error.
- MCP: `gamma` reaches the frame and the metadata; an invalid gamma is `isError: true`; the scenario `gamma` key parses, and unknown keys are still rejected.
- Goldens: the arena scenario on both CI buckets, top shots unchanged at `zero`, perspective shots at `low`.
- CI on Windows and Linux.

## Out of scope

- Exposure areas with fades from one value to another (HPL3, and the Bunker executable's exposure areas and fade time). They need volumes in the sim and fades tied to ticks. This design proposes a ROADMAP line for them; Felipe places it.
- Eye adaptation. HPL3 has none.
- A calibration chart or settings menu for gamma.
- Changing the arena's light values.
- The sRGB texture format and filtered sampling (8.5), bloom and film grain (8.8), SSAO in the buffer's alpha (8.7).

## Steps

Each step is committed and pushed before the next.

1. `tone` module with its unit tests.
2. Renderer: `Rgba16Float` buffer, decode in the mesh shader, tone-map pass, `Display`, `Scene3` fields; `render3` tests updated and added.
3. MCP and scenarios: `gamma` argument and key, metadata, arena constants, `hp_color_linear` decode, docs.
4. Felipe judges the four lit frames; `EXPOSURE` is set if he asks.
5. Goldens re-recorded from CI as above.
6. Tolerance table measured again and written up.
7. ROADMAP item 2 marked done with the run IDs; session notes updated.

## Sources

- Frictional Games, "Tech Feature: HDR Lightning", 2012: https://frictionalgames.blogspot.com/2012/09/tech-feature-hdr-lightning.html
- Frictional Games, "Tech Feature: Linear-space lighting", 2013: https://frictionalgames.blogspot.com/2013/11/tech-feature-linear-space-lighting.html
- colour-hdri, `tonemapping_operator_filmic`: https://colour-hdri.readthedocs.io/en/latest/generated/colour_hdri.tonemapping_operator_filmic.html
- rayimage, `render_tonemap`: https://rdrr.io/cran/rayimage/man/render_tonemap.html
- Unreal Engine 5.0 Python API, `FilmStockSettings`: https://docs.unrealengine.com/5.0/en-US/PythonAPI/class/FilmStockSettings.html
- Epic, "Color Grading and the Filmic Tonemapper": https://dev.epicgames.com/documentation/en-us/unreal-engine/color-grading-and-the-filmic-tonemapper-in-unreal-engine
- Unreal 5.5 install: `Engine/Source/Runtime/Engine/Classes/Engine/Texture.h`, `Engine/Source/Runtime/Engine/Classes/Engine/Scene.h`, `Engine/Config/BaseScalability.ini`, `Engine/Shaders/Private/TonemapCommon.ush`, `Engine/Shaders/Private/GammaCorrectionCommon.ush`, `Engine/Shaders/Private/PostProcessTonemap.usf`
