# Chat 3: MCP server (PRD phase 2)

Done:
- Built `crates/tickora-mcp` with `serde` and `serde_json` only: JSON-RPC parsing, a newline-delimited stdio loop generic over reader and writer, both protocol eras (legacy `initialize` and stateless 2026-07-28 with `server/discover`), the `Host` trait, the demo arena and the six tools `tickora_step`, `tickora_state_hash`, `tickora_list_components`, `tickora_query`, `tickora_get_entity`, `tickora_reset`, plus a `tickora-mcp` binary.
- 14 unit and 9 session tests that run the whole loop in-process, including same-seed determinism, era equivalence, a match against a direct `Runner`, and a pinned session hash.
- Checked which era Claude Code speaks: 2.1.311 is dual-era and probes `server/discover` first; confirmed live, a headless run logged `protocolEra: modern`.
- Registered the server with one command into the committed `.mcp.json`.
- Fixed a gap the live run found: `ttlMs` and `cacheScope` are required on `server/discover` and `tools/list` in 2026-07-28.
- CI green on windows-latest and ubuntu-latest, last run 40548279652 at 0d2bd6b.

Flags opened: one simulation per process versus the spec's explicit-identifier rule for cross-request state; no `outputSchema` on tools; no input-log export or replay tool; Felipe must approve the project MCP server once in an interactive session.
Flags closed: MCP server (phase 2); which MCP era Claude Code speaks; one-command registration (PRD section 10).
