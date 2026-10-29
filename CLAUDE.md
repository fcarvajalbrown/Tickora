# Tickora

Read `PRD.md` first.

## Goal

- Two goals, equally important: make horror games fast, the way Corpofobia was made, with visuals like Amnesia: The Bunker, built with an agent; and GitHub stars and discoverability. Weigh every feature and decision against both.

## HPL3 first, Unreal as a would-like

- Hard rule: HPL3, the engine behind Amnesia: The Bunker that Tickora reverse-engineers, comes first. Wherever its choices are known (Frictional's tech posts, `docs/research/bunker-look.md`, `research-private/bunker/NOTES.md`), follow HPL3, because that keeps the engine simpler to code.
- Unreal is a would-like, mainly for Blueprints-style scripting and mesh modification, and the reference where HPL3 is silent. Features already built from Unreal (camera, viewports, screenshots, automation tests, naming) stay as they are.

## Review images in paint.net

- When Felipe is asked to review an image, render or contact sheet, open it in paint.net (`C:\Program Files\paint.net\paintdotnet.exe <file>`) in the same turn; nothing else works on this machine. A path alone is not enough.

## Genre: Frictional-style, first-person 3D

- Hard rule: Tickora is for games in the style of Frictional Games (Amnesia, SOMA), which are first-person 3D. Every game, demo and renderer decision assumes first-person 3D; never propose 2D or top-down as the game's view. A top-down view is only a debug view.

## Binary research

- `research-private/` is git-ignored and never committed, pushed or published. It holds engine executables and DLLs copied from games Felipe owns, never game assets, plus notes and analysis output.
- Nothing from closed binaries is copied into Tickora as-is: no assets, no identifiers or strings beyond what names a finding. A function that serves Tickora may be ported to Rust from its decompiled logic.
- Stop and tell Felipe on copy protection or anti-tamper; never bypass it.
- Analyze only the binaries Felipe names.
- Binary analysis uses pyghidra-mcp: start the server in the background with `GHIDRA_INSTALL_DIR` and `JAVA_HOME` from `.claude/settings.local.json` and `JAVA_TOOL_OPTIONS=-Xmx6g`: `uvx pyghidra-mcp --transport streamable-http --project-path research-private/<game>/pyghidra --project-name <game> --wait-for-analysis <binary>`. Query it with `uvx --with mcp==1.30.0 pyghidra-mcp-cli`; the client fails to connect with mcp 2.3.0. Stop the server and check `tasklist` for its `java.exe` when done.
- Findings go to `research-private/<game>/NOTES.md`.
