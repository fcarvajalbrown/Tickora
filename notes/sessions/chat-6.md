# Chat 6: first-person 3D target, roadmap, phase 5 asset pipeline

Done:
- Felipe set the genre focus: Frictional-style first-person 3D, never 2D or top-down. Recorded in `CLAUDE.md`, PRD section 5 and ADR 0002. The demo is a Bunker-style survival game in a fictional abandoned coal mine inspired by Chile's southern mines.
- ADR 0003: grey-box 3D first, meshes and textures from generators like Corpofobia's. ADR 0004: assets in plain git without LFS (research in `docs/research/gitattributes-and-lfs.md`), `.gitattributes` with binary, diff and linguist attributes.
- Added `ROADMAP.md`, `docs/ROADMAP-HISTORY.md`, `CHANGELOG.md`; phase order confirmed by Felipe: 5 asset pipeline, 6 grey-box 3D renderer, 7 audio, 8 coal-mine demo, 9 public release. Tags v0.1.0 to v0.5.0.
- `docs/research/demo-toolkit.md`: inventory of every tool in Felipe's projects that can build the demo.
- Phase 5 shipped as 0.5.0: CC0 texture and sound fetchers, meshkit, interior kit, mesh checks, PSX bake, contact sheet, credits, licence records folder; Python deps approved and pinned; 67 tool tests in CI.
- claude-backup: restored a mirror damaged by two concurrent index builds and added a lock to `build_index.py`.

Flags opened: Corpofobia's `build_credits.py` has the title-only record bug fixed in Tickora's copy.
Flags closed: phase 5 decisions; script-index race.
