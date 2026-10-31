# Phase 8 design, item 4: spot light shadows and specular highlights

Status: approved by Felipe.

Today every light in `Scene3` passes through walls and crates, and every surface is matte: the lit shader adds Lambert diffuse and nothing else. This item adds HPL3's shadow maps for spot lights and HPL's Blinn-Phong specular for both light types.

The ROADMAP line asks for "shadows from point and spot lights". HPL3 casts no shadows from point lights, so following the CLAUDE.md rule, Felipe chose spot lights only, and the ROADMAP line changes to "Shadows from spot lights, and specular highlights" when this design is approved.

## Sources checked

- Amnesia: The Bunker executable, decompile and strings (`research-private/bunker/NOTES.md`, "Shadows and specular"):
  - The shadow-list gate returns "no shadow map" unless the light is a spot, its CastShadows is on and shadows are on globally. Point and box lights never get a map. The `samplerCubeShadow` names in the executable sit in the shader translator's table of every GLSL sampler type and say nothing about point lights; `docs/research/bunker-look.md` said otherwise and is corrected.
  - Light defaults: CastShadows off, ShadowMapResolution High, ShadowMapBiasMul 1.0, ShadowMapSlopeScaleBiasMul 1.0, ShadowDistance -1 (off). Each light has a SpecularColor and a CastSpecularLight flag; materials have a specular power.
  - Each shadow map is drawn with a rasterizer depth bias: a global bias times the light's bias multiplier, and a global slope-scale bias times its slope multiplier.
  - Shadow quality has three levels, Low, Medium and High, and is a graphics setting.
- HPL3's own documentation: the scripting API says CastShadows is "used only by spotlights (for now)"; the editor's Light EditMode page says spot lights "are also the only lights that can make objects cast shadows"; the materials page says a specular map holds RGB intensity and power in alpha. The indoor level performance page prefers a gobo to a shadow-casting light where one will do.
- HPL2 source (`FrictionalGames/AmnesiaTheDarkDescent`, GPLv3, read only): only spot lights cast shadows; Depth16 maps of 256, 512 and 1024 for Low, Medium and High; jitter maps of 32 by 32 texels with 32, 16 or 0 samples for High, Medium and Low; the jitter offsets come from a stratified grid warped onto a disk with `r = sqrt(v)`, `angle = 2 pi u`, sorted longest first and stored as bytes.
- HPL2's light shader, read for behaviour only from third-party copies of the shader the game ships (`roozerxc/atrim`, `deferred_light_frag_vanilla.glsl`):
  - Specular: `H = normalize(L + V)`, `power = exp2(a * 10) + 1` for a stored value `a` from 0 to 1 ("Range 0 - 1024"), `spec = intensity * pow(saturate(dot(H, N)), power) * light colour`. The light's output is `(spec + diffuse) * attenuation`, times the gobo when there is one. Specular is not multiplied by the surface colour or by `dot(N, L)`.
  - Shadow lookup: the receiver moves along its normal by `(1 - dot(N, L)) * 0.03` m before projecting. Low takes one compare. Medium and High read the jitter map at the screen pixel modulo 32, scale each offset by `blur / map size`, take 4 taps first and only take the rest when those 4 disagree.
- Frictional Games, "Tech Feature: Pre-pass lighting", 2010: the G-buffer holds specular power and the light buffer "color and specular intensity". The executable has no GPU shaders, so HPL3's exact formula is not seen; HPL3 makes intensity RGB where HPL2's was grey.
- Unreal 5.5 is not used here: HPL3 covers every piece of this item.

## Choices

