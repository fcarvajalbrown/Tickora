# Phase 7 design, item 1: noise as simulation state

Status: approved by Felipe.

Noise is hashed simulation state: events with a position and a loudness that listeners such as the phase 9 monster can hear. It lives in `tickora-core`, with no new dependency. Under the CLAUDE.md rule it starts from Unreal's AI hearing sense.

## Sources checked

- `FAINoiseEvent` (Epic Python API docs, `AINoiseEvent`): loudness is the "Loudness modifier of the sound. If MaxRange is non-zero, this modifies the range (by multiplication). If there is no MaxRange, then if Square(DistanceToSound) <= Square(HearingRange * Loudness), the sound is heard, false otherwise." MaxRange is the "Max range at which the sound can be heard. Multiplied by Loudness. A value of 0 indicates that there is no range limit, though listeners are still limited by their own hearing range."
- `UAISense_Hearing::ReportNoiseEvent` (Epic Python API docs): defaults loudness 1.0, max range 0.0; "Values <= 0 mean no limit (still limited by listener's range however)."
- Unreal forum thread "ReportNoiseEvent MaxRange is Multiplying by Loudness Problem": a max range of 1000 at loudness 0.2 gives an effective range of 200.

Not verified: `AISense_Hearing.cpp` itself (the local UE installs ship no engine .cpp). With MaxRange set, Tickora reads "still limited by their own hearing range" as both limits applying: heard only if within `HearingRange * Loudness` and within `MaxRange * Loudness`.

## Rule

A listener with `Hearing { range }` at `Location` hears a `Noise` when all hold:

- `loudness > 0` and `range > 0`; a noise of zero or negative loudness is never heard (Unreal's squared form would let negative loudness be heard).
- `distance <= range * loudness`.
- if `max_range > 0`: `distance <= max_range * loudness`.

The comparison is exact: `distance^2 * 2^64 <= (range_raw * loudness_raw)^2` in 256-bit unsigned integers built from `u128` halves, where the raws are Q32.32. Squaring an `Fx` would saturate past about 463 m (`sqrt(2^31)` cm), so a far noise would read as near; the wide form has no rounding and no saturation over the whole Fx range.

## Components

Units and axes are Unreal's, as in the renderer: centimetres, X forward, Y right, Z up. Every value is `Fx`.

- `Location { x, y, z }` (`location`): a shared 3D location. The arena keeps its own 2D `Position`.
- `Noise { location, loudness, max_range, instigator: Option<Entity> }` (`noise`): one entity per noise event.
- `Hearing { range }` (`hearing`): on every entity that listens.
- `HeardNoise { location, loudness, instigator, tick }` (`heard_noise`): the last noise the listener heard, as in Unreal's last sensed stimulus. It stays until a newer noise replaces it.

Fields seen through MCP: `x`, `y`, `z`, `loudness`, `max_range`, `range`, `tick`, and `instigator_index`, `instigator_generation` only when there is an instigator.

## Functions

- `noise::register(world)` registers the four components in the order above.
- `noise::emit(world, noise) -> Entity` spawns a noise entity.
- `noise::clear(world)` despawns every noise entity. A sim calls it at the start of `step`, so after a step `tickora_query noise` shows exactly that tick's noises.
- `noise::hear(world, tick)` runs at the end of `step`. Each listener keeps the loudest noise it heard this tick; equal loudness goes to the lower entity index. Listeners and noises are walked in store order and the choice is made by explicit comparison, never by HashMap order.
- `noise::hears(listener, hearing, noise) -> bool` is the rule above, public for tests and games.

## Out of scope

- The arena is unchanged, so its pinned hashes and goldens stay valid. Phase 9's miner and monster are the users.
- Unreal's noise `Tag`, line-of-sight hearing, stimulus age and forgetting.
- Occlusion by walls: the phase 9 tile grid decides that later.

## Tests

- Unit: the range edge (exactly at the limit heard, one raw unit past not), `max_range` on and off, loudness and range of zero or less, loudest wins, ties by entity index, far noises that would saturate a squared `Fx`, `clear` despawns only noises.
- Replay: a small sim with a walking emitter and a listener, run twice with the same seed and inputs, pinned replay hash.
- CI on Windows and Linux.
