# 0002: First-person 3D as the target for every Tickora game

- Status: Accepted
- Date: 2026-10-27
- Deciders: Felipe Carvajal Brown

## Context

Tickora is for games in the style of Frictional Games (Amnesia, SOMA), and the HPL3 engine behind them was studied for that reason. Frictional's games are first-person 3D. The PRD never wrote this down, so phases 2 to 4 built only what a placeholder arena needed: squares with 2D positions, a renderer that draws flat quads, and a single top orthographic viewport.

## Decision

Every Tickora game, demo and renderer decision assumes first-person 3D. 2D and top-down are never offered as a game's view. A top-down view survives only as a debug view.

## Consequences

- The renderer needs a perspective viewport, a depth buffer, meshes, textures and lights; ADR 0003 sets how much of that comes first.
- The simulation core, MCP server, scenario runner, screenshot tool and golden comparer carry over unchanged, since none of them depends on the number of dimensions.
- The arena's 2D components and the quad-and-grid scene format stay as the test fixture they were, not as a model for games.
- Perspective rasterization differs between adapters at edges, so goldens will need the `low`, `medium` and `high` tolerance presets measured against the CI software adapters.

## Alternatives considered

- Top-down 2D demo on the current renderer: rejected by Felipe, since Frictional games are not 2D.
