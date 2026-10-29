# Chat 16: phase 8 item 2, linear space and HDR

Done:
- Design `docs/design/phase-8-linear-hdr.md`, approved by Felipe, with his choices: Hable with HPL3's constants, white point 11.2 as a `Scene3` field, linear exposure per scene, `pow(c, 2.2)` decode in the shader, `pow(c, 1/gamma)` encode with gamma as a display setting (default 2.2), unlit and wireframe without the curve.
- CLAUDE.md: HPL3 first, Unreal as a would-like (ac83298); review images open in paint.net (e319d5e).
- Code: `tone` module (3a5048d); meshkit writes palette factors as (byte/255)^2.2 and the render fixtures are rebuilt (e601adb); `Rgba16Float` light buffer and tone pass (71b16e8); MCP and scenario `gamma`, tone metadata, arena `EXPOSURE` 1.0 and `WHITE_POINT` 11.2 (4538492).
- Felipe compared exposures 1.0, 2.0 and 4.0 and kept 1.0. The four lit goldens were re-recorded from CI run 40890878977 (4bba79d); unlit, wireframe and top goldens stayed byte-identical. CI run 40891832875 green.
- Tolerance table measured again on four adapters: `low` still passes (e076ec7).
- ROADMAP: item 2 Done, its text now says 2.2 power decode; exposure areas added as 8.6 before fog (dependency first), later items renumbered to 8.10.

Flags opened: none.
Flags still open: README status paragraph; Corpofobia hearing reply unread; K-weighting gap against Keel.
Next: the 8.3 design, spot lights with gobo textures, for Felipe's approval before code.
