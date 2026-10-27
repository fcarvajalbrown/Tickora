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

An int comparison is `{ "eq": n }`, or `{ "min": n }`, `{ "max": n }` or both.

A field value comparison depends on the field type:

- Int: an int comparison.
- Bool and Text: `{ "eq": value }`.
- Fx: `{ "raw": "<decimal i64>" }` for an exact Q32.32 match, `{ "raw_min": ..., "raw_max": ... }` for a raw range, or `{ "approx": 1.2, "tol": 1e-6 }`. The approximate form converts the field with `to_f64` for the comparison only; nothing flows back into the simulation.

An assertion on a dead entity, an unknown component or an unknown field fails with a message.

## Running

- CLI: `tickora-mcp scenario <file>...`, with `--json` for the full report. Exit code 0 when every assertion passed, 1 when one failed, 2 when a file is malformed. With no arguments the binary serves MCP on stdio.
- MCP: `tickora_run_scenario` with exactly one of `path` (relative to the server's working directory) or `scenario` (the object inline). It runs on a fresh simulation and leaves the live one untouched. A failed assertion is a normal result with `passed: false`; a malformed scenario is a tool error.

Every assertion is evaluated; a run does not stop at the first failure.
