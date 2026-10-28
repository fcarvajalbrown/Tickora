# Phase 7 design, item 2: headless audio output a scenario can check

Status: approved by Felipe.

Tickora renders game audio offline, one fixed block of samples per tick, in integer math, so a scenario can pin an exact audio hash and check levels without a sound card. Under the CLAUDE.md rule each part starts from how Unreal does it.

## Decisions (Felipe)

- Render model: offline per-tick render, like Unreal's Movie Render Queue; no audio device.
- Sample math: integer fixed point, byte-identical on Windows and Linux.
- Assertions: an audio hash plus peak and RMS meters; a WAV can be written to listen to; no golden WAVs.
- Keel: offline, outside CI. It levels source sounds in item 3 and can measure LUFS and true peak on an exported WAV by hand.
- Scope: one-shots and looping emitters, procedural sources with seeded pitch and gain variation, Unreal's Linear attenuation, equal-power stereo panning, named submixes into a master with a hard ceiling. Quartz and the other falloff curves wait for phase 9.
- Demo: the arena makes sounds; its state and replay hashes are re-pinned once.
- MCP: a new tool, `tickora_listen`.

## Sources checked

- Movie Render Queue renders audio non-realtime, asked for the samples that match each game-thread tick delta, and writes a `.wav` (Epic forum reply in "Trouble getting wav audio to render through the movie render queue"; Epic's "Cinematic rendering export formats" page for the WAV output). The per-tick wording is a forum answer, not an Epic page.
- Submix envelope follower: smoothed amplitude per channel through `OnSubmixEnvelope`, because "audio signals can be significantly faster than game frames" (Epic, "Overview of Submixes"; `USoundSubmix` API).
- MetaSound `WatchOutput` with analyzer `UE.Audio.EnvelopeFollower` (Epic, "MetaSounds reference guide"; `UMetaSoundOutputSubsystem::WatchOutput`).
- Quartz: sample-accurate scheduling of quantized commands on clock boundaries (Epic, "Overview of Quartz"; `FQuartzClock`).
- Sound attenuation: inner radius is the region where the source does not attenuate; falloff distance is the fade beyond it (Epic, "Sound attenuation in Unreal Engine"; UDK "Distance Model Attenuation").
- Keel's README: BS.1770-4 integrated LUFS, 4x oversampled true peak, deterministic, offline, Python.

Not verified in this session: that Linear is Unreal's default distance algorithm, and that Unreal's default stereo panner is equal power. Both are from my knowledge of Unreal; I found no Epic page stating either, and the local installs ship no engine `.cpp`.

## Crate

`crates/tickora-audio`, a workspace member that depends only on `tickora-core`, with no other dependency. Like `tickora-render`, it reads the world and never writes hashed state.

## Format

- `AudioFormat { sample_rate, tick_rate }`, default 48,000 Hz and 60 ticks per second, so 800 samples per tick. `sample_rate` must be a multiple of `tick_rate`.
- Stereo. Samples are `i32` at 24-bit full scale (`2^23`); voices and submixes sum in `i64`.
- The block for tick `t` covers the step from `t` to `t + 1`. A sound started during that step starts at the block's first sample; sample offsets inside a tick wait for Quartz.

## Components (hashed state)

Units and axes are Unreal's: centimetres, X forward, Y right, Z up; every value is `Fx`, reusing `noise::Location`.

- `Listener { location, yaw }` (`listener`): yaw in degrees. With several listeners, the lowest entity index is used.
- `PlaySound { location, sound, gain, pitch, seed }` (`play_sound`): a one-shot, like `SpawnSoundAtLocation`. One entity per play; `audio::clear(world)` at the start of `step` despawns them, as noise does.
- `SoundEmitter { sound, gain, pitch, seed }` (`sound_emitter`): a looping sound on an entity with a `Location`, like an `AudioComponent`. It follows the entity, starts when the component appears and stops when the component or the entity goes.

`sound` is a `u32` id into the sound bank. `audio::register(world)` registers the three components in this order.

## Sound bank and sources (not hashed, like meshes)

A `SoundBank` maps names to ids and `SoundDef`s, each built at runtime from parameters, in the spirit of a MetaSound Source:

- Source: `Sine { hz }`, `Square { hz }`, `Noise` (white noise from a PCG32 seeded by the play's `seed`), or `Pcm { samples, rate }` (filled by item 3).
- Envelope: attack and release ramps in samples; a duration for one-shot oscillators and noise; PCM plays its length.
- Variation: `gain` and `pitch` ranges. `audio::play` draws the actual gain, pitch and seed into `PlaySound`, so the variation is hashed state and the renderer stays a pure function of the world.
- Attenuation: `None` (2D: no distance gain, no panning), or `Linear { inner_radius, falloff_distance }`, where gain is 1 within the inner radius, falls in a straight line to 0 at `inner_radius + falloff_distance`, and is 0 beyond.
- Submix: the id of the submix the voice feeds.

Variation draws come from their own Rng stream, `Rng::new(seed, AUDIO_STREAM)`, never the gameplay stream, so adding sounds to a game does not move its gameplay randomness.

## Mixing

- Sine and square read a 4,096-entry table with a `u32` phase accumulator; pitch scales the phase increment. PCM steps a Q32.32 position and interpolates linearly in integers.
- The sine table is built from a Taylor series using only f64 add, multiply and divide, which IEEE 754 rounds the same way on every platform, never `f64::sin`. A unit test pins its hash on both platforms.
- Distance uses an exact integer square root of the squared distance in `u128`.
- Panning is equal power from the listener: pan is the source's direction projected onto the listener's right axis, from -1 to 1, and the channel gains are the cosine and sine of `(pan + 1) * pi / 4` from the same table.
- Submixes: a `MixGraph` of named submixes, each with a gain and a parent, ending in `master`. It is defined outside the world, as Unreal submixes are assets. Cycles are rejected.
- Master: master gain, then a hard ceiling that clamps to plus or minus `2^23 - 1` and counts clipped samples. A true-peak limiter is Keel's job, offline.
- Gains multiply as `i64 * Fx raw` in `i128`, then shift right by 32.
- Up to 64 voices. A start past the cap is dropped and counted; Unreal's Sound Concurrency rules wait for phase 9.

## Capture and meters

Keeping every sample of a 1,000,000-tick run would take gigabytes, so the host keeps a summary per tick: the block's FNV-1a hash (over 24-bit little-endian samples, left then right), per-channel peak and sum of squares, clipped samples and active voices.

- Audio hash over ticks `[from, to)`: FNV-1a over the per-tick block hashes in order.
- Peak and RMS in dBFS per channel over the range. The integers are exact; the dB conversion happens last, in f64, for reports and comparisons only. Silence is `-inf` (JSON `null`) and passes any `max`.
- A WAV of any range comes from a fresh host replaying the input log, which determinism allows: 24-bit PCM stereo, Keel's input format.

## Scenario assertions

An assertion with `"audio": [from, to]` (with `to <= at`) and exactly one check:

| Check | Shape |
| --- | --- |
| Hash | `"hash": "<decimal u64>"` |
| Peak | `"channel": "left" or "right", "peak_dbfs": { "min": x, "max": y }` (either bound) |
| RMS | `"channel": "left" or "right", "rms_dbfs": { "min": x, "max": y }` |
| Voices | `"voices": <int comparison>`, the most active voices in any tick of the range |
| Clipping | `"clipped": <int comparison>` |

The CLI gains `--wav <path>`, for one scenario file: it writes the whole run.

## MCP tool 9: `tickora_listen`

Arguments `from` and `to` (default 0 to the current tick) and an optional `path` for a WAV, relative to the server's working directory. It returns the format, the range, the audio hash as a decimal string, per-channel peak and RMS in dBFS, clipped samples, the most active voices in the range and the voices active now.

## Arena

- Registers the audio and noise `Location` components, a listener at the default first-person camera (`EYE_HEIGHT_CM`, its yaw), and a bank with `spawn` (short sine chirp) and `hit` (noise burst with pitch and gain variation), both `Linear` attenuation, into an `sfx` submix.
- Spawn plays `spawn` at the new entity; damage plays `hit` at the target.
- Gameplay randomness is untouched, so positions, hp values and screenshot goldens stay as they are. The listener and the `PlaySound` entities live in the arena's one World, as Unreal puts audio components on actors in the level, so `tickora_query` sees them; that shifts live entity counts and entity indices, so those assertions in `scenarios/arena.json` are re-derived once (Felipe's choice at review). The state and replay pins in `scenarios/arena.json` and the MCP session test change once, with the reason in the commit. `arena.json` gains audio assertions: silence before the first input, a hit audible in its window, no clipping, a pinned hash.

## Out of scope

- Quartz, music and beat-quantized starts; falloff curves other than Linear; reverb, occlusion and filters; Sound Concurrency rules; no-repeat variant choice (needs item 3's sounds); real-time device output.
- Keel in CI.

## Tests

- Unit: sine table hash, integer square root, Linear attenuation at the inner radius, the edge and beyond; panning hard left, centre and hard right; submix cycle rejected; ceiling clamp and clip count; voice cap; PCM interpolation; WAV header bytes.
- Determinism: the same world rendered twice gives the same block hashes; a pinned hash for a fixed test world, checked in CI on Windows and Linux.
- Scenarios: the new arena assertions and a `.fail.json` with wrong audio expectations.
- MCP: `tickora_listen` in the session test.
