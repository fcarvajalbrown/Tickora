# Tolerance presets: adapter measurement

Phase 6 item 6 adds `low`, `medium` and `high` tolerance presets for the 3D goldens. This page holds the measurement they are derived from (design: `docs/design/phase-6-renderer.md`, "Tolerance presets"). The criterion that turns it into numbers is Felipe's, recorded under "Criterion".

## What was measured

- Shots: the ten perspective shots in `scenarios/pending/arena-3d.json` (default camera, an elevated overview and a close view of entity 0, each in `lit`, `unlit` and `wireframe` at tick 1, plus the default camera in `lit` at tick 600), 256 by 256 pixels. The four top shots of `scenarios/arena.json` were measured too and are byte-identical on all four adapters.
- Adapters:
  - `rtx2050`: NVIDIA GeForce RTX 2050, DX12, Windows 10 build 19045 (local).
  - `warp-local`: WARP, DX12, the same machine.
  - `warp-ci`: WARP, DX12, GitHub `windows-latest`, CI run 40616842650.
  - `lavapipe-ci`: Mesa 26.1.3 lavapipe, Vulkan, GitHub `ubuntu-latest`, CI run 40616842650.
- Metrics, per pair and shot, from `tickora_render::compare`, the code the golden check runs: the largest difference in any channel, the share of pixels that differ, and the share in the worst 16 by 16 block, each with and without the 8-neighbour search. The neighbour search is run in both directions and the worse one is kept, since it looks up the neighbours of the second image only. The main table counts any channel difference; the sweep repeats the shares with a per-channel tolerance of 1, 2, 4, 8 and 16.

## How to repeat it

1. Record every shot on each local adapter into a copy of `scenarios/`, for `TICKORA_ADAPTER` set to `hardware` and then `software`:
   `tickora-mcp scenario --record <copy>/arena.json <copy>/pending/arena-3d.json`
2. Download the CI frames: `gh run download <run id> -D target/measure/ci` (artifacts `scenario-shots-Windows` and `scenario-shots-Linux`).
3. Build the table:
   `cargo run --release -p tolerance-table -- --shots perspective --sweep 1,2,4,8,16 rtx2050=<dir> warp-local=<dir> warp-ci=<dir> lavapipe-ci=<dir>`
   where each `<dir>` is a `goldens/arena` folder holding one primary golden per shot.

## Findings

- Most differing pixels are off by one unit per channel: rounding in the lit shading. At channel tolerance 0 the RTX 2050 differs from the software adapters in up to 4.40% of the pixels and 21.09% of the worst block (`perspective-1-entity0`). At channel tolerance 1 every pair falls to at most 0.02% and 0.78%, except the pairs with lavapipe, where the wireframe lines below remain.
- The large channel differences (19 to 231) come from a handful of edge pixels covered by a different surface or line. Lavapipe draws a few wireframe line pixels elsewhere than DX12 (up to 0.18% of the pixels and 3.91% of the worst block, in the wireframe shots); the neighbour search absorbs all but 0.78% of the worst block.
- Raising the channel tolerance from 1 to 8 changes nothing in these shots. At 16, with the neighbour search, the pairs with `warp-local` drop to zero.
- WARP on this Windows 10 machine and WARP on `windows-latest` are not the same adapter: they differ in up to 0.02% of the pixels, with channel differences up to 194. A golden recorded on one does not match the other exactly.
- `unlit` shots differ in a few pixels (at most 0.02% of the frame and 0.78% of a block, channel difference 80), all absorbed by the neighbour search.

## Criterion

Felipe chose Unreal's numbers, following the project rule to copy Unreal where it fits. Unreal 5.5 sets the presets in `FAutomationScreenshotOptions::SetToleranceAmounts` (`Engine/Source/Developer/FunctionalTesting/Public/AutomationScreenshotOptions.h`): Low 16, Medium 24 and High 32 per channel, with a brightness window (16 to 240, 24 to 220, 64 to 96) Tickora does not have. The block and frame limits are not part of the preset; the options default them to 0.10 and 0.02 with the anti-aliasing search on, and Tickora puts those in every preset.

| Preset | Per channel | Worst block | Frame | Neighbour search |
| --- | --- | --- | --- | --- |
| `low` | 16 | 0.10 | 0.02 | on |
| `medium` | 24 | 0.10 | 0.02 | on |
| `high` | 32 | 0.10 | 0.02 | on |

The table is the check: at channel tolerance 16 with the neighbour search, the worst pair is 0.00% of the frame and 0.78% of a block, so `low` passes every adapter pair measured. The price is that `low` also lets shading drift up to 16 units before a test fails.

## Results

