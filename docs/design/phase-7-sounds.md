# Phase 7 design, item 3: CC0 and generated sounds through the phase 5 tools

Status: proposed, waiting for Felipe's approval.

The arena's two synthetic sounds become real ones: `hit` becomes a few CC0 impact takes picked without repeats, and `spawn` becomes one sound generated from a spec. Both go through the phase 5 fetchers, are levelled with Keel's meters, baked small into `assets/gen/audio/` and embedded in the binary. Under the CLAUDE.md rule each part starts from how Unreal does it.

## Decisions (Felipe)

- Scope: the arena's two sounds; no coal-mine sounds before the phase 9 demo design.
- Generator: Corpofobia's `tools/sfx_synth/synth.py`, copied and extended; scipy is pinned in `tools/requirements.txt`.
- Levelling: Keel's `short_term_lufs_max` over 0.4 s windows with a 0.1 s hop, to -18 LUFS, with a per-sound trim in the spec. Sounds shorter than the window are padded first. This is Corpofobia's rule (`specs/audio/mix.json` `reference_lufs`, `tools/audio_mix/build_mix.py`).
- Keel access: the tool imports `meters.py` from `KEEL` (default `C:\Projects\Keel`) inside Tickora's `.venv`, as lighthouse's `Tools/audio/survey.py` does; pyloudnorm is pinned. Tests stub the meter, so CI never needs Keel.
- Bake format: 48 kHz, mono, 16-bit PCM WAV.
- Loading: the arena's bank stays in Rust and embeds the WAVs with `include_bytes!`. A bank file format waits for phase 9.
- Variants: several PCM takes with optional weights and a no-repeat count, default 1.
- Credits: next to the bake, `assets/gen/audio/CREDITS.md`, under Corpofobia's credits rule.

## Sources checked

- MetaSound Random Get (Array), UE 5.5 `MetasoundArrayRandomNode.h`: inputs `In Array`, `Weights` ("Will repeat if this array is shorter than the input array"), `Seed` (-1 for a random seed), `No Repeats` ("The number of elements to track to avoid repeating in a row", default 1) and shared state. The `.cpp` with the pick itself is not in the install, so the pick below is Tickora's own and is not checked against Epic's code.
- SoundWave, UE 5.5 `SoundWave.h`: `Volume` ("0 to 1 - Default is 1.0"), `bLooping`, `NumChannels`, `SampleRate`, cue points and loop regions. Tickora keeps the level in the baked file and leaves the bank gain at 1, as a SoundWave left at its default volume does.
- Keel's README and `meters.py`: BS.1770-4 through pyloudnorm, `short_term_lufs_max(audio, rate, window_s, hop_s)`.
- Spatial audio: Tickora's `Source::Pcm` is one channel, panned and attenuated per voice from the listener, so 3D sounds are baked mono.

## Credits rule (Corpofobia's `.claude/rules/assets-and-credits.md`)

- CC0 and public domain from shared hosts (Freesound, archive.org, OpenGameArt, Wikimedia Commons) are never credited.
- CC0 from a site its makers own (`public_domain_own_sites` in the credits spec, such as `bigsoundbank.com`) is credited by role, never per asset: "Sounds by Joseph Sardin", the site name and domain, then a personal thank-you tied to a cited fact about why that person gives the work away. The fact is never invented; when none is found, I ask Felipe.
- Attribution licences are always credited. No licence other than CC0 or public domain is used in this item.
- Order follows IGDA Game Crediting Guidelines 10.1: Tools, then Sound, Voice, Textures, Fonts, then the engine. Within a section, alphabetical by last name.
- `tools/credits/build_credits.py` already applies this (`public_domain_own_sites`, `skip_sites`, `skip_authors`). If the hit comes only from shared hosts, `CREDITS.md` lists no sound entry, which is the rule working.

## Files

| Path | What | Committed |
| --- | --- | --- |
| `work/audio/` | raw CC0 downloads and candidates | no (ADR 0004) |
| `specs/audio/freesound.json`, `bigsoundbank.json`, ... | fetcher manifests: page, URL, licence, SHA-256 | yes |
| `specs/audio/synth.json` | the generated sounds' spec, synth.py's format | yes |
| `specs/audio/bake.json` | per baked sound: source (manifest file or synth id), time range, trim dB | yes |
| `specs/credits.json` | credits spec: fixed entries, own sites, readers | yes |
| `assets/gen/audio/<name>.wav` | baked sounds, 48 kHz mono 16-bit | yes |
| `assets/gen/audio/manifest.json` | per baked file: source, measured LUFS before and after, gain, SHA-256 | yes |
| `assets/gen/audio/CREDITS.md` | built by `build_credits` | yes |

