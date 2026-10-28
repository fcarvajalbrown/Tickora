# Chat 10: phase 7 item 2 design, demo design item, scripting research

Done:
- Read Keel's README: offline Python, BS.1770-4 LUFS and 4x true peak, deterministic; no runtime mixer, so it stays offline for item 3 levelling and hand checks on exported WAVs.
- Checked Unreal: Movie Render Queue's non-realtime audio render per tick (forum source), Submix envelope follower, MetaSound WatchOutput, Quartz, sound attenuation.
- Felipe chose the item 2 design: offline per-tick render in a new zero-dependency `tickora-audio`, integer math, audio hash plus peak and RMS assertions, Keel offline outside CI, core Unreal set (one-shots, looping emitters, procedural sources with seeded variation, Linear attenuation, equal-power panning, submixes, hard ceiling), arena makes sounds, MCP tool `tickora_listen`.
- Design written: `docs/design/phase-7-audio-output.md` (cb7588b). Felipe asked to review it before any code.
- ROADMAP phase 9 item 2: demo game design is Felipe's, laid out from real mine plans with branching paths (d38fe05).
- Scripting research `docs/research/scripting-language.md` (d5914b2); post-v1 scripting layer and node editor in ROADMAP Unscheduled (92815f4); teammates who don't use AI named as a post-v1 user in PRD section 3 (59cc5ab).

Flags opened: item 2 design awaits Felipe's review; Linear as Unreal's default distance algorithm and equal power as its default panner are unverified.
Flags closed: none.