Per shot and pair, any channel difference counted:

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| perspective-1-default | rtx2050 / warp-local | 19 | 0.28% | 10.16% | 0.19% | 8.98% |
| perspective-1-default | rtx2050 / warp-ci | 1 | 0.28% | 10.16% | 0.19% | 8.98% |
| perspective-1-default | rtx2050 / lavapipe-ci | 19 | 0.31% | 11.72% | 0.22% | 10.55% |
| perspective-1-default | warp-local / warp-ci | 19 | 0.00% | 0.39% | 0.00% | 0.39% |
| perspective-1-default | warp-local / lavapipe-ci | 19 | 0.11% | 6.25% | 0.09% | 5.47% |
| perspective-1-default | warp-ci / lavapipe-ci | 19 | 0.11% | 6.25% | 0.09% | 5.86% |
| perspective-1-default-unlit | rtx2050 / warp-local | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-default-unlit | rtx2050 / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-default-unlit | rtx2050 / lavapipe-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-default-unlit | warp-local / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-default-unlit | warp-local / lavapipe-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-default-unlit | warp-ci / lavapipe-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-default-wireframe | rtx2050 / warp-local | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-default-wireframe | rtx2050 / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-default-wireframe | rtx2050 / lavapipe-ci | 231 | 0.03% | 2.73% | 0.00% | 0.39% |
| perspective-1-default-wireframe | warp-local / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-default-wireframe | warp-local / lavapipe-ci | 231 | 0.03% | 2.73% | 0.00% | 0.39% |
| perspective-1-default-wireframe | warp-ci / lavapipe-ci | 231 | 0.03% | 2.73% | 0.00% | 0.39% |
| perspective-1-entity0 | rtx2050 / warp-local | 114 | 4.40% | 21.09% | 1.09% | 9.38% |
| perspective-1-entity0 | rtx2050 / warp-ci | 29 | 4.38% | 21.09% | 1.08% | 9.38% |
| perspective-1-entity0 | rtx2050 / lavapipe-ci | 29 | 4.40% | 21.09% | 1.09% | 9.38% |
| perspective-1-entity0 | warp-local / warp-ci | 114 | 0.02% | 0.78% | 0.01% | 0.39% |
| perspective-1-entity0 | warp-local / lavapipe-ci | 114 | 0.09% | 3.12% | 0.05% | 2.34% |
| perspective-1-entity0 | warp-ci / lavapipe-ci | 1 | 0.07% | 2.73% | 0.04% | 2.34% |
| perspective-1-entity0-unlit | rtx2050 / warp-local | 80 | 0.02% | 0.78% | 0.00% | 0.00% |
| perspective-1-entity0-unlit | rtx2050 / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-entity0-unlit | rtx2050 / lavapipe-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-entity0-unlit | warp-local / warp-ci | 80 | 0.02% | 0.78% | 0.00% | 0.00% |
| perspective-1-entity0-unlit | warp-local / lavapipe-ci | 80 | 0.02% | 0.78% | 0.00% | 0.00% |
| perspective-1-entity0-unlit | warp-ci / lavapipe-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-entity0-wireframe | rtx2050 / warp-local | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-entity0-wireframe | rtx2050 / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-entity0-wireframe | rtx2050 / lavapipe-ci | 231 | 0.07% | 3.91% | 0.00% | 0.78% |
| perspective-1-entity0-wireframe | warp-local / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-entity0-wireframe | warp-local / lavapipe-ci | 231 | 0.07% | 3.91% | 0.00% | 0.78% |
| perspective-1-entity0-wireframe | warp-ci / lavapipe-ci | 231 | 0.07% | 3.91% | 0.00% | 0.78% |
| perspective-1-overview | rtx2050 / warp-local | 194 | 1.23% | 14.45% | 0.63% | 11.33% |
| perspective-1-overview | rtx2050 / warp-ci | 1 | 1.23% | 14.45% | 0.63% | 11.33% |
| perspective-1-overview | rtx2050 / lavapipe-ci | 1 | 1.21% | 14.45% | 0.61% | 11.33% |
| perspective-1-overview | warp-local / warp-ci | 194 | 0.00% | 0.39% | 0.00% | 0.39% |
| perspective-1-overview | warp-local / lavapipe-ci | 194 | 0.02% | 1.95% | 0.02% | 1.95% |
| perspective-1-overview | warp-ci / lavapipe-ci | 1 | 0.02% | 1.95% | 0.02% | 1.95% |
| perspective-1-overview-unlit | rtx2050 / warp-local | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-overview-unlit | rtx2050 / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-overview-unlit | rtx2050 / lavapipe-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-overview-unlit | warp-local / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-overview-unlit | warp-local / lavapipe-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-overview-unlit | warp-ci / lavapipe-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-overview-wireframe | rtx2050 / warp-local | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-overview-wireframe | rtx2050 / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-overview-wireframe | rtx2050 / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.39% |
| perspective-1-overview-wireframe | warp-local / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-overview-wireframe | warp-local / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.39% |
| perspective-1-overview-wireframe | warp-ci / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.39% |
| perspective-600-default | rtx2050 / warp-local | 19 | 0.30% | 10.16% | 0.21% | 8.98% |
| perspective-600-default | rtx2050 / warp-ci | 1 | 0.30% | 10.16% | 0.21% | 8.98% |
| perspective-600-default | rtx2050 / lavapipe-ci | 19 | 0.34% | 11.72% | 0.24% | 10.55% |
| perspective-600-default | warp-local / warp-ci | 19 | 0.00% | 0.39% | 0.00% | 0.39% |
| perspective-600-default | warp-local / lavapipe-ci | 19 | 0.12% | 6.25% | 0.10% | 5.47% |
| perspective-600-default | warp-ci / lavapipe-ci | 19 | 0.13% | 6.25% | 0.10% | 5.86% |

### Worst over all shots, per pair

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 194 | 4.40% | 21.09% | 1.09% | 11.33% |
| all | rtx2050 / warp-ci | 80 | 4.38% | 21.09% | 1.08% | 11.33% |
| all | rtx2050 / lavapipe-ci | 231 | 4.40% | 21.09% | 1.09% | 11.33% |
| all | warp-local / warp-ci | 194 | 0.02% | 0.78% | 0.01% | 0.39% |
| all | warp-local / lavapipe-ci | 231 | 0.18% | 6.25% | 0.10% | 5.47% |
| all | warp-ci / lavapipe-ci | 231 | 0.18% | 6.25% | 0.10% | 5.86% |