Names follow the arena: `hit_01.wav` to `hit_0N.wav`, `spawn.wav`.

## Tools

- `tools/sfx_synth/synth.py`: copied from Corpofobia, then extended where Tickora needs it: 16-bit mono output at 48 kHz, and source paths resolved against `work/`. Its sample layers check sources by SHA-256, as Corpofobia's copy does.
- `tools/audio_bake/bake.py`: reads `specs/audio/bake.json`, cuts each source's range with short fades, folds to mono, resamples to 48 kHz, levels to -18 LUFS momentary max plus the trim, refuses a result whose sample peak would clip, and writes the WAV and the manifest. It is built from lighthouse's `Tools/audio/prepare.py` (`momentary_max`, padding) and Corpofobia's `build_mix.py` (`short_term_lufs_max` at 0.4 s and 0.1 s). Before writing it I grep the script index for a closer match and extend that if there is one.
- Candidates: `search_sources.py` finds CC0 impacts. `find_blows.py` locates single blows in longer recordings. A listening file built the way Corpofobia's `tools/listen_batch` does it (each candidate preceded by its number) lets Felipe pick the takes by ear. The spawn sound's synth spec is also played to Felipe before it is baked. Felipe picks both; I do not.
- `tools/requirements.txt` gains exact pins for scipy and pyloudnorm.

## Engine

- `tickora_audio::wav::read`: parses RIFF/WAVE with a `fmt ` chunk of PCM, 1 channel, 48,000 Hz, 16 bits, and a `data` chunk. Other chunks are skipped, and any other format is an error naming what was found. Samples become `i32` shifted left by 8, which is exact. No dependency.
- Takes: `Source::Takes { takes: Arc<[Arc<[i32]>]>, rate, weights, no_repeats }`, or an equivalent field on `SoundDef`, decided while writing it, with the shape recorded here before the commit. Weights repeat when shorter than the takes, as in Unreal. `no_repeats` is clamped to `takes - 1`.
- Pick: `audio::play` draws the take from the audio Rng stream, weighted, among the takes not in the last `no_repeats` picks for that sound. A `TakeHistory` per sound holds those picks. The arena hashes it next to its audio Rng, so replays and `first_divergence` stay exact. `PlaySound` gains `take: u32`, hashed and shown by `tickora_query`.
- Arena: `sound_bank()` builds `hit` from the embedded `hit_0N.wav` takes (keeping its pitch and gain variation and Linear attenuation) and `spawn` from `spawn.wav`, both into `sfx`. Gameplay randomness is untouched, so positions, hp values and the 14 screenshots stay as they are. `PlaySound` gains a field, so the state and replay pins change: `scenarios/arena.json` (state, replay, audio hash and any audio level bounds that move) and the MCP session test are re-pinned once, with the reason in the commit.

## Tests

- Python: synth.py renders a seeded spec to the same bytes twice. bake.py's levelling with a stubbed meter applies the right gain and refuses clipping. A test reads every file in `assets/gen/audio/manifest.json` and checks its SHA-256, and that `CREDITS.md` equals what `build_credits` writes from `specs/credits.json`. These run in CI on both platforms.
- Rust: the WAV reader accepts a fixture and rejects stereo, 24-bit, 44.1 kHz and truncated data. The 16-to-24-bit shift is exact. With `no_repeats` 1 the same take never plays twice in a row over 10,000 picks, weights are respected, and a pick is deterministic for a seed. The embedded arena bank loads.
- Determinism: the audio determinism pin is unchanged unless its test world changes. The arena's audio hash is pinned again on both platforms in CI.

## Build order

1. Pin scipy and pyloudnorm; copy and extend synth.py, with tests.
2. bake.py, with tests.
3. Search and fetch hit candidates, write the listening file, Felipe picks the takes; manifests under `specs/audio/`.
4. Write the spawn synth spec, Felipe listens and approves.
5. Bake both, write the manifest; credits spec and `CREDITS.md` under the credits rule.
6. WAV reader in tickora-audio.
7. Takes and the no-repeat pick.
8. Arena bank from the embedded WAVs, the one re-pin, `docs/scenarios.md` if anything there changes.
9. CI green on Windows and Linux, then ROADMAP 7.3 is marked done.

Each step is tested in debug and release (`CARGO_TARGET_DIR=target/verify` while the MCP server runs), committed and pushed before the next.

## Out of scope

- Looping emitters with baked loops, music, Quartz, beat-synced transitions (phase 9).
- A bank file read at runtime (phase 9).
- Stereo PCM and non-spatialised beds.
- Sound Concurrency rules; shared Random Get state across sounds.
- Running Keel or the bake in CI; CI only checks the committed bytes.
