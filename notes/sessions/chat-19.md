# Chat 19: phase 8 item 4 design

Done:
- Design `docs/design/phase-8-shadows-specular.md` drafted with Felipe's choices and pushed as a draft (c990575); waiting for his approval before any code.
- Research: HPL3 casts shadows from spot lights only (Bunker shadow gate, HPL3 scripting API and editor wiki); `docs/research/bunker-look.md` corrected (0077873) and extended with how HPL3 levels light rooms without point shadows (236bc8b). Private notes in `research-private/bunker/NOTES.md`, "Shadows and specular".
- HPL2's light shader (third-party copy of the shipped file) gave the specular formula, power mapping, normal offset and jitter filter; HPL2's `TextureCreator.cpp` gave the jitter disk.
- CLAUDE.md rule: search four times as much before calling an HPL3 fact unknown (f1b7906).

Findings:
- `samplerCubeShadow` in the Bunker exe is the HPSL-to-GLSL type table, not point-light shadows.
- Bunker light defaults: CastShadows off, resolution High, both bias multipliers 1.0, ShadowDistance -1.

Flags opened: ROADMAP 8.4 wording changes on approval; light mask boxes left out of scope (a separate item only if Felipe wants it).
Flags still open: README status paragraph; Corpofobia hearing reply unread; K-weighting gap against Keel.
Next: Felipe approves or amends the 8.4 design; then mark it approved, reword ROADMAP 8.4, start step 1.
