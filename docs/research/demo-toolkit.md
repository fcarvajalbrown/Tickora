# Tools that can help build the coal-mine demo

An inventory of the tools in Felipe's other projects that serve the phase 5 demo (ADR 0002, ADR 0003), taken from the script index and the projects themselves. Python tools are engine-independent and can be copied into Tickora's `tools/` and extended there (ADR 0003). Godot `.gd` tools cannot run against Tickora; they are listed as the model for the Tickora equivalent.

Paths are relative to `C:\Users\Beetlejuice\Desktop\Corpofobia\` unless another project is named.

## CC0 sourcing and licence records

Every fetcher downloads only CC0 or public-domain files and writes source, licence and SHA-256 to a manifest; `specs/textures/cc0_manifest.json` is the format (asset, attribute, url, page, licence, sha256, files).

| Tool | What it does |
|------|--------------|
| `tools/cc0_textures/fetch_ambientcg.py` | CC0 materials from ambientCG, with manifest |
| `tools/cc0_textures/fetch_polyhaven.py` | CC0 textures from Poly Haven (colour, occlusion, height, roughness), same manifest |
| `tools/cc0_textures/fetch_texture_ninja.py` | CC0 photo textures from Texture.ninja, same manifest |
| `tools/cc0_textures/fetch_missing.py` | Fetches every source a bake spec needs and is missing, checks SHA-256 against the manifest |
| `C:\Projects\lighthouse-godot-spike\Tools\textures\search_ambientcg.py` | Searches ambientCG before fetching |
| `tools/cc0_audio/search_sources.py` | Searches every approved CC0 sound source at once: BigSoundBank, Freesound CC0, the Red Library on archive.org, Wikimedia Commons, OpenGameArt CC0 |
| `tools/cc0_audio/fetch_bigsoundbank.py`, `fetch_freesound.py`, `fetch_commons.py`, `fetch_archive_org.py`, `archive_catalog.py` | Per-source CC0 sound download with manifest; Commons and Freesound decode to 48 kHz mono WAV |
| `tools/cc0_fetch/fetch_cc0.py` | Any CC0 archive: download, safe unpack, manifest |
| `tools/radio_songs/song_licences.py` | Licence row per archive.org item |
| `tools/credits/build_credits.py` | Credits file built from the asset manifests |
| `tools/ref_images/fetch_refs.py`, `tools/ref_sheet/ref_sheet.py` | Internal reference images (never shipped) and a contact sheet of them; for the coal-mine look |
| `C:\Projects\ue-remote\moodboard\commons_fetch.py` | Wikimedia Commons moodboard fetch |
| `C:\Projects\Brown-FPS-Creator\.claude\skills\source-asset\SKILL.md` | Search-first sourcing procedure and the vetted CC0 site list (Kenney, Quaternius, Poly Haven, OpenGameArt, ambientCG) |

Corpofobia keeps per-pack licence notes in `docs/licences/`.

## Meshes

| Tool | What it does |
|------|--------------|
| `tools/meshkit/meshkit.py`, `parts.py` | Seeded mesh builder: box, frustum, extrude, tube, palette materials, box UVs, GLB writer; the base for tunnel pieces, props and the lift |
| `tools/interior_kit/build_interior.py` | Builds an interior from meshkit pieces and a placement list: GLBs, `layout.csv` (Unreal convention) and manifest; the model for building the mine from the sim's tile grid |
| `tools/build_greybox.sh`, `tools/build_galpon.sh` | End-to-end grey-box and level builds |
| `tools/layout_build/build_layout.py` | Validates a gameplay layout spec and writes it canonically |
| `tools/layout_to_godot/layout_to_godot.py` | Unreal-convention layout (cm, Z-up) to metres, Y-up; the same conversion Tickora needs |
| `tools/sign_board/build_sign.py`, `fallen_sign.py`, `tools/ground_sheet/build_sheet.py`, `tools/flag_cloth/build_flag.py`, `tools/police_tape/build_tape.py`, `tools/tree_kit/build_trees.py` | Seeded prop generators (signs, tarps, cloth, tape, scatter) |
| `tools/costume_blockout/build_costume.py`, `tools/costume_cleanup/run_cleanup.py`, `tools/costume_rig/rig_costume.py`, `tools/prop_pose/pose_glb.py` | Organic mesh build, cleanup, rig and pose through headless Blender; the route for the monster |
| `tools/viewmodel_pose/solve_poses.py` | First-person arm poses; the miner's hand and lamp |
| `docs/modelling-pipeline.md` | Evaluation of local 3D generators against the RTX 2050's 4 GB |

## Textures and decals

| Tool | What it does |
|------|--------------|
| `tools/cc0_textures/psx_bake.py` | Bakes CC0 maps to PSX albedo: colour times occlusion and height shading, tint, one power-of-two size |
| `tools/cc0_textures/contact_sheet.py` | One labelled sheet of fetched materials |
| `tools/glb_texture_preview/texture_glb.py` | Applies baked textures to meshkit GLBs by material name |
| `tools/decal_atlas/decal_atlas.py`, `tools/decal_scatter/scatter_decals.py` | Decal atlas and seeded wall decals (soot, water stains) |
| `tools/label_texture/label_texture.py` | Small text textures (signs, dials) with UV transforms for meshkit pieces |
| `tools/crack_mask/crack_mask.py`, `tools/blood_hands/build_hands.py` | Masks cut from CC0 photos |
| `tools/texture_stats/texture_stats.py` | Luma, stripe contrast and saturation of textures |

## Mesh and texture checks

| Tool | What it does |
|------|--------------|
| `tools/check_mesh/check_mesh.py` + `tools/mesh_gate/mesh_gate.py` | Topology check and gate: winding, open and non-manifold edges, degenerate triangles, grid snap |
| `tools/uv_check/uv_check.py` | Textured primitives have valid UVs |
| `tools/coplanar_check/coplanar_check.py`, `tools/overlap_check/piece_overlap.py`, `tools/fold_check/fold_check.py` | Z-fighting and overlap checks |
| `tools/costume_cleanup/canonical_glb.py`, `tools/costume_blockout/rebuild_compare.py` | Canonical GLB bytes and byte-for-byte rebuild comparison; fits Tickora's determinism |
| `tools/mesh_budget/mesh_budget.py`, `tools/mesh_density/mesh_density.py` | Triangle budgets by on-screen size |
| `tools/tree_kit/render_glbs.py`, `tools/glb_anim_video/*` | Headless F3D renders of GLBs and animations |

## Light and look

| Tool | What it does |
|------|--------------|
| `tools/brightness_check/brightness_check.py` | Measures a tour of shots against a brightness target (median luma, darkest view, contrast); the gate for a dark mine that stays readable |
| `tools/cover_reference/steam_screenshot_luma.py` | Luma and contrast of Steam screenshots of reference games |
| `tools/shot_compare/shot_compare.py`, `tools/shot_sheet/shot_sheet.py`, `tools/crop_strip/crop_strip.py`, `tools/godot_shots/contact_sheet.py` | Side-by-side and contact sheets of shots with luma and colour stats |
| `tools/light_map/light_map.gd`, `tools/variant_shots/variant_shots.gd`, `tools/godot_shots/shots.gd` | Godot: light and view captures; model for Tickora scenario shots |

## Monster, pacing and play testing

| Tool | What it does |
|------|--------------|
| `tools/night_bot/night_bot.gd`, `tools/night_sim/run_night.gd` | Godot: plays a night headless over many seeds; model for scenario sweeps |
| `tools/hunter_trace/analyze.py` | Reports hunters stuck in blocked cells or idle while hunting, from a trace log |
| `tools/beat_probe/beat_report.py` | Summarises a tension director's beats per night |
| `tools/walker_probe/walker_probe.py`, `tools/wander_probe/wander_probe.py` | Count AI events over seeded headless runs |
| `tools/crash_soak/soak_loop.py`, `tools/native_crash/*` | Seeded soak runs and Windows crash collection |

## Sound

Tickora has no audio yet; noise is half of the Bunker's loop, so it is a gap for the demo. Keel (`C:\Projects\Keel`) is the mixing and mastering tool for any sound work.

| Tool | What it does |
|------|--------------|
| `tools/sfx_synth/synth.py` | Game sounds from a JSON spec: seeded noise, tones, click trains, CC0 sample layers, seamless loops |
| `tools/sfx_synth/*` (layer, sprinkle, loopify, slice_bursts, to_mono, close_up, trim, concat, crest_cap) | Seeded sound assembly and shaping |
| `tools/stinger_variants/build_variants.py` | Seeded stinger variants from a shared core |
| `tools/audio_mix/*`, `tools/keel_gaming/*` | Mix levels, audibility over the bed, must-hear ranking, captures metered with Keel |
| `tools/audio_listen/listen.py`, `tools/onset_check/*`, `tools/scare_level/scare_level.py`, `tools/scream_pick/scream_pick.py` | Describe and measure sounds without listening |

## Release and promotion

| Tool | What it does |
|------|--------------|
| `tools/package_builds/package_builds.py`, `tools/release_check.sh` | Versioned build zips that are never overwritten |
| `tools/credits/build_credits.py` | Credits from manifests |
| `tools/vertical_cut/*`, `tools/game_capture/*`, `tools/devlog_cover/make_cover.py`, `tools/itch_banner` | Promo clips, captures, covers |
| `tools/roadmap_split/*`, `tools/session_guard/*` | ROADMAP live and history split; ROADMAP consistency guard |
