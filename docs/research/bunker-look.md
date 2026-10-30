# Amnesia: The Bunker's look

Phase 8 item 1. Question: what makes up Amnesia: The Bunker's look, and which of those pieces Tickora's renderer lacks, so the rest of phase 8 can be fixed.

## Method and limits

- Frictional's own HPL3 tech posts, written for SOMA (2012 to 2014). The Bunker runs on a later HPL3 build that the fan wiki calls "HPL 3.5", with a "likely" hedge; GameGPU says the same and names OpenGL as the default API. No Frictional post or talk on the Bunker's rendering was found, so what the SOMA posts say is HPL3's design, not proof of the Bunker's exact settings.
- A text search over the Bunker executable Felipe owns, for feature names (details in the git-ignored binary notes). It shows which features and settings the engine carries, not how they are tuned in the game's levels. No decompiled code was read for this.
- A settings guide that names four menu toggles.

## Findings

1. **Pre-pass lighting.** HPL3 replaced HPL2's deferred shading with light pre-pass: a first pass stores normals, depth and specular power; lights then add colour and specular intensity into a light buffer; a second geometry pass reads that buffer, so each material can run its own final shading. Cost grows with objects plus lights, not objects times lights; one test level used about 30 lights. (Frictional 2010.)
2. **Linear space.** Colour textures (diffuse, specular, ambient occlusion) are decoded to linear on load, ideally through the sRGB texture format; normal, height and alpha maps are not decoded. The output is encoded once, after lighting and post effects, with the monitor's gamma, which the player calibrates. Doing only one of the two looks worse than doing neither. (Frictional 2013.) The executable has a gamma correction setting.
3. **HDR and filmic tone mapping.** Lighting goes into a 16-bit float RGBA buffer, its alpha holding SSAO. Tone mapping is John Hable's Uncharted 2 curve (A 0.15, B 0.50, C 0.10, D 0.20, E 0.02, F 0.30), divided by the curve at a white point. Exposure is set by hand per area of a level, darker areas raised, with no eye adaptation. Bloom is a bright pass over the white point, blurred and added before tone mapping. (Frictional 2012.) The executable keeps a tone-mapping post effect with key, exposure, white cut and fade time, and exposure areas that fade from one value to another, so per-area exposure carries on in the Bunker.
4. **SSAO.** Half resolution, 16 samples in a growing circle, 1.5 m radius, a bilateral blur that rejects neighbours more than 2 cm apart in depth, then a temporal pass reprojected with the previous view matrix and a depth-aware upsample: 1.1 ms at 1080p on an AMD 5870. It supplies medium-frequency occlusion; high-frequency occlusion is baked into the diffuse textures. (Frictional 2014.) The Bunker has SSAO on or off and a quality setting.
5. **Lights and shadows.** Four light types: point, spot, box and directional, the directional one with shadow slices (cascades). Spot lights can project a gobo texture, animated or cheap. Light mask boxes limit where light reaches. Shadows have low, medium and high resolution and quality, a shadow distance, and per-light casting and bias. Only spot lights cast shadows: the Bunker's shadow setup skips every other light type, and HPL3's scripting API says casting is "used only by spotlights (for now)". An earlier reading of cube shadow maps for point lights came from GLSL type names in the shader translator and was wrong. The menu has a flashlight shadows toggle.
6. **Indirect light.** Irradiance probes in sets that fade in, a specular irradiance option, separate sky and ground ambient colours, environment maps and a skybox. Thomas Grip said SOMA's global illumination was limited to spherical harmonics (GamingBolt's paraphrase, not a quote).
7. **Fog.** Distance fog with start, end, density, falloff exponent, height density and horizon, and brightness; noise that moves with the camera and turbulence; a second fog layer with its own settings; fog area volumes; a fog skybox.
8. **Materials.** Parallax mapping, simple and relief, with an on/off and quality setting; translucency; decals, including dynamic ones in a ring buffer; an emissive colour; anisotropic filtering and texture quality.
9. **Post effects.** Bloom with tint, width and falloff; film grain; FXAA; colour grading; lens flares; resolution scale; refraction. The menu's "distortion effects" toggle, which the guide turns off, likely covers the executable's lens distortion, chromatic aberration, radial blur and image trail effects; that mapping is inferred, not checked.

## Tickora today

The phase 6 renderer writes `Rgba8Unorm` with no texture decode, no HDR buffer and no tone mapping, so lighting runs in gamma space. It has point lights only, Lambert diffuse plus a flat ambient, Karis falloff, nearest sampling without mipmaps, no shadows, no specular, no fog and no anti-aliasing.

## Gaps, as candidates for phase 8

Each is a candidate, not an order; Felipe fixes the list and its order.

- Linear space: sRGB decode of colour textures, a 16-bit float light buffer, Hable's filmic curve with a white point, manual exposure, one gamma encode at the end.
- Spot lights, with gobo textures.
- Shadows: single maps for spot lights; HPL3 casts none from point lights (already ROADMAP 8.2, with specular).
- Specular with a specular power per material (ROADMAP 8.2).
- Normal maps, mipmaps and filtered sampling (ROADMAP 8.3), and anisotropic filtering.
- Distance and height fog, with fog volumes.
- SSAO. The temporal pass reads earlier frames, so a screenshot would depend on the frames rendered before it; goldens would need a fixed frame sequence or the spatial pass only.
- Bloom and film grain. Film grain needs a seeded pattern per tick to stay deterministic.
- Sky and ground ambient, then irradiance probes.
- Parallax mapping, decals, colour grading.
- FXAA, which collides with the decision to add anti-aliasing after the coal-mine demo.

## Sources

- Frictional Games, "Tech Feature: Pre-pass lighting", 2010: https://frictionalgames.com/2010-10-tech-feature-pre-pass-lighting/
- Frictional Games, "Tech Feature: HDR Lightning", 2012: https://frictionalgames.blogspot.com/2012/09/tech-feature-hdr-lightning.html
- Frictional Games, "Tech Feature: Linear-space lighting", 2013: https://frictionalgames.blogspot.com/2013/11/tech-feature-linear-space-lighting.html
- Frictional Games, "Tech Feature: SSAO and Temporal Blur", 2014: https://frictionalgames.blogspot.com/2014/01/tech-feature-ssao-and-temporal-blur.html
- GameGPU, Amnesia: The Bunker GPU and CPU test: https://en.gamegpu.com/test-gpu/rpgrolevye/amnesia-the-bunker-test-gpu-cpu
- Amnesia fan wiki, HPL Engine: https://amnesia.fandom.com/wiki/HPL_Engine
- Frictional wiki, HPL3 Light scripting API: https://wiki.frictionalgames.com/page/HPL3/SOMA/Scripting/Scripting_Api/Light
- Gamepretty, recommended video and control settings: https://gamepretty.com/amnesia-the-bunker-recommended-video-and-control-settings/
- GamingBolt on SOMA's HPL3 features: https://gamingbolt.com/?p=244793