### Worst over all shots, per pair, with a channel tolerance of 1

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 194 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | rtx2050 / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| all | rtx2050 / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-local / warp-ci | 194 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | warp-local / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-ci / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |

### Worst over all shots, per pair, with a channel tolerance of 2

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 194 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | rtx2050 / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| all | rtx2050 / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-local / warp-ci | 194 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | warp-local / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-ci / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |

### Worst over all shots, per pair, with a channel tolerance of 4

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 194 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | rtx2050 / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| all | rtx2050 / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-local / warp-ci | 194 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | warp-local / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-ci / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |

### Worst over all shots, per pair, with a channel tolerance of 8

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 194 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | rtx2050 / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| all | rtx2050 / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-local / warp-ci | 194 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | warp-local / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-ci / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |

### Worst over all shots, per pair, with a channel tolerance of 16

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 194 | 0.02% | 0.78% | 0.00% | 0.00% |
| all | rtx2050 / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| all | rtx2050 / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-local / warp-ci | 194 | 0.02% | 0.78% | 0.00% | 0.00% |
| all | warp-local / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-ci / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |


## Linear space and HDR (phase 8 item 2)

Phase 8 item 2 (`docs/design/phase-8-linear-hdr.md`) moved perspective frames to a 16-bit float light buffer, a `pow(c, 2.2)` decode, Hable's curve in `lit` and a `pow(c, 1/2.2)` encode, so the table was measured again with the same command, shots and four adapters as above.

- Frames: `rtx2050` and `warp-local` recorded on this machine at commit 4bba79d; `warp-ci` and `lavapipe-ci` from CI run 40890878977, the run whose artifacts became the goldens. Exposure 1.0, white point 11.2, gamma 2.2.
- The unlit and wireframe frames are byte-identical to the phase 6 ones on every adapter: meshkit's palette now carries the 2.2 decode, which the encode cancels exactly. Only the four lit shots changed.
- At channel tolerance 0 the lit shots differ more than before: `perspective-1-entity0`, RTX 2050 against the software adapters, reaches 4.65% of the pixels and 33.20% of the worst block (phase 6: 4.40% and 21.09%). At channel tolerance 1 every pair shows the same pixel and block shares as in phase 6, so the new differences are one-unit rounding from the curve and the encode.
- The largest channel difference between WARP on this machine and the other adapters is now 82 (phase 6: 194), on a few edge pixels of `perspective-1-overview`.
- Criterion check: at channel tolerance 16 with the neighbour search the worst pair is 0.00% of the frame and 0.78% of a block, the same as in phase 6, so `low` still passes every adapter pair. The presets are unchanged.

### Results (phase 8 item 2)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| perspective-1-default | rtx2050 / warp-local | 27 | 0.25% | 10.16% | 0.14% | 7.42% |
| perspective-1-default | rtx2050 / warp-ci | 1 | 0.24% | 10.16% | 0.14% | 7.42% |
| perspective-1-default | rtx2050 / lavapipe-ci | 27 | 0.24% | 10.16% | 0.14% | 7.81% |
| perspective-1-default | warp-local / warp-ci | 27 | 0.00% | 0.39% | 0.00% | 0.39% |
| perspective-1-default | warp-local / lavapipe-ci | 27 | 0.04% | 2.73% | 0.02% | 1.95% |
| perspective-1-default | warp-ci / lavapipe-ci | 27 | 0.05% | 2.73% | 0.02% | 1.95% |
| perspective-1-default-unlit | rtx2050 / warp-local | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-default-unlit | rtx2050 / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-default-unlit | rtx2050 / lavapipe-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-default-unlit | warp-local / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-default-unlit | warp-local / lavapipe-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-default-unlit | warp-ci / lavapipe-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-default-wireframe | rtx2050 / warp-local | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-default-wireframe | rtx2050 / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-default-wireframe | rtx2050 / lavapipe-ci | 231 | 0.03% | 2.73% | 0.00% | 0.39% |
| perspective-1-default-wireframe | warp-local / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-default-wireframe | warp-local / lavapipe-ci | 231 | 0.03% | 2.73% | 0.00% | 0.39% |
| perspective-1-default-wireframe | warp-ci / lavapipe-ci | 231 | 0.03% | 2.73% | 0.00% | 0.39% |
| perspective-1-entity0 | rtx2050 / warp-local | 60 | 4.65% | 33.20% | 1.42% | 20.31% |
| perspective-1-entity0 | rtx2050 / warp-ci | 33 | 4.63% | 33.20% | 1.41% | 20.31% |
| perspective-1-entity0 | rtx2050 / lavapipe-ci | 33 | 4.64% | 33.20% | 1.42% | 20.31% |
| perspective-1-entity0 | warp-local / warp-ci | 60 | 0.02% | 0.78% | 0.00% | 0.39% |
| perspective-1-entity0 | warp-local / lavapipe-ci | 60 | 0.03% | 0.78% | 0.00% | 0.39% |
| perspective-1-entity0 | warp-ci / lavapipe-ci | 1 | 0.01% | 0.39% | 0.00% | 0.39% |
| perspective-1-entity0-unlit | rtx2050 / warp-local | 80 | 0.02% | 0.78% | 0.00% | 0.00% |
| perspective-1-entity0-unlit | rtx2050 / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-entity0-unlit | rtx2050 / lavapipe-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-entity0-unlit | warp-local / warp-ci | 80 | 0.02% | 0.78% | 0.00% | 0.00% |
| perspective-1-entity0-unlit | warp-local / lavapipe-ci | 80 | 0.02% | 0.78% | 0.00% | 0.00% |
| perspective-1-entity0-unlit | warp-ci / lavapipe-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-entity0-wireframe | rtx2050 / warp-local | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-entity0-wireframe | rtx2050 / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-entity0-wireframe | rtx2050 / lavapipe-ci | 231 | 0.07% | 3.91% | 0.00% | 0.78% |
| perspective-1-entity0-wireframe | warp-local / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-entity0-wireframe | warp-local / lavapipe-ci | 231 | 0.07% | 3.91% | 0.00% | 0.78% |
| perspective-1-entity0-wireframe | warp-ci / lavapipe-ci | 231 | 0.07% | 3.91% | 0.00% | 0.78% |
| perspective-1-overview | rtx2050 / warp-local | 82 | 1.22% | 20.31% | 0.45% | 7.81% |
| perspective-1-overview | rtx2050 / warp-ci | 1 | 1.22% | 20.31% | 0.45% | 7.81% |
| perspective-1-overview | rtx2050 / lavapipe-ci | 1 | 1.22% | 19.92% | 0.45% | 7.81% |
| perspective-1-overview | warp-local / warp-ci | 82 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-overview | warp-local / lavapipe-ci | 82 | 0.01% | 0.78% | 0.00% | 0.39% |
| perspective-1-overview | warp-ci / lavapipe-ci | 1 | 0.01% | 0.78% | 0.00% | 0.39% |
| perspective-1-overview-unlit | rtx2050 / warp-local | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-overview-unlit | rtx2050 / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-overview-unlit | rtx2050 / lavapipe-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-overview-unlit | warp-local / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-overview-unlit | warp-local / lavapipe-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-overview-unlit | warp-ci / lavapipe-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-overview-wireframe | rtx2050 / warp-local | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-overview-wireframe | rtx2050 / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-overview-wireframe | rtx2050 / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.39% |
| perspective-1-overview-wireframe | warp-local / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-overview-wireframe | warp-local / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.39% |
| perspective-1-overview-wireframe | warp-ci / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.39% |
| perspective-600-default | rtx2050 / warp-local | 27 | 0.26% | 10.16% | 0.16% | 7.42% |
| perspective-600-default | rtx2050 / warp-ci | 1 | 0.26% | 10.16% | 0.16% | 7.42% |
| perspective-600-default | rtx2050 / lavapipe-ci | 27 | 0.26% | 10.16% | 0.16% | 7.81% |
| perspective-600-default | warp-local / warp-ci | 27 | 0.00% | 0.39% | 0.00% | 0.39% |
| perspective-600-default | warp-local / lavapipe-ci | 27 | 0.05% | 2.73% | 0.02% | 1.95% |
| perspective-600-default | warp-ci / lavapipe-ci | 27 | 0.05% | 2.73% | 0.02% | 1.95% |

