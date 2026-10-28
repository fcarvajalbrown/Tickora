# Chat 8: phase 6 tolerance presets, 3D goldens, release 0.6.0

Done:
- Defined ten arena 3D shots: default camera, an elevated overview (`perspective -2500 0 1200 -25 0 0 90`) and a close view of entity 0 (`perspective -1250 300 170 -15 26 0 90`), each lit, unlit and wireframe at tick 1, plus the default camera at tick 600. Tick 1 because by tick 600 the cubes have wandered 500 to 700 m off the 25 m floor.
- `tools/tolerance_table` (Rust workspace member, reuses `tickora_render::compare`): pairwise table per shot with and without the neighbour search, plus `--sweep` over per-channel tolerances. Measurement in `docs/research/tolerance-presets.md`: lit shading differs by one unit per channel between adapters (up to 4.40% of pixels and 21.09% of a block at channel 0, gone at channel 1); lavapipe wireframe lines and edge pixels make the rest; local WARP and CI WARP differ.
- Felipe chose Unreal's preset numbers: `low`, `medium`, `high` = 16, 24, 32 per channel, block 0.10, frame 0.02, neighbour search on (UE 5.5 `AutomationScreenshotOptions.h`), without Unreal's brightness window.
- 3D goldens recorded by CI (run 40618435179, WARP and lavapipe) and checked in `scenarios/arena.json` with `low`; arena now 30 assertions; passes locally on the RTX 2050 and WARP. CI green on run 40619161517.
- Released 0.6.0: CHANGELOG, phase 6 moved to `docs/ROADMAP-HISTORY.md`, tag `v0.6.0`.
- Results page of the ten 3D shots on four adapters with difference maps, published as a private artifact for Felipe.
- Felipe set two goals of equal weight, now in `CLAUDE.md` and PRD sections 1 and 4: horror games made fast like Corpofobia with visuals like Amnesia: The Bunker, built with an agent; and GitHub stars and discoverability.
- Felipe added phase 8, Bunker-level visuals (study the Bunker's look first, then shadows and specular, then normal maps, mipmaps and filtered sampling), ordered by dependency: 7 audio, 8 visuals, 9 coal-mine demo, 10 public release. Anti-aliasing waits until after the demo.

Flags opened: the script index does not list Rust tools such as `tools/tolerance_table`.
Flags closed: tolerance presets; 3D goldens; phase 6.
