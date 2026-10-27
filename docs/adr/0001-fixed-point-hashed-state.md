# 0001: Numeric representation of hashed simulation state

- Status: Accepted
- Date: 2026-10-27
- Deciders: Felipe Carvajal Brown

## Context

PRD section 10 requires identical replay hashes on Windows and Linux. Cross-platform f32 results, especially transcendentals, are not guaranteed identical.

## Decision

Hashed state and component data use `Fx`, an i64 with 32 fractional bits, behind a typed wrapper with operator overloads. Multiplication and division use i128 intermediates. Trigonometry comes from tables or integer algorithms in the core. f32 is allowed for rendering and MCP output derived from state, and is never written back into hashed state.

## Consequences

Replays are bit-identical by construction. The costs are building the `Fx` type and trigonometry, 8 bytes per value, and less natural syntax in gameplay code.

## Alternatives considered

- Restricted f32 in sim state (only + - * / sqrt and comparisons, hashed by bit pattern): easier to write, but identical results depend on compiler and target flags staying fixed, and one stray transcendental or a new target breaks replay silently.
- i32 with 16 fractional bits (Q16.16): compact, but range of about +/-32,768 units and coarse precision overflow in large worlds and integrated velocities.
- Generic `Fx<T, FRAC>`: flexible, but more code and more ways to mix formats wrongly.
