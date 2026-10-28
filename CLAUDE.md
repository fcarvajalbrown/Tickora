# Tickora

Read `PRD.md` first.

## Goal

- Tickora exists to make horror games fast, the way Corpofobia was made, with visuals like Amnesia: The Bunker, built with an agent. Weigh every feature and decision against that goal.

## Unreal Engine as the model

- Hard rule: copy from Unreal Engine anything that can be transported and does not collide with the PRD's original mission. When a feature has an Unreal counterpart (camera, viewports, screenshots, automation tests, naming), start from how Unreal does it.

## Genre: Frictional-style, first-person 3D

- Hard rule: Tickora is for games in the style of Frictional Games (Amnesia, SOMA), which are first-person 3D. Every game, demo and renderer decision assumes first-person 3D; never propose 2D or top-down as the game's view. A top-down view is only a debug view.

## Binary research

- `research-private/` is git-ignored and never committed, pushed or published. It holds engine executables and DLLs copied from games Felipe owns, never game assets, plus notes and analysis output.
- Nothing from closed binaries enters Tickora source: no decompiled code, no copied identifiers or strings beyond what names a finding, no assets.
- Stop and tell Felipe on copy protection or anti-tamper; never bypass it.
- Analyze only the binaries Felipe names.
- Binary analysis uses pyghidra-mcp: start the server in the background with `GHIDRA_INSTALL_DIR` and `JAVA_HOME` from `.claude/settings.local.json` and `JAVA_TOOL_OPTIONS=-Xmx6g`: `uvx pyghidra-mcp --transport streamable-http --project-path research-private/<game>/pyghidra --project-name <game> --wait-for-analysis <binary>`. Query it with `uvx --with mcp==1.30.0 pyghidra-mcp-cli`; the client fails to connect with mcp 2.3.0. Stop the server and check `tasklist` for its `java.exe` when done.
- Findings go to `research-private/<game>/NOTES.md`.
