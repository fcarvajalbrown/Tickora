# Chat 9: phase 7 item 1, noise as simulation state

Done:
- The handoff Felipe pasted was stale (written after phase 1): the ECS and the float policy (ADR 0001, fixed-point) were already done. Felipe chose phase 7 item 1 as the next piece.
- Checked Unreal's hearing rule in Epic's docs (`FAINoiseEvent`, `UAISense_Hearing::ReportNoiseEvent`): heard when `Square(DistanceToSound) <= Square(HearingRange * Loudness)`; `MaxRange` is multiplied by loudness, 0 means no noise-specific limit.
- Design `docs/design/phase-7-noise.md`, approved by Felipe (d7e0d94).
- `tickora-core::noise` (e60d033): `Location`, `Noise`, `Hearing`, `HeardNoise`; `register`, `emit`, `clear`, `hear`, `hears`; exact 256-bit distance comparison; loudest heard noise wins, ties to the lower entity index. 12 unit tests and 3 replay tests, replay pinned at `1636647955146064675`. CI green on Windows and Linux, run 40697705009. ROADMAP item 7.1 marked done (4b9dca9).
- The two `cargo.exe` processes at session start are the `tickora` MCP server chain of the running session, not leftovers.

Flags opened: the `MaxRange` case applies both limits, read from Epic's doc wording, not from `AISense_Hearing.cpp`.
Flags closed: phase 7 item 1; the idle cargo.exe question.
