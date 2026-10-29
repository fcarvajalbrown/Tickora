# Chat 15: release 0.7.0 and fix the phase 8 list

Done:
- 0.7.0 released: CHANGELOG [0.7.0] entry, version bump, phase 7 moved to `docs/ROADMAP-HISTORY.md`, release commit 2cc98ab, CI run 40876435012 green on Windows and Linux, tag `v0.7.0` pushed.
- Phase 8 item 1: `docs/research/bunker-look.md` from Frictional's HPL3 tech posts and a feature-name search of the Bunker exe (raw names in `research-private/bunker/NOTES.md`). Main gap: Tickora lights in gamma space with 8-bit output and no tone mapping.
- Phase 8 list approved by Felipe, dependency first: 8.2 linear space and HDR, 8.3 spot lights with gobos, 8.4 shadows and specular, 8.5 normal maps, mipmaps and filtering, 8.6 fog, 8.7 SSAO, 8.8 bloom and film grain, 8.9 ambient and irradiance probes; parallax, decals and colour grading unscheduled.

Flags opened: README status paragraph still says phases 1 to 4 are done.
Flags still open: Felipe judges the arena's spawn and hit balance by ear; Corpofobia's reply to the hearing proposal unread; K-weighting gap against Keel.
Next: the 8.2 design in `docs/design/phase-8-linear-hdr.md`, for Felipe's approval before code.