### Worst over all shots, per pair (phase 8 item 2)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 82 | 4.65% | 33.20% | 1.42% | 20.31% |
| all | rtx2050 / warp-ci | 80 | 4.63% | 33.20% | 1.41% | 20.31% |
| all | rtx2050 / lavapipe-ci | 231 | 4.64% | 33.20% | 1.42% | 20.31% |
| all | warp-local / warp-ci | 82 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | warp-local / lavapipe-ci | 231 | 0.18% | 3.91% | 0.02% | 1.95% |
| all | warp-ci / lavapipe-ci | 231 | 0.18% | 3.91% | 0.02% | 1.95% |

### Worst over all shots, per pair, with a channel tolerance of 1 (phase 8 item 2)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 82 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | rtx2050 / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| all | rtx2050 / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-local / warp-ci | 82 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | warp-local / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-ci / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |

### Worst over all shots, per pair, with a channel tolerance of 2 (phase 8 item 2)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 82 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | rtx2050 / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| all | rtx2050 / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-local / warp-ci | 82 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | warp-local / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-ci / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |

### Worst over all shots, per pair, with a channel tolerance of 4 (phase 8 item 2)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 82 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | rtx2050 / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| all | rtx2050 / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-local / warp-ci | 82 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | warp-local / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-ci / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |

### Worst over all shots, per pair, with a channel tolerance of 8 (phase 8 item 2)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 82 | 0.02% | 0.78% | 0.00% | 0.00% |
| all | rtx2050 / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| all | rtx2050 / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-local / warp-ci | 82 | 0.02% | 0.78% | 0.00% | 0.00% |
| all | warp-local / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-ci / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |

### Worst over all shots, per pair, with a channel tolerance of 16 (phase 8 item 2)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 82 | 0.02% | 0.78% | 0.00% | 0.00% |
| all | rtx2050 / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| all | rtx2050 / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-local / warp-ci | 82 | 0.02% | 0.78% | 0.00% | 0.00% |
| all | warp-local / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-ci / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |

## Spot lights and gobos (phase 8 item 3)

Phase 8 item 3 (`docs/design/phase-8-spot-lights.md`) replaced Karis with HPL3's linear distance falloff, set the arena's lights to intensity 1.5, and added spot lights with an f32 cosine fade, a projected gobo read with nearest lookups, and the lighting scene. The table was measured again with the same command and four adapters, once per scenario: the ten arena perspective shots (`--shots perspective`) and all twelve lighting shots (no `--shots`, since every lighting shot is perspective).

