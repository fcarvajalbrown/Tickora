# Session notes

Current state:
- Bunker binary fully decompiled into `research-private/bunker/decompile/code/`, one file per function address. The pyghidra-mcp project has its code and string indexes built, so the server is ready in about a minute.
- Tool measurements and findings: `research-private/bunker/NOTES.md`.
- Bunker main loop and tick answered (NOTES, "Main loop and tick"): fixed-step game logic and physics at a configurable rate (default 60 Hz, constant dt), capped catch-up of 4 steps per frame with backlog drop, variable update once per frame at a whole multiple of the step, render on real frame time. Step count per frame depends on wall time, which is where determinism breaks. Found by grep alone, no server.

Open:
- REA in MCP server mode, untested.
- Tickora phase 1 (PRD section 11) not started.
- Bunker: which game state toggles running variable update inside the fixed loop; and confirming by xref that the scene update is the fixed update handler (inferred so far).
- Repo is public at github.com/fcarvajalbrown/Tickora with README, description and 10 topics (hosting, PRD open question 12, is answered).
- License: Felipe says Tickora is proprietary for now, to discuss later; PRD.md still says MIT OR Apache-2.0 and has not been changed. The README carries no license section.
