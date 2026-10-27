# Session notes

Current state:
- Repo is public at github.com/fcarvajalbrown/Tickora with README, description and 10 topics.
- Bunker binary fully decompiled into `research-private/bunker/decompile/code/`, one file per function address; pyghidra-mcp indexes are built, server ready in about a minute.
- Bunker main loop and tick answered in `research-private/bunker/NOTES.md`, "Main loop and tick": constant dt, but steps per frame come from wall time, catch-up cap and backlog drop, which is where determinism breaks.

Open:
- Tickora phase 1 (PRD section 11) not started.
- License: Felipe says proprietary for now, to discuss later; PRD.md still says MIT OR Apache-2.0, unchanged. README has no license section.
- Bunker: which game state toggles variable update inside the fixed loop; confirm by xref that the scene update is the fixed update handler.
- REA in MCP server mode, untested.