- Frames: `rtx2050` and `warp-local` recorded on this machine at commit 1c5444f; `warp-ci` and `lavapipe-ci` from CI run 40917327743, the run whose artifacts became the goldens. Exposure 1.0, white point 11.2, gamma 2.2.
- Arena: the four top shots and the six unlit and wireframe perspective shots in the artifacts match the committed goldens byte for byte on both CI adapters. The lit shots differ between RTX 2050 and the software adapters by one unit over up to 3.98% of the pixels and 24.61% of a block; at channel tolerance 1 every pair is back to the phase 8 item 2 shares.
- Lighting, RTX 2050 against the software adapters: one-unit rounding over the smooth spot and point light gradients, up to 61% of the pixels in `spot-tight`; at channel tolerance 1 it falls to 0.31% of the pixels and 6.25% of a block, and to 0.00% with the neighbour search.
- Lighting, lavapipe against WARP: `spot-round` and `spot-tight`, the two spots without a gobo, differ by up to 8 units over 4.6% to 4.9% of the pixels and 62.50% to 68.75% of a block. This is new: f32 rounding in the cosine fade's `sqrt` and `pow`. It is gone at channel tolerance 8. The gobo shots (`gobo-bars`, `flicker-*`) differ by at most one unit, so nearest lookups land on the same texels on all four adapters.
- The worst pair left at channel tolerance 16 is lavapipe's wireframe lines in `overview-wireframe` (largest difference 132, as for the arena's lines): 0.09% of the frame and 5.47% of a block with the neighbour search.
- Criterion check: at channel tolerance 16 with the neighbour search, every pair in both scenarios is within 0.10 of a block and 0.02 of the frame (worst: 0.09% and 5.47%), so `low` passes every adapter pair. The presets are unchanged.

### Results (phase 8 item 3, arena)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| perspective-1-default | rtx2050 / warp-local | 34 | 0.22% | 7.42% | 0.08% | 4.30% |
| perspective-1-default | rtx2050 / warp-ci | 1 | 0.22% | 7.42% | 0.07% | 4.30% |
| perspective-1-default | rtx2050 / lavapipe-ci | 34 | 0.21% | 7.42% | 0.07% | 4.30% |
| perspective-1-default | warp-local / warp-ci | 34 | 0.00% | 0.39% | 0.00% | 0.39% |
| perspective-1-default | warp-local / lavapipe-ci | 34 | 0.02% | 0.78% | 0.00% | 0.78% |
| perspective-1-default | warp-ci / lavapipe-ci | 34 | 0.02% | 0.78% | 0.01% | 0.78% |
| perspective-1-default-unlit | rtx2050 / warp-local | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-default-unlit | rtx2050 / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-default-unlit | rtx2050 / lavapipe-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-default-unlit | warp-local / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-default-unlit | warp-local / lavapipe-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-default-unlit | warp-ci / lavapipe-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-default-wireframe | rtx2050 / warp-local | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-default-wireframe | rtx2050 / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-default-wireframe | rtx2050 / lavapipe-ci | 231 | 0.03% | 2.73% | 0.00% | 0.39% |
| perspective-1-default-wireframe | warp-local / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-default-wireframe | warp-local / lavapipe-ci | 231 | 0.03% | 2.73% | 0.00% | 0.39% |
| perspective-1-default-wireframe | warp-ci / lavapipe-ci | 231 | 0.03% | 2.73% | 0.00% | 0.39% |
| perspective-1-entity0 | rtx2050 / warp-local | 56 | 3.98% | 24.61% | 1.01% | 13.28% |
| perspective-1-entity0 | rtx2050 / warp-ci | 43 | 3.96% | 24.61% | 1.01% | 13.28% |
| perspective-1-entity0 | rtx2050 / lavapipe-ci | 43 | 3.97% | 24.61% | 1.02% | 13.28% |
| perspective-1-entity0 | warp-local / warp-ci | 56 | 0.02% | 0.78% | 0.00% | 0.39% |
| perspective-1-entity0 | warp-local / lavapipe-ci | 56 | 0.03% | 0.78% | 0.00% | 0.00% |
| perspective-1-entity0 | warp-ci / lavapipe-ci | 1 | 0.01% | 0.78% | 0.00% | 0.39% |
| perspective-1-entity0-unlit | rtx2050 / warp-local | 80 | 0.02% | 0.78% | 0.00% | 0.00% |
| perspective-1-entity0-unlit | rtx2050 / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-entity0-unlit | rtx2050 / lavapipe-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-entity0-unlit | warp-local / warp-ci | 80 | 0.02% | 0.78% | 0.00% | 0.00% |
| perspective-1-entity0-unlit | warp-local / lavapipe-ci | 80 | 0.02% | 0.78% | 0.00% | 0.00% |
| perspective-1-entity0-unlit | warp-ci / lavapipe-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-entity0-wireframe | rtx2050 / warp-local | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-entity0-wireframe | rtx2050 / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-entity0-wireframe | rtx2050 / lavapipe-ci | 231 | 0.07% | 3.91% | 0.00% | 0.78% |
| perspective-1-entity0-wireframe | warp-local / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-entity0-wireframe | warp-local / lavapipe-ci | 231 | 0.07% | 3.91% | 0.00% | 0.78% |
| perspective-1-entity0-wireframe | warp-ci / lavapipe-ci | 231 | 0.07% | 3.91% | 0.00% | 0.78% |
| perspective-1-overview | rtx2050 / warp-local | 62 | 1.12% | 12.50% | 0.39% | 8.59% |
| perspective-1-overview | rtx2050 / warp-ci | 1 | 1.11% | 12.50% | 0.39% | 8.59% |
| perspective-1-overview | rtx2050 / lavapipe-ci | 1 | 1.10% | 12.50% | 0.39% | 8.59% |
| perspective-1-overview | warp-local / warp-ci | 62 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-overview | warp-local / lavapipe-ci | 62 | 0.01% | 0.78% | 0.00% | 0.39% |
| perspective-1-overview | warp-ci / lavapipe-ci | 1 | 0.01% | 0.39% | 0.00% | 0.39% |
| perspective-1-overview-unlit | rtx2050 / warp-local | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-overview-unlit | rtx2050 / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-overview-unlit | rtx2050 / lavapipe-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-overview-unlit | warp-local / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-overview-unlit | warp-local / lavapipe-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| perspective-1-overview-unlit | warp-ci / lavapipe-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-overview-wireframe | rtx2050 / warp-local | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-overview-wireframe | rtx2050 / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-overview-wireframe | rtx2050 / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.39% |
| perspective-1-overview-wireframe | warp-local / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| perspective-1-overview-wireframe | warp-local / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.39% |
| perspective-1-overview-wireframe | warp-ci / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.39% |
| perspective-600-default | rtx2050 / warp-local | 34 | 0.23% | 7.42% | 0.09% | 4.30% |
| perspective-600-default | rtx2050 / warp-ci | 1 | 0.23% | 7.42% | 0.08% | 4.30% |
| perspective-600-default | rtx2050 / lavapipe-ci | 34 | 0.23% | 7.42% | 0.08% | 4.30% |
| perspective-600-default | warp-local / warp-ci | 34 | 0.00% | 0.39% | 0.00% | 0.39% |
| perspective-600-default | warp-local / lavapipe-ci | 34 | 0.02% | 0.78% | 0.00% | 0.39% |
| perspective-600-default | warp-ci / lavapipe-ci | 34 | 0.02% | 0.78% | 0.00% | 0.78% |

