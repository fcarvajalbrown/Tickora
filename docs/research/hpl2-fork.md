# Forking HPL2 instead of building Tickora

Felipe asked whether turning one of Frictional's open engines into an agent-driven engine would be easier than Tickora, or too far from the goals. This measures HPL2 against the PRD's four Must features. Read from a shallow clone of `FrictionalGames/AmnesiaTheDarkDescent` (single commit, 12 October 2020); architecture notes only, no code taken.

## What is open

- HPL1 (Penumbra), open since 2010.
- HPL2 (Amnesia: The Dark Descent, A Machine for Pigs), released September 2020 under GPLv3. Code only: the game content stays Frictional's.
- HPL3 (SOMA, Amnesia: The Bunker) is not open. The engine behind the Bunker look is not available.

## Size and build state

- About 114,000 lines of C++ in `HPL2/core`, 734 `.cpp` files across the repo.
- Does not compile as shipped: the README says it needs FBX SDK 2012, which Autodesk no longer offers.
- Visual Studio 2010 project files and CMake for Linux and macOS.
- Prebuilt dependencies in `dependencies.zip`: Newton Dynamics 2.00 as static libraries, Windows only as 32-bit (`lib/win32`), Linux 32 and 64-bit; also DevIL, AngelScript, GLEW, Theora, FBX.
- No test suite in the repo.

## Against the four Must features

| Must | HPL2 today | Work to get there |
|------|------------|-------------------|
| Deterministic core, seeded RNG, replay | Fixed logic step (`cLogicTimer`, updates per second) driven by the wall clock, with a catch-up cap that drops updates when behind. Float math throughout. `cMath` seeds `srand` from `time(NULL)` unless given a seed. Physics is Newton 2.00, a prebuilt float library. | Decouple the loop from the wall clock (small). Seed and route all randomness (small). Bit-identical state across Windows and Linux with float Newton: not reachable without replacing physics and the hashed-state model of ADR 0001, which is the core rewrite Tickora already did. |
| MCP server | None. | Add-on; doable. |
| Scenario files with assertions | None; AngelScript level scripts exist. | Add-on; doable. |
| Headless screenshots | SDL window with OpenGL (`SDL_CreateWindow` with `SDL_WINDOW_OPENGL`), CG and GLSL shaders. | Hidden window or offscreen context on both platforms; moderate, plus GPU variance. |

## Other costs

- License: a fork is GPLv3; the PRD sets MIT OR Apache-2.0.
- Visuals: Dark Descent era (2010) deferred OpenGL renderer; reaching the Bunker look still needs the phase 8 renderer work, in C++.
- Assets: none usable; same as Tickora today.
- Toolchain: 64-bit Windows build needs a Newton rebuild or replacement and the FBX loader removed or replaced.
- Tickora phases 1 to 6 and items 7.1 to 7.3 would be dropped.

## What it would give

- A complete horror game loop that shipped two Amnesia games: physics, lights, sound, scripting, editors.
- A stronger headline for stars: "Amnesia's engine, driven by an agent".

## Finding

HPL2 is easier for making an Amnesia-like game and harder for the agent-native half. The Must that defines Tickora, cross-platform replay hashes, needs physics and state replaced, which is a rewrite of HPL2's core. A separate MCP bridge for HPL2, without determinism, is the part that transfers cheaply.