| Piece | HPL3 | Tickora (Felipe's choice) |
| --- | --- | --- |
| Lights that cast | spot lights only, off by default | spot lights only, `cast_shadows` off by default |
| Map resolution | Low, Medium, High per light, default High | 256, 512, 1024, default High |
| Cap | none known | at most 4 shadow-casting spots per frame |
| Edge filter | jittered disk, 0, 16 or 32 samples | HPL2's jittered disk, built from a seeded PCG32, default Medium (16) |
| Bias | rasterizer bias, `glPolygonOffset(1.0 * slope mul, 0.5 * bias mul)` on a `GL_DEPTH_COMPONENT32` map | the same rasterizer bias on a Depth32Float map, constant rounded to a whole unit (step 5, Felipe's choice) |
| Specular model | not seen; HPL2 is Blinn-Phong | HPL2's Blinn-Phong with HPL3's fields |
| Material specular | specular map, RGB intensity, power in alpha | per-material values now, maps with normal maps in 8.5 |
| Light specular colour | separate SpecularColor, CastSpecularLight | the light's colour unless set, `cast_specular` on by default |
| Arena | n/a | unchanged |
| Test scene | n/a | a second room in the lighting scene; the first room's shots unchanged |

## Light model

All distances are Unreal centimetres. For a lit surface point `p` with normal `n`, `v` the unit vector from `p` to the camera, and per light `l` the unit vector to the light:

- Diffuse, as today: `colour * intensity * max(dot(n, l), 0) * distance * spot * shadow`.
- Specular, HPL2's shape: `specular_colour * intensity * material_specular * pow(max(dot(n, h), 0), power) * distance * spot * shadow`, with `h = normalize(l + v)`, `power = 2^(10 * material_power) + 1`. `specular_colour` is the light's own `specular_color` if set, otherwise its `color`; zero when `cast_specular` is off.
- Output: `vertex_colour * albedo * (ambient + sum of diffuse) + sum of specular`. The specular sum is not tinted by the surface colour, as in HPL2. `spot` already holds the gobo, so a gobo shapes the highlight as well, also as in HPL2.
- `shadow` is 1 for point lights and for spots that do not cast. For a casting spot it comes from its shadow map, below. Outside the spot's frustum, behind the light, or where the map holds nothing, `shadow` is 1.

### Shadow maps

- Each casting spot renders scene depth through its existing `view_projection` (reversed Z, infinite far plane, the spot's near plane), so a stored value is `near_plane / d` with `d` the depth along the spot's forward axis.
- The lit shader moves the receiver first: `p' = p + n * (1 - dot(n, l)) * SHADOW_NORMAL_OFFSET_CM * shadow_slope_bias_mul`, with `SHADOW_NORMAL_OFFSET_CM = 3` from HPL2's 0.03 m. HPL2 skips this move when the spot has a gobo, because that path reuses the gobo's projection; Tickora always applies it.
- A tap at texel `t` is lit when `dot(p' - location, forward) <= near_plane / stored(t)`. Changed at step 5 from a centimetre bias in this compare to HPL3's rasterizer bias on the shadow pass: `DepthBiasState { constant: -round(0.5 * shadow_bias_mul), slope_scale: -1.0 * shadow_slope_bias_mul }`, negative because the map is reversed Z, rounded half away from zero because wgpu and D3D12 take an integer constant. wgpu fixes the bias per pipeline, so the renderer builds one shadow pipeline per distinct bias the first time a spot needs it. The f64 reference applies D3D12's float-depth rule per plate: `stored + constant * 2^(exponent(max stored) - 23) + slope_scale * max(|dz/dx|, |dz/dy|)`, clamped to 0..1.
- Texels are read with `textureLoad`, nearest, at `floor(uv * side + offset * SHADOW_BLUR)`, clamped to the map. `SHADOW_BLUR` is HPL2's per-light blur amount. The Bunker executable has no such name, so Tickora keeps one constant, set at step 5.
- Quality, as HPL2 on shader model 4:
  - Low: one tap, no offset.
  - Medium and High: N = 16 or 32 taps. The first 4 taps are averaged; if that average is 0 or 1 it is the result. Otherwise the result is `(sum of the first 4) / N + (sum of the other N - 4) / N`, HPL2's early out written without its division order trick.
- Jitter map: HPL2's `GenerateScatterDiskMap2D`, for a 32 by 32 screen tile and N samples. Per texel, cells of a `g` by `g` grid (`g = round(sqrt(N))`) are picked with HPL2's stride, each moved by a random amount from -0.5 to 0.5 of a cell, warped to the disk, sorted longest first, and stored as bytes `floor((x + 1) / 2 * 255)`. The random numbers come from tickora-core's PCG32 with a fixed seed, as HPL2's come from its own generator. The map is built on the CPU once per quality, so the CPU and GPU read the same bytes. Pixel `(x, y)` reads tile texel `(x mod 32, y mod 32)`; each texel holds two offsets per tap pair.

## API changes

- `SpotLight` gains `cast_shadows: bool` (default false), `shadow_resolution: ShadowResolution` (`Low`, `Medium`, `High` = 256, 512, 1024 per side, default `High`), `shadow_bias_mul: f32` and `shadow_slope_bias_mul: f32` (default 1.0), `specular_color: Option<[f32; 3]>` (default `None`) and `cast_specular: bool` (default true).
- `PointLight` gains `specular_color` and `cast_specular`, with the same defaults.
- New `Material { specular: [f32; 3], specular_power: f32 }`, where `specular_power` is HPL's 0 to 1 value. `Scene3` gains `materials: BTreeMap<String, Material>`. A material with no entry has no specular, like an HPL3 material without a specular map.
- `Display` gains `shadow_quality: ShadowQuality` (`Low`, `Medium`, `High`, default `Medium`), since HPL3 keeps quality in the player's graphics settings, as gamma is. `tickora_screenshot` and scenario frames take `shadow_quality` (`"low"`, `"medium"`, `"high"`), perspective only, and the result reports it with the count of shadow-casting spots.
- `render3` rejects more than 4 casting spots, a bias multiplier that is not finite or below 0, a material specular that is not finite or below 0, and a `specular_power` outside 0 to 1. `cast_shadows` and the materials do nothing in `unlit` and `wireframe`.
- `light` gains the f64 reference: `specular_power`, `blinn_phong`, `shadow_receiver`, `jitter_map`, and `shadow_factor` over a depth map held as f64.

## Frame uniform and bind groups

The device keeps `wgpu::Limits::downlevel_defaults()`. Today the frame uniform is 2,912 bytes and the mesh pipeline uses 2 of 4 bind groups.

| Piece | Today | After |
| --- | --- | --- |
| Frame header | view-projection 64, ambient 16, counts 16 | view-projection 64, camera position 16, ambient 16, counts 16 (lights, spots, shadow taps, 0) |
| `Light` | 48 bytes (position and radius, colour and intensity, falloff) | 64: adds the specular colour, zero when off |
| `Spot` | 128 bytes | 160: adds the specular colour and a shadow vector (map layer or -1, map side, normal offset in cm, spare); the near plane goes in the spare slot of `falloff_cone_layer` |
| Frame uniform | 2,912 bytes | 64 + 48 + 16 x 64 + 16 x 160 = 3,696 bytes, under the 16,384-byte limit |
| Group 0 | frame uniform, gobo array | adds the shadow maps (`texture_depth_2d_array`, `textureLoad`) and the jitter map (`texture_2d<f32>`, `textureLoad`) |
| Group 1 | albedo, sampler | adds one 16-byte uniform per batch: material specular and power |

- Sampled textures per fragment stage go from 2 to 4 (limit 16), uniform buffers from 1 to 2 (limit 11), samplers stay at 1, bind groups at 2 of 4.
- Shadow maps are one `Depth32Float` array, one layer per casting spot, sized to the largest resolution in the frame. A smaller map draws into the top-left corner through the viewport, and the shader reads it at its own side. At most 4 layers at 1024, 16 MB. With no casting spot, a 1 by 1 layer is bound, as the gobo array binds a white frame today.
- A depth-only pipeline draws each map from the same triangle batches, with its own 64-byte uniform for the spot's view-projection, culling off as in the main pass, clear 0, compare Greater. It runs in the same encoder before the geometry pass, in `lit` only.

## The lighting scene

Felipe's choice: a second room in the same `LightingHost`, so the first room's 13 shots are a check that nothing changed.

- Placed 20 m along x through `specs/lighting/placements.json`: the same meshkit room, the existing crate, and a new `crate_gloss` piece in `specs/lighting/pieces.json`, two copies with the materials `gloss_dull` and `gloss_sharp`. The first room's lights reach at most 6 m, so none touches the second room.
- Lights, each testing one thing:
  1. A shadow-casting spot, High, over a crate on the floor: umbra, penumbra and the normal offset on a flat floor.
  2. A shadow-casting spot with the bars gobo, through a crate onto a wall: shadow and gobo together, as HPL3 allows.
  3. A Low-resolution casting spot beside spot 1: the coarser map is visible.
  4. A point light and a spot over the two gloss crates: specular from both light types, at two powers.
- Materials: `gloss_dull` and `gloss_sharp` get specular values that Felipe sets at step 5. Everything else stays without specular.
- New shots in `scenarios/lighting.json`, all perspective at `low`: an overview of the second room in lit, unlit and wireframe; the floor shadow at Medium, and again at `shadow_quality` low and high; the gobo shadow on the wall; the Low and High maps side by side; the specular crates under the point light and under the spot.

## The arena

Unchanged, Felipe's choice. It has only point lights and no material table, so all 14 arena shots must stay byte-identical, and CI's Python audio check, which runs `arena.json` first, is unaffected.

## Goldens, recorded once

1. With everything in place, render every new lighting shot locally, headless, into scratch copies of `scenarios/` (`CARGO_TARGET_DIR=target/verify` while the MCP server holds the release binary). Open them in paint.net for Felipe.
2. Felipe sets the bias, `SHADOW_BLUR`, the gloss materials and the second room's light intensities. Step 1 runs again until he is satisfied. Nothing is committed as a golden before that.
3. Commit and push the code with no goldens for the new shots. CI fails on that commit, as expected, and uploads `scenario-shots-Windows` and `scenario-shots-Linux`.
4. Download both and copy `windows_dx12.png` and `linux_vulkan.png` into each new folder. No `.altN` files. Compare the 14 arena shots and the 13 first-room shots in the artifacts against the committed files, byte for byte. If any differ, stop and tell Felipe.
5. Commit the goldens on their own with the run ID, push, and confirm CI is green on both platforms. Record the run IDs in the ROADMAP and session notes.

## Measuring the tolerance table again

Yes, once, as in 8.3. The new f32 maths (the depth compare, the jitter offsets, `pow` with powers up to 1025) can round differently per adapter, and one tap that flips on a penumbra edge moves a pixel by up to 1/16 of the light.

1. Record every perspective shot of both scenarios on `TICKORA_ADAPTER=hardware` (RTX 2050) and `software` (WARP).
2. CI frames are the artifacts from goldens step 4 (WARP on `windows-latest`, lavapipe on `ubuntu-latest`).
3. Run `tolerance-table` with `--shots perspective --sweep 1,2,4,8,16` over the four adapters, once per scenario.
4. Results go in a new section of `docs/research/tolerance-presets.md`, "Shadows and specular (phase 8 item 4)", with the CI run ID.
5. The check is unchanged: at channel tolerance 16 with the neighbour search, every pair within block 0.10 and frame 0.02. If a pair fails, most likely on a penumbra or a sharp highlight, stop and ask Felipe; presets do not change without him.

## How the shader is checked against the f64 reference

- `light` unit tests: `specular_power` gives 2 at 0 and 1025 at 1; `blinn_phong` is 1 at the mirror direction and falls with the angle, faster at higher power; `shadow_receiver` does not move a point facing the light and moves a grazing one by the full offset; `jitter_map` is byte-stable for its seed, every offset is inside the unit disk, and each texel's taps run longest first; `shadow_factor` gives 1 with nothing in front, 0 under a full occluder, Low takes one tap, and the 4-tap early out returns the same value as all N taps whenever those 4 agree.
- `render3`, through the shared renderer as today:
  - A casting spot over a floor with a raised square occluder. The reference computes the stored depth of each map texel exactly, by casting the texel centre against the two planes, and predicts every pixel's shadow factor. Umbra pixels are ambient only, lit pixels match the reference within one unit, and the reference's penumbra pixels match within one unit on at least 99% of them; the rest are texels the occluder's edge crosses, where rasterization may round either way.
  - The same frame with `cast_shadows` off is byte-identical to the frame without the occluder's shadow, and a Low spot beside a High one each match their own reference.
  - A floor under a point light, seen at the mirror angle: the highlight pixel matches the reference within one unit; with no material entry, or `cast_specular` off, the frame is byte-identical to today's; a `specular_color` tints the highlight as the reference says.
  - Every rejected input under API changes is an error.
- MCP and scenarios: `shadow_quality` parses, is rejected for `top`, and comes back in the metadata with the casting-spot count. The lighting host is still deterministic.
- Python: the new piece and placements are byte-stable, and `tests/tools` checks the committed meshes.
- Goldens: both scenarios on both CI buckets, with the byte-identical check in goldens step 4.

## Change at step 5: HPL3's shadow filter

Felipe approved this after the shadow acne at step 5. It replaces the jitter disk, the blur and the 3 cm offset above with HPL3's spot path from its shipped `deferred_light_frag.hpsl` and `helper_shadow_filter.hpsl`, read for behaviour (notes in `research-private/bunker/NOTES.md`). Where this section and the text above disagree, this section holds.

- Map depth: each casting spot renders through a standard `[0, 1]` projection with near the spot's near plane and far its radius, cleared to 1, compare Less. A stored value is the same window depth HPL3 stores, `(f / (f - n)) * (1 - n / d)`, so HPL3's constants apply unchanged. The map stays `Depth32Float`, since wgpu has no 32-bit unorm depth.
- Raster bias: HPL3's `glPolygonOffset(1.0 * slope mul, 0.5 * bias mul)`, rounded half away from zero, so `DepthBiasState { constant: +round(0.5 * shadow_bias_mul), slope_scale: +1.0 * shadow_slope_bias_mul }`, positive now that depth grows away from the light. One pipeline per distinct bias, as before.
- Receiver: `p' = p + n * (1 - dot(l, n)) * 0.5` cm (HPL3's 0.005 m), projected through the spot's shadow projection and divided by `|w|`.
- Per texel: `occlusion = clamp(1 + (stored - z) * k, 0, 1)`, with `k = sqrt(radius_m - near_m) * ((side / 14.4 + 80 / 9) / near_m)`, near and radius in metres and `side` the map's side in texels.
- Filters, all from 2x2 gathers around `floor(uv * side - 0.5) + 0.5` with `f = fract(uv * side - 0.5)`: Low `PCF2x2`, one gather mixed bilinearly by `f`; Medium `PCF4x4`, 4 gathers at offsets of plus or minus 1 texel weighted into a 3x3-texel tent, sum / 9, then `a * a * (3 - 2a)`; High `PCF6x6`, 9 gathers at offsets -2, 0, 2, sum / 25, then the same smoothstep.
- Removed: the jitter maps and their binding, `SHADOW_BLUR`, the tap count in the frame header, `jitter_map` in the reference.
- Test bar: unchanged, Felipe's nudge rule (every pixel within one unit unless a 0.001 cm nudge of its receiver changes the reference by more than one unit, such pixels at most 5% of the checked penumbra).

## Out of scope

- Point-light shadows: HPL3 has none. Its levels fill with unshadowed point and box lights and stop leaks with light mask boxes, a separate item if Felipe wants it.
- Specular maps, with normal maps in 8.5. The gobo's specular type and specular irradiance (8.10).
- Shadow distance and the distance-based drop in resolution (HPL3's ShadowDistance and render-settings thresholds): screenshots are taken from one camera, and the defaults are off.
- Static shadow maps rendered once, shadow-caster masks, terrain shadows, translucency: performance or features of a real-time game loop.
- Box and directional lights, and the directional light's cascades.

## Steps

Each step is committed and pushed before the next.

1. `light` reference with its unit tests.
2. Renderer: the new light and material fields, `Display.shadow_quality`, the jitter maps, the shadow pass, the frame uniform and bind groups, the shader; `render3` tests.
3. The second room and `crate_gloss` from the generators, with specs and Python tests.
4. `LightingHost`'s second room, its lights and materials, the new shots in `scenarios/lighting.json`, `shadow_quality` in screenshots and scenarios, metadata, docs (`docs/scenarios.md`, `docs/screenshots.md`).
5. Felipe judges the frames and sets the bias, the blur, the gloss materials and the intensities.
6. Goldens recorded from CI as above.
7. Tolerance table measured again and written up.
8. ROADMAP item 4 marked done with the run IDs; session notes updated.

## Sources

- Frictional Games, "Tech Feature: Pre-pass lighting", 2010: https://frictionalgames.com/2010-10-tech-feature-pre-pass-lighting/
- Frictional Games, "Tech feature: Light Masking", 2010: https://frictionalgames.com/2010-11-tech-feature-light-masking/
- Frictional wiki, HPL3 Light scripting API: https://wiki.frictionalgames.com/page/HPL3/SOMA/Scripting/Scripting_Api/Light
- Frictional wiki, HPL3 Light EditMode: https://wiki.frictionalgames.com/page/Hpl3:Tools:maineditors:common:light_editmode
- Frictional wiki, HPL3 indoor level performance: https://wiki.frictionalgames.com/page/HPL3/SOMA/Level_Design/Indoor_Level_Performance
- Frictional wiki, HPL3 materials: https://oldwiki.frictionalgames.com/hpl3/engine/materials
- HPL2 source: https://github.com/FrictionalGames/AmnesiaTheDarkDescent (`HPL2/core/sources/graphics/RendererDeferred.cpp`, `HPL2/core/sources/graphics/TextureCreator.cpp`)
- HPL2 light shader, third-party copy of the shipped file, read for behaviour only: https://github.com/roozerxc/atrim (`patches/core/shaders/deferred_light_frag_vanilla.glsl`)
- Amnesia: The Bunker executable, decompile notes in `research-private/bunker/NOTES.md` (git-ignored)