### Worst over all shots, per pair (phase 8 item 3, arena)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 80 | 3.98% | 24.61% | 1.01% | 13.28% |
| all | rtx2050 / warp-ci | 80 | 3.96% | 24.61% | 1.01% | 13.28% |
| all | rtx2050 / lavapipe-ci | 231 | 3.97% | 24.61% | 1.02% | 13.28% |
| all | warp-local / warp-ci | 80 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | warp-local / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-ci / lavapipe-ci | 231 | 0.18% | 3.91% | 0.01% | 0.78% |

### Worst over all shots, per pair, with a channel tolerance of 1 (phase 8 item 3, arena)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 80 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | rtx2050 / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| all | rtx2050 / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-local / warp-ci | 80 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | warp-local / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-ci / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |

### Worst over all shots, per pair, with a channel tolerance of 2 (phase 8 item 3, arena)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 80 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | rtx2050 / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| all | rtx2050 / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-local / warp-ci | 80 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | warp-local / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-ci / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |

### Worst over all shots, per pair, with a channel tolerance of 4 (phase 8 item 3, arena)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 80 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | rtx2050 / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| all | rtx2050 / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-local / warp-ci | 80 | 0.02% | 0.78% | 0.00% | 0.39% |
| all | warp-local / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-ci / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |

### Worst over all shots, per pair, with a channel tolerance of 8 (phase 8 item 3, arena)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 80 | 0.02% | 0.78% | 0.00% | 0.00% |
| all | rtx2050 / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| all | rtx2050 / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-local / warp-ci | 80 | 0.02% | 0.78% | 0.00% | 0.00% |
| all | warp-local / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-ci / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |

### Worst over all shots, per pair, with a channel tolerance of 16 (phase 8 item 3, arena)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 80 | 0.02% | 0.78% | 0.00% | 0.00% |
| all | rtx2050 / warp-ci | 80 | 0.00% | 0.39% | 0.00% | 0.00% |
| all | rtx2050 / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-local / warp-ci | 80 | 0.02% | 0.78% | 0.00% | 0.00% |
| all | warp-local / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |
| all | warp-ci / lavapipe-ci | 231 | 0.18% | 3.91% | 0.00% | 0.78% |

