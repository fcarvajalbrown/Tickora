# Scenario files

A scenario is a JSON file that seeds a simulation, feeds it inputs at fixed ticks, runs it to a final tick and checks assertions along the way. It is the way a game built on Tickora proves its behaviour without an editor.

## Top level

| Key | Required | Meaning |
| --- | --- | --- |
| `name` | no | Free text shown in reports. |
| `seed` | yes | Unsigned 64-bit seed, as an integer or a decimal string for values above 2^53. |
| `inputs` | no | List of `{ "at": tick, "inputs": [...] }`. Each input is the same object `tickora_step` takes. Ticks strictly increasing. |
| `run_to` | yes | Final tick, at most 1,000,000. At least the last input tick + 1 and at least the last assertion tick. |
| `expect` | yes | List of assertions. |

Unknown keys are rejected at every level.

## Ticks

Inputs `at: t` are applied in the step that moves the simulation from tick t to t + 1, as `InputLog` records them. An assertion `at: t` is checked when the tick equals t, so `at: 0` is checked before any step runs.

## Assertions

Each entry has `at` and exactly one check:

| Check | Shape |
| --- | --- |
| State hash | `"state_hash": "<decimal u64>"` |
| Replay hash | `"replay_hash": "<decimal u64>"` |
| Live entities | `"entities": <int comparison>` |
| One component | `"component": "health", "count": <int comparison>` |
| Query | `"query": ["position", "velocity"], "count": <int comparison>` |
| Field | `"entity": { "index": 0, "generation": 0 }, "field": "health.hp", "value": <value comparison>` |
| Screenshot | `"screenshot": "goldens/arena/top-600"`, plus optional camera keys and `"tolerance"` |
| Audio | `"audio": [from, to]` plus one audio check, below |

An int comparison is `{ "eq": n }`, or `{ "min": n }`, `{ "max": n }` or both.

A field value comparison depends on the field type:

- Int: an int comparison.
- Bool and Text: `{ "eq": value }`.
- Fx: `{ "raw": "<decimal i64>" }` for an exact Q32.32 match, `{ "raw_min": ..., "raw_max": ... }` for a raw range, or `{ "approx": 1.2, "tol": 1e-6 }`. The approximate form converts the field with `to_f64` for the comparison only; nothing flows back into the simulation.

An assertion on a dead entity, an unknown component or an unknown field fails with a message.

## Screenshot assertions

A screenshot assertion renders the simulation at its tick and compares the frame with a golden image.

- `screenshot` names the golden folder, relative to the scenario file. For an inline scenario over MCP it is relative to the server's working directory.
- The frame takes the same keys as `tickora_screenshot`: `camera`, or `viewport`, `location`, `rotation`, `fov` and `ortho_width`, plus `width`, `height`, `view_mode` and `show_grid`, with the same defaults and the same per-viewport rules.
- The folder holds one golden per bucket, `<platform>_<backend>.png` (for example `windows_dx12.png` or `linux_vulkan.png`), and alternatives `<bucket>.alt1.png`, `<bucket>.alt2.png` and so on. The frame passes if it matches any golden of its bucket; a failure reports the closest one.
- `tolerance` is a preset name or an object. Presets: `"zero"` (the default) is an exact match; `"low"`, `"medium"` and `"high"` are Unreal's presets, allowing 16, 24 and 32 per channel, a share of 0.10 in any 16 by 16 block and 0.02 over the frame, with the neighbour search on. The object: `red`, `green`, `blue` and `alpha` are the largest allowed difference per channel (0 to 255, default 0); `max_local_error` is the largest share of differing pixels in any 16 by 16 block, and `max_global_error` the largest share over the whole frame (0 to 1, default 0); `ignore_aa` (default true) lets a pixel match a golden colour in any of its 8 neighbours.
- A missing golden fails the assertion and names the file to record. A machine with no graphics adapter cannot evaluate the assertion, so the run is a scenario error.

## Audio assertions

The host renders one block of audio per tick, offline: 800 stereo samples at 48,000 Hz and 60 ticks per second, 24-bit integers. The block for tick t is the sound of the step from t to t + 1, so it exists once the simulation reaches t + 1.

- `audio` is the half-open tick range `[from, to)` the check covers, with `from <= to <= at`.
- Exactly one check follows:

| Check | Shape |
| --- | --- |
| Hash | `"hash": "<decimal u64>"`, FNV-1a over the per-tick block hashes in order |
| Peak | `"channel": "left"` or `"right"`, `"peak_dbfs": { "min": x, "max": y }` (either bound or both) |
| RMS | `"channel": "left"` or `"right"`, `"rms_dbfs": { "min": x, "max": y }` |
| Voices | `"voices": <int comparison>`, the most voices active in any tick of the range |
| Clipped | `"clipped": <int comparison>`, samples the master ceiling clamped |
| Loudness | `"loudness_lufs": { "min": x, "max": y }`, integrated LUFS over the range as listen.py measures it |
| Onsets | `"onsets": <int comparison>`, transients found in the range (a 10 dB rise over the last 0.3 s) |
| Sound | `"sound": "<name>"`, `"starts": <int comparison>`, voice starts of that sound in the range |

- Levels are dBFS against 2^23, computed from exact integer peaks and sums of squares; only the final dB value is a float. A silent range reads `null`: it passes a `max` and fails a `min`.
- Loudness, onsets and sound are heard: the host replays the range once per run and every heard check on that range shares the result. A range gated to nothing (under 0.4 s, or silent) reads `null` loudness, with the same `min` and `max` rule. Onsets need at least 3 ticks; a shorter range fails with the reason. Loudness reads about 0.25 dB below a strict BS.1770 meter, because listen.py's K-weighting is an approximation that the port keeps.
- A golden WAV is never compared. The hash pins the audio exactly; `--wav` writes it out to listen to.

## Running

- CLI: `tickora-mcp scenario <file>...`, with `--json` for the full report. Exit code 0 when every assertion passed, 1 when one failed, 2 when a file is malformed. With no arguments the binary serves MCP on stdio.
- Recording goldens: `--record` writes or replaces each screenshot's `<bucket>.png`, and `--record-alternative` adds the next `<bucket>.altN.png`. A recorded screenshot counts as passed and prints `RECORDED <path>`. The two flags cannot be combined, and the MCP tool never records.
- Listening: `--wav <path>` writes the whole run of one scenario file as a 24-bit stereo WAV and prints `WROTE <path>`. The host replays its input log on a fresh simulation to produce the samples, so a run of any length keeps only per-tick summaries in memory.
- Hearing: `--listen` prints, after the report, the reading of the whole run of one file (listen.py's summary with ticks beside seconds) and the sound list: every voice start with its take, gain, pitch, distance and per-channel level, every end, and every start dropped at the voice cap. `--sheet <path>` writes the spectrogram PNG: log frequency from 20 Hz to 20 kHz, a short-term loudness strip and a centroid strip, cyan transient marks along the top and a dashed line per sound start. With `--json` the sound list, every measure and the reading go into the report as `hearing`. `--wav`, `--listen` and `--sheet` share one replay pass. A run is heard up to 36,000 ticks.
- MCP: `tickora_run_scenario` with exactly one of `path` (relative to the server's working directory) or `scenario` (the object inline). It runs on a fresh simulation and leaves the live one untouched. A failed assertion is a normal result with `passed: false`; a malformed scenario is a tool error.

Every assertion is evaluated; a run does not stop at the first failure.
