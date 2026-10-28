# Chat 12: phase 7 item 3, CC0 and generated sounds

Done:
- Design `docs/design/phase-7-sounds.md`, approved by Felipe after choices made one at a time: arena's two sounds only; Corpofobia's synth.py with scipy pinned; Keel's momentary max to -18 LUFS; Keel reached through a `KEEL` path with pyloudnorm pinned; 48 kHz mono 16-bit bake; WAVs embedded with `include_bytes!`; takes with No Repeats; credits next to the bake under Corpofobia's credits rule.
- Tools: `tools/sfx_synth/synth.py` and `tools/listen_batch/` copied from Corpofobia; `tools/audio_bake/bake.py` new, built from lighthouse's prepare.py and Corpofobia's build_mix.py; `build_credits` writes Markdown for `.md` (586ff64, 57feb80, a82b2ef, 5103497).
- Finding: punches cannot reach -18 LUFS momentary max without clipping (nine of ten candidates top out at -23 to -32 LUFS with the peak at -1 dBFS). Web check backs per-file levelling plus a level in data. Felipe chose a shared trim per transient sound (f431cc5).
- Hit takes: BigSoundBank Punch #5, #6, #8 (Joseph Sardin, CC0), picked by measured criteria because I cannot hear; Felipe judges them in the engine. Spawn: generated brown-noise thump with a 220 Hz rising tone, Felipe's pick of three. Baked 293 kB (5ff2a50).
- Felipe added Unreal's Sound Cue "Randomize Without Replacement" as a second pick mode; Corpofobia's dynamic-sound rule (section 7) is followed; its voice filter not adopted now.
- Credits: `assets/gen/audio/CREDITS.md`, header ends "© Felipe Carvajal Brown. All rights reserved." (Felipe); Joseph Sardin's thank-you reused word for word from Corpofobia (Felipe). Corpofobia's session added the same rights line at Felipe's request (its commit cb9147f0).
- Engine: WAV reader (9e21c08), `Source::Takes` with `Pick::NoRepeats(n)` and `Pick::WithoutReplacement`, hashed `TakeHistory`, `PlaySound.take` (342549b), arena wired and re-pinned once (f83da3d).
- Tests pass in debug and release locally and in CI on Windows and Linux, run 40731326989. ROADMAP 7.3 marked done (3b7d7eb).
- New ROADMAP item 7.4, agent hearing (Felipe: agents cannot hear, the biggest issue).

Flags opened: the spawn sound (-18 LUFS) is about 10 dB above the hits (-28.3 LUFS) by momentary loudness; Felipe judges the balance by ear. Hit pitch range 4/5 to 6/5 and gain range 1/2 to 3/4 kept from the noise hit, also to judge by ear.
Flags closed: item 3 design; how transient one-shots are levelled; old CI failure notifications (runs 11 to 84, all followed by green runs).