### Results (phase 8 item 3, lighting)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| flicker-0 | rtx2050 / warp-local | 1 | 36.65% | 100.00% | 32.29% | 100.00% |
| flicker-0 | rtx2050 / warp-ci | 1 | 36.65% | 100.00% | 32.29% | 100.00% |
| flicker-0 | rtx2050 / lavapipe-ci | 1 | 36.66% | 100.00% | 32.29% | 100.00% |
| flicker-0 | warp-local / warp-ci | 1 | 0.01% | 0.39% | 0.00% | 0.39% |
| flicker-0 | warp-local / lavapipe-ci | 1 | 0.05% | 0.78% | 0.01% | 0.39% |
| flicker-0 | warp-ci / lavapipe-ci | 1 | 0.04% | 0.39% | 0.01% | 0.39% |
| flicker-12 | rtx2050 / warp-local | 1 | 36.61% | 100.00% | 32.39% | 100.00% |
| flicker-12 | rtx2050 / warp-ci | 1 | 36.61% | 100.00% | 32.39% | 100.00% |
| flicker-12 | rtx2050 / lavapipe-ci | 1 | 36.60% | 100.00% | 32.39% | 100.00% |
| flicker-12 | warp-local / warp-ci | 1 | 0.00% | 0.39% | 0.00% | 0.00% |
| flicker-12 | warp-local / lavapipe-ci | 1 | 0.05% | 0.78% | 0.02% | 0.39% |
| flicker-12 | warp-ci / lavapipe-ci | 1 | 0.05% | 0.78% | 0.02% | 0.39% |
| flicker-18 | rtx2050 / warp-local | 1 | 37.63% | 100.00% | 32.39% | 100.00% |
| flicker-18 | rtx2050 / warp-ci | 1 | 37.63% | 100.00% | 32.39% | 100.00% |
| flicker-18 | rtx2050 / lavapipe-ci | 1 | 37.64% | 100.00% | 32.40% | 100.00% |
| flicker-18 | warp-local / warp-ci | 1 | 0.00% | 0.39% | 0.00% | 0.00% |
| flicker-18 | warp-local / lavapipe-ci | 1 | 0.04% | 1.17% | 0.02% | 0.78% |
| flicker-18 | warp-ci / lavapipe-ci | 1 | 0.04% | 1.17% | 0.02% | 0.78% |
| flicker-24 | rtx2050 / warp-local | 1 | 36.61% | 100.00% | 32.35% | 100.00% |
| flicker-24 | rtx2050 / warp-ci | 1 | 36.62% | 100.00% | 32.35% | 100.00% |
| flicker-24 | rtx2050 / lavapipe-ci | 1 | 36.61% | 100.00% | 32.36% | 100.00% |
| flicker-24 | warp-local / warp-ci | 1 | 0.00% | 0.39% | 0.00% | 0.39% |
| flicker-24 | warp-local / lavapipe-ci | 1 | 0.05% | 0.78% | 0.02% | 0.39% |
| flicker-24 | warp-ci / lavapipe-ci | 1 | 0.05% | 0.78% | 0.02% | 0.39% |
| flicker-6 | rtx2050 / warp-local | 1 | 37.06% | 100.00% | 32.43% | 100.00% |
| flicker-6 | rtx2050 / warp-ci | 1 | 37.06% | 100.00% | 32.43% | 100.00% |
| flicker-6 | rtx2050 / lavapipe-ci | 1 | 37.06% | 100.00% | 32.44% | 100.00% |
| flicker-6 | warp-local / warp-ci | 1 | 0.00% | 0.39% | 0.00% | 0.00% |
| flicker-6 | warp-local / lavapipe-ci | 1 | 0.05% | 0.78% | 0.02% | 0.39% |
| flicker-6 | warp-ci / lavapipe-ci | 1 | 0.05% | 0.78% | 0.02% | 0.39% |
| gobo-bars | rtx2050 / warp-local | 1 | 59.25% | 100.00% | 56.58% | 100.00% |
| gobo-bars | rtx2050 / warp-ci | 1 | 59.25% | 100.00% | 56.58% | 100.00% |
| gobo-bars | rtx2050 / lavapipe-ci | 1 | 59.24% | 100.00% | 56.58% | 100.00% |
| gobo-bars | warp-local / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| gobo-bars | warp-local / lavapipe-ci | 1 | 0.04% | 1.56% | 0.01% | 0.78% |
| gobo-bars | warp-ci / lavapipe-ci | 1 | 0.04% | 1.56% | 0.01% | 0.78% |
| overview | rtx2050 / warp-local | 1 | 49.93% | 100.00% | 48.02% | 100.00% |
| overview | rtx2050 / warp-ci | 1 | 49.93% | 100.00% | 48.02% | 100.00% |
| overview | rtx2050 / lavapipe-ci | 1 | 49.96% | 100.00% | 48.04% | 100.00% |
| overview | warp-local / warp-ci | 1 | 0.00% | 0.39% | 0.00% | 0.39% |
| overview | warp-local / lavapipe-ci | 1 | 0.14% | 2.34% | 0.11% | 1.95% |
| overview | warp-ci / lavapipe-ci | 1 | 0.14% | 2.34% | 0.11% | 1.95% |
| overview-unlit | rtx2050 / warp-local | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| overview-unlit | rtx2050 / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| overview-unlit | rtx2050 / lavapipe-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| overview-unlit | warp-local / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| overview-unlit | warp-local / lavapipe-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| overview-unlit | warp-ci / lavapipe-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| overview-wireframe | rtx2050 / warp-local | 96 | 0.01% | 0.78% | 0.00% | 0.00% |
| overview-wireframe | rtx2050 / warp-ci | 96 | 0.01% | 0.78% | 0.00% | 0.00% |
| overview-wireframe | rtx2050 / lavapipe-ci | 132 | 0.42% | 7.03% | 0.09% | 5.47% |
| overview-wireframe | warp-local / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| overview-wireframe | warp-local / lavapipe-ci | 132 | 0.42% | 7.03% | 0.09% | 5.47% |
| overview-wireframe | warp-ci / lavapipe-ci | 132 | 0.42% | 7.03% | 0.09% | 5.47% |
| point-falloff | rtx2050 / warp-local | 1 | 37.20% | 100.00% | 34.27% | 100.00% |
| point-falloff | rtx2050 / warp-ci | 1 | 37.21% | 100.00% | 34.27% | 100.00% |
| point-falloff | rtx2050 / lavapipe-ci | 95 | 37.21% | 100.00% | 34.28% | 100.00% |
| point-falloff | warp-local / warp-ci | 1 | 0.00% | 0.39% | 0.00% | 0.39% |
| point-falloff | warp-local / lavapipe-ci | 95 | 0.16% | 1.95% | 0.09% | 1.17% |
| point-falloff | warp-ci / lavapipe-ci | 95 | 0.16% | 1.95% | 0.09% | 1.17% |
| spot-round | rtx2050 / warp-local | 8 | 59.93% | 100.00% | 58.31% | 100.00% |
| spot-round | rtx2050 / warp-ci | 8 | 59.93% | 100.00% | 58.31% | 100.00% |
| spot-round | rtx2050 / lavapipe-ci | 8 | 59.95% | 100.00% | 58.33% | 100.00% |
| spot-round | warp-local / warp-ci | 0 | 0.00% | 0.00% | 0.00% | 0.00% |
| spot-round | warp-local / lavapipe-ci | 8 | 4.62% | 62.50% | 4.30% | 62.50% |
| spot-round | warp-ci / lavapipe-ci | 8 | 4.62% | 62.50% | 4.30% | 62.50% |
| spot-tight | rtx2050 / warp-local | 1 | 61.07% | 100.00% | 59.56% | 100.00% |
| spot-tight | rtx2050 / warp-ci | 1 | 61.07% | 100.00% | 59.56% | 100.00% |
| spot-tight | rtx2050 / lavapipe-ci | 8 | 61.09% | 100.00% | 59.57% | 100.00% |
| spot-tight | warp-local / warp-ci | 1 | 0.00% | 0.39% | 0.00% | 0.39% |
| spot-tight | warp-local / lavapipe-ci | 8 | 4.87% | 68.75% | 4.56% | 68.75% |
| spot-tight | warp-ci / lavapipe-ci | 8 | 4.87% | 68.75% | 4.56% | 68.75% |

