# Phase 7 design, item 4: agent hearing

Status: proposed, waiting for Felipe's approval.

An agent cannot listen to a WAV, so Tickora describes the audio it renders in a form an agent can act on: which sounds played when, what the mix measures, a plain-words reading, and a spectrogram picture. Felipe: "since most MCPs or agents like you have big trouble hearing or can't at all... that is the biggest issue". Under the CLAUDE.md rule each part starts from how Unreal does it. Prior art in `docs/research/agent-hearing.md`.

## Decisions (Felipe)

- Scope: both kinds of hearing. The mix analysis, like Unreal's Audio Synesthesia, and the sound list, like Unreal's Audio Insights Sounds tab and `au.Debug.Sounds`.
- Runtime: Corpofobia's `tools/audio_listen/listen.py` ported to Rust inside `tickora-audio`, with a hand-written FFT and filters and no new dependency.
- Output: structured JSON, a short text reading, and a spectrogram PNG with the sound list marked on it, returned as an image block as `tickora_screenshot` does.
- Measures: listen.py's core (integrated and short-term loudness, true peak, clipping, band shares, centroid and colour, onsets, a timeline) plus every extra: tones, stereo, the whole-range seam and loop-candidate search, voice likeness, DC offset and the sub-80 Hz warning.
- Scenario assertions: a few, on loudness, onset count and named sound starts.

## Sources checked

- Audio Synesthesia (Epic, "Audio Synesthesia in Unreal Engine"): non-real-time analyzers run on a sound offline. LoudnessNRT gives perceptual loudness over time; ConstantQNRT gives band strengths in perceptual bands; OnsetNRT gives onsets with a timestamp and a strength, with settings for granularity and sensitivity.
- Audio Insights (Epic, "Audio Insights in Unreal Engine"): the Sounds tab lists every playing sound with its runtime data. `au.Debug.Sounds` prints sound information on the viewports (Epic, "Audio Console Commands").
- Corpofobia's `listen.py`: BS.1770 K-weighting and gating, 4x oversampled true peak, STFT of 2048 with a hop of 512, five bands (sub 20-80, low 80-300, mid 300-2000, presence 2000-6000, air 6000-20000 Hz), spectral flatness, onsets as a 10 dB rise over a 0.3 s median background in 10 ms frames, tones as peaks 10 dB over a median-filtered spectrum, syllable-rate voice score, stereo correlation and mono-fold loss, seams over 0.2 s third-octave spectra.

## Sound list (Audio Insights)

- The mixer reports, per block, each voice that starts or ends and each start dropped at the 64-voice cap. A start carries the tick, sound name, take, origin (a location or an emitter entity), gain, pitch, distance to the listener in cm, the left and right gains after attenuation and pan in dB, and the submix. An end carries the tick and why: finished, stopped, or still playing at the end of the range.
- The report is a field on `Block`, outside `BlockSummary`, so no audio hash or state hash moves.

## Mix analysis (Synesthesia, measured as listen.py does)

- Lives in a new `tickora_audio::listen` module: radix-2 FFT, Hann window, RBJ biquads, a 4th-order Butterworth band-pass run forward and back, a windowed-sinc 4x upsampler for true peak, median filters and peak finding.
- Input is the rendered 24-bit frames as `f64` over full scale. Every measure is derived output, as `Fx::to_f64` is: nothing is hashed, so the audio and state pins stay where they are.
- Times are given in ticks and in seconds; the timeline uses 12 segments by default, as listen.py does.
- The plain-words reading reuses listen.py's thresholds and wording word for word, so the same sound reads the same in Corpofobia and Tickora.
- `f64::sin` comes from each platform's maths library, so the last digits can differ between Windows and Linux. Assertions use tolerances for that; a reading word that sits exactly on a threshold could differ between platforms.

## Spectrogram

- Log-frequency spectrogram (20 Hz to 20 kHz) over the range, with a short-term loudness strip and a centroid strip below, as listen.py's sheet lays out.
- Each sound start is a vertical line in a colour per sound name; the JSON maps names to colours, since the PNG has no text. Onsets are tick marks along the top.
- Rasterised to RGBA in `tickora-audio` with a fixed colour ramp, encoded with the `png` crate Tickora already uses through `tickora_render::image`. Default 1000 by 500 pixels.

## Samples

- The capture keeps only per-tick summaries. Listening replays the input log on a fresh simulation, as `--wav` does, and feeds the frames and the sound list to the analysis; `write_wav` and listening share one replay pass in the host.
- A range is capped at 36,000 ticks (10 minutes) for analysis, about 230 MB of stereo `f64`; longer ranges are an error naming the cap. Hash and meter checks keep no cap.

## MCP tool 9, `tickora_listen`

- Gains `describe` (default true). With it, the result adds `sounds` (the sound list), `analysis` (every measure), `reading` (the text) and a spectrogram image block. `describe: false` returns what the tool returns today.
- `path` still writes the WAV; `sheet` writes the PNG to a path as well.

## Scenario CLI

- `tickora-mcp scenario --listen <file>` prints the reading and the sound list for the whole run of one file; `--sheet <path>` writes the PNG. With `--json` the full analysis goes into the report.

## Scenario assertions

New checks for an `audio` range, one per assertion, following the existing shape in `docs/scenarios.md`:

| Check | Shape |
| --- | --- |
| Loudness | `"loudness_lufs": { "min": x, "max": y }`, integrated over the range; a silent range reads `null` |
| Onsets | `"onsets": <int comparison>`, onsets found in the range |
| Sound | `"sound": "<name>", "starts": <int comparison>`, voice starts of that sound in the range |

`scenarios/arena.json` gains one of each; `scenarios/arena-wrong-audio.fail.json` gains a wrong one of each.

## Reference copy and cross-check

- `listen.py` is copied into `tools/audio_listen/` as the reference, under the copy-and-extend rule.
- A Python test in `tests/tools/` runs the Rust CLI and `listen.py` on the same arena WAV and compares every number within a stated tolerance, so the port is checked against the original, not only against itself.

## Tests

- Unit tests per measure on synthetic signals: a 1 kHz sine at -20 dBFS reads its known LUFS, peak and tone; silence reads `null`; clicks give the onsets placed; a hard-left voice gives the expected correlation.
- The sound list on the arena: spawn and hit starts at the ticks the input log sets.
- Scenario tests for the three checks, passing and failing.
- Everything in debug and release, locally (`CARGO_TARGET_DIR=target/verify`) and in CI on Windows and Linux.

## Build order

1. Sound list from the mixer, and the shared replay pass in the host.
2. `listen` module: FFT, filters, loudness, true peak, bands, centroid, flatness.
3. Onsets, tones, stereo, voice likeness, seams and loop search, DC offset, sub warning, segments.
4. The reading text.
5. Spectrogram raster and PNG.
6. `tickora_listen` with `describe` and `sheet`; CLI `--listen` and `--sheet`.
7. Scenario assertions, `arena.json` and the fail file.
8. Reference copy and Python cross-check.
9. Docs (`docs/scenarios.md`, README tools list), CI, ROADMAP 7.4.

## Out of scope

- Naming sounds with a trained classifier (AudioSet-style labels); it would need a model runtime and a dependency decision.
- Real-time analysis during play; Tickora renders offline.
- Felipe's by-ear judgement of the arena's spawn and hit balance stays open; hearing gives numbers to discuss it with, not a verdict.
