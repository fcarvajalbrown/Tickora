# Tolerance presets: adapter measurement

Phase 6 item 6 adds `low`, `medium` and `high` tolerance presets for the 3D goldens. This page holds the measurement they are derived from (design: `docs/design/phase-6-renderer.md`, "Tolerance presets"). The criterion that turns it into numbers is Felipe's and is recorded below once picked.

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