### Worst over all shots, per pair (phase 8 item 3, lighting)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 96 | 61.07% | 100.00% | 59.56% | 100.00% |
| all | rtx2050 / warp-ci | 96 | 61.07% | 100.00% | 59.56% | 100.00% |
| all | rtx2050 / lavapipe-ci | 132 | 61.09% | 100.00% | 59.57% | 100.00% |
| all | warp-local / warp-ci | 1 | 0.01% | 0.39% | 0.00% | 0.39% |
| all | warp-local / lavapipe-ci | 132 | 4.87% | 68.75% | 4.56% | 68.75% |
| all | warp-ci / lavapipe-ci | 132 | 4.87% | 68.75% | 4.56% | 68.75% |

### Worst over all shots, per pair, with a channel tolerance of 1 (phase 8 item 3, lighting)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 96 | 0.31% | 6.25% | 0.00% | 0.00% |
| all | rtx2050 / warp-ci | 96 | 0.31% | 6.25% | 0.00% | 0.00% |
| all | rtx2050 / lavapipe-ci | 132 | 4.76% | 68.75% | 4.49% | 68.75% |
| all | warp-local / warp-ci | 1 | 0.00% | 0.00% | 0.00% | 0.00% |
| all | warp-local / lavapipe-ci | 132 | 4.76% | 68.75% | 4.49% | 68.75% |
| all | warp-ci / lavapipe-ci | 132 | 4.76% | 68.75% | 4.49% | 68.75% |

### Worst over all shots, per pair, with a channel tolerance of 2 (phase 8 item 3, lighting)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 96 | 0.31% | 6.25% | 0.00% | 0.00% |
| all | rtx2050 / warp-ci | 96 | 0.31% | 6.25% | 0.00% | 0.00% |
| all | rtx2050 / lavapipe-ci | 132 | 4.76% | 68.75% | 4.49% | 68.75% |
| all | warp-local / warp-ci | 1 | 0.00% | 0.00% | 0.00% | 0.00% |
| all | warp-local / lavapipe-ci | 132 | 4.76% | 68.75% | 4.49% | 68.75% |
| all | warp-ci / lavapipe-ci | 132 | 4.76% | 68.75% | 4.49% | 68.75% |

### Worst over all shots, per pair, with a channel tolerance of 4 (phase 8 item 3, lighting)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 96 | 0.31% | 6.25% | 0.00% | 0.00% |
| all | rtx2050 / warp-ci | 96 | 0.31% | 6.25% | 0.00% | 0.00% |
| all | rtx2050 / lavapipe-ci | 132 | 4.76% | 68.75% | 4.49% | 68.75% |
| all | warp-local / warp-ci | 1 | 0.00% | 0.00% | 0.00% | 0.00% |
| all | warp-local / lavapipe-ci | 132 | 4.76% | 68.75% | 4.49% | 68.75% |
| all | warp-ci / lavapipe-ci | 132 | 4.76% | 68.75% | 4.49% | 68.75% |

### Worst over all shots, per pair, with a channel tolerance of 8 (phase 8 item 3, lighting)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 96 | 0.01% | 0.78% | 0.00% | 0.00% |
| all | rtx2050 / warp-ci | 96 | 0.01% | 0.78% | 0.00% | 0.00% |
| all | rtx2050 / lavapipe-ci | 132 | 0.42% | 7.03% | 0.09% | 5.47% |
| all | warp-local / warp-ci | 1 | 0.00% | 0.00% | 0.00% | 0.00% |
| all | warp-local / lavapipe-ci | 132 | 0.42% | 7.03% | 0.09% | 5.47% |
| all | warp-ci / lavapipe-ci | 132 | 0.42% | 7.03% | 0.09% | 5.47% |

### Worst over all shots, per pair, with a channel tolerance of 16 (phase 8 item 3, lighting)

| Shot | Pair | Largest channel difference | Differing pixels | Worst 16x16 block | Differing pixels, neighbour search | Worst 16x16 block, neighbour search |
| --- | --- | --- | --- | --- | --- | --- |
| all | rtx2050 / warp-local | 96 | 0.01% | 0.78% | 0.00% | 0.00% |
| all | rtx2050 / warp-ci | 96 | 0.01% | 0.78% | 0.00% | 0.00% |
| all | rtx2050 / lavapipe-ci | 132 | 0.42% | 7.03% | 0.09% | 5.47% |
| all | warp-local / warp-ci | 1 | 0.00% | 0.00% | 0.00% | 0.00% |
| all | warp-local / lavapipe-ci | 132 | 0.42% | 7.03% | 0.09% | 5.47% |
| all | warp-ci / lavapipe-ci | 132 | 0.42% | 7.03% | 0.09% | 5.47% |
