# Chat 22

Phase 8 item 4 step 5, the shadow bias, run alongside chat 21.

## Done

- HPL3's rasterizer bias on the shadow pass: constant -round(0.5 * mul), slope -1.0 * mul, one pipeline cached per bias, cm bias dropped, f64 reference applies D3D12's float-depth rule (e8b15b1).
- `shot_baseline.py compare --stats --diff-dir` (07ec139).
- Bunker findings: shadow maps are `GL_DEPTH_COMPONENT32`; `glPolygonOffset(1.0 * slope mul, 0.5 * bias mul)` by disassembly; per-spot constants near, radius, size and k; the shipped `deferred_light_frag.hpsl` and `helper_shadow_filter.hpsl` read from Felipe's install (0b8a31f, 16cd04a, 0e91504, 10be203).

## Flags opened

- None. The acne this bias left was fixed in chat 21 by the HPL3 filter port.

## Flags closed

- wgpu integer depth-bias constant (rounded to 1). Where HPL3's shadow lookup lives.
