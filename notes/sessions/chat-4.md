# Chat 4: scenario files with assertions (PRD phase 3)

Done:
- Felipe's decisions through the UI: JSON format, all four assertion kinds (hashes, entity and component counts, query match counts, field values; Fx as raw or approx with tolerance), runner as both a CLI and an MCP tool. He approved the design, recorded in `docs/scenarios.md`.
- `crates/tickora-mcp/src/scenario.rs`: strict parser and validation, runner generic over `Host` that evaluates every assertion and reports in file order.
- CLI `tickora-mcp scenario [--json] <file>...` with exit codes 0, 1, 2; no arguments still serves MCP.
- `Host::fresh` and the seventh tool `tickora_run_scenario` (path or inline, fresh sim, live sim untouched).
- `scenarios/arena.json` (16 assertions, 600 ticks) and `scenarios/arena-wrong-expectations.fail.json`; integration tests for repo scenarios, a match against a direct `Runner` fed the same `InputLog`, CLI exit codes and the tool by path.
- Tests: MCP 26 unit, 4 scenario, 10 session, all green in debug and release. CI green on windows-latest and ubuntu-latest, runs 40551169613 at ae5f90a and 40551452393 at f72c5e5.

Flags opened: release binary locked while a Claude Code session runs the `tickora` server (use `CARGO_TARGET_DIR=target/verify`); no record mode; scenarios only for the arena; no `outputSchema` on `tickora_run_scenario`.
Flags closed: PRD phase 3; the three phase 3 design decisions.
