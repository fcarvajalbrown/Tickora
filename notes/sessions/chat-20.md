# Chat 20

Phase 8 item 4, shadows from spot lights and specular highlights.

## Done

- Felipe approved `docs/design/phase-8-shadows-specular.md` as written; ROADMAP 8.4 reworded (a40dd72).
- Step 1, f64 reference in `light.rs` (135101f).
- Step 2, renderer: shadow array, jitter maps, specular, materials (2a1bce0).
- Step 3, second lighting room at -20 m with `crate_gloss_dull` and `crate_gloss_sharp` (fc0fc96).
- Step 4, second-room lights, `shadow_quality` in shots and scenarios, 10 new shots (69a694e).
- WARP fix: shadow pass takes CPU clip-space vertices, binds nothing (0770ccc, notes ed8b21a).
- Step 5 re-aim of the second room's lights and cameras (fbe5026).
- `tools/shot_baseline/` records and diffs scenario shots per adapter.
- All 26 existing shots byte-identical on the RTX 2050 and WARP after every change.

## Flags opened

- Step 5: Felipe chose HPL3's raster bias over the centimetre bias. Not implemented yet.
- wgpu `DepthBiasState.constant` is an i32, so HPL3's constant 0.5 cannot be set as is; needs a decision.
- Gloss material values have no HPL3 default; Felipe still picks them, plus the second room's intensities.

## Flags closed

- Penumbra test bar: nudge rule. Second room position: -20 m. Gloss pieces: two meshkit pieces.
