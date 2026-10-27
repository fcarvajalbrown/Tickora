# 0003: Grey-box 3D first, with meshes and textures from generators

- Status: Accepted
- Date: 2026-10-27
- Deciders: Felipe Carvajal Brown

## Context

ADR 0002 makes first-person 3D the target, and the renderer draws only flat quads today. The coal-mine demo needs enough 3D to be shown and proven, and Felipe's game Corpofobia already has seeded generators for meshes (`tools/meshkit`, GLB output), CC0 textures (`tools/cc0_textures`, fetch with SHA-256 records and PSX bake) and mesh checks (`check_mesh`, `uv_check`, `coplanar_check`).

## Decision

The renderer gets grey-box 3D first, the way Unreal starts a level as a blockout: a perspective viewport at eye height, a depth buffer, meshes, textures and point lights with distance falloff (the miner's lamp, the generator). Meshes and textures come from generators like Corpofobia's, copied into Tickora's `tools/` and extended there, not rewritten. The renderer reads their output.

## Consequences

- The renderer needs a mesh format reader for the generators' GLB output; that is a new dependency for Felipe to approve.
- Shadows, physics and 3D collision come in later phases, not before the demo.
- The simulation stays a deterministic grid: who sees and hears whom is decided in fixed-point state, and the renderer only draws it.

## Alternatives considered

- Full 3D first (mesh loading, dynamic lights with shadows, 3D movement and collision before the demo): rejected; it delays anything that can be shown or proven.
