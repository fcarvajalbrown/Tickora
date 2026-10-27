# Session notes

Current state:
- Repo is public at github.com/fcarvajalbrown/Tickora with README, description and 10 topics.
- Bunker binary fully decompiled into `research-private/bunker/decompile/code/`, one file per function address; pyghidra-mcp indexes are built, server ready in about a minute.
- Bunker main loop and tick answered in `research-private/bunker/NOTES.md`, "Main loop and tick": constant dt, but steps per frame come from wall time, catch-up cap and backlog drop, which is where determinism breaks.
- Phase 1 (PRD section 11, step 1) built: Cargo workspace, `crates/tickora-core`, zero dependencies. Modules: `rng` (PCG32, matches the reference vector for seed 42 stream 54), `hash` (FNV-1a 64, little-endian writers, matches reference vectors), `input` (tick-indexed `InputLog`, versioned little-endian binary format `TKIL` v1, strict parse errors), `sim` (`Sim` trait, `Runner`, chained `ReplayHash`, `run_with` per-tick callback, `first_divergence`).
- Tests: 16 unit tests and 9 replay tests, including 300 fuzzed input logs and a pinned golden hash (`238730705328002493` for seed 0xC0FFEE, 600 ticks). Passed on Windows local (debug and release) and in CI on windows-latest and ubuntu-latest (debug and release), run 40532173793. The golden value comes from this implementation, so it guards against regressions and platform drift, not against a wrong algorithm.
- The core avoids the three HPL3 problems by construction: no clock anywhere, the tick is an integer counter, the input log is addressed by tick so there is no catch-up cap and no dropped backlog. A wall-clock driver, when one exists, must live outside the core and only decide how many ticks to request.
- Rules the toy sim in `tests/replay.rs` follows and real sims must too: integer or fixed-point state only in hashed data, no HashMap iteration order, no threads inside `step`, all randomness through the `Rng` passed in.

Open:
- License: Felipe says proprietary for now, to discuss later; PRD.md still says MIT OR Apache-2.0, unchanged. README has no license section; no license files added. The workspace sets `publish = false` and no `license` field.
- Floats: the core has no float math. When rendering or physics needs floats, decide between fixed-point and a restricted f32 policy before adding them; cross-platform f32 transcendental results are not guaranteed identical.
- ECS: PRD section 6 names an ECS; phase 1 ships only the tick, RNG, input log and hash. The ECS is not started and needs its own plan.
- FNV-1a is not collision resistant; fine for replay checks, not for any security use.
- Bunker: which game state toggles variable update inside the fixed loop; confirm by xref that the scene update is the fixed update handler.
- REA in MCP server mode, untested.
- Two idle `cargo.exe` processes were still listed at session end; could not inspect their command lines (denied), so unconfirmed whether they are the editor's rust-analyzer or mine.
