# Chat 21

CI fix, then phase 8 item 4 steps 5 to 8.

## Done

- CI red for 11 runs because ten second-room shots had no goldens; moved to a pending scenario the repo test skips (b6eec14), CI green again (41156652815).
- The pasted handoff was stale (phase 1 and ECS long done); Felipe chose to continue 8.4.
- Felipe approved porting HPL3's spot shadow filter over the HPL2 jitter disk (9af4c7d); ported in 735413c. Acne gone; 26 old shots byte-identical on the RTX 2050 and WARP.
- Step 5 closed: Felipe kept gloss_dull 0.3/0.3, gloss_sharp 0.6/0.7, intensity 4 (58308f6).
- Step 6: second-room goldens from CI run 41158610447, shots back in lighting.json (2ea2582).
- Step 7: local WARP material-uniform fault found and fixed with a per-batch instance vertex buffer (076d492); tolerance table re-measured, `low` passes every pair (bc706d0).
- Step 8: ROADMAP 8.4 Done.

## Flags opened

- None.

## Flags closed

- Shadow acne (HPL3 filter). Local WARP specular gap (material uniform fault). CI failing on golden-less shots.
