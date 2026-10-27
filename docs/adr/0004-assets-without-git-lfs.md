# 0004: Assets in plain git without LFS, raw CC0 downloads re-fetched from manifests

- Status: Accepted
- Date: 2026-10-27
- Deciders: Felipe Carvajal Brown

## Context

Phase 5 brings CC0 textures and sounds and generated meshes into the repo. The repo is public and its goal is stars and forks. On GitHub's free plan Git LFS has 10 GiB of storage and 10 GiB of bandwidth, and bandwidth is charged to the owner for every LFS download, including clones, forks and Actions checkouts; over the quota without a payment method LFS is disabled until the next month. Moving to LFS later rewrites history and breaks forks. Research: `docs/research/gitattributes-and-lfs.md`.

## Decision

No Git LFS. Raw CC0 downloads live in the git-ignored `work/` and are re-fetched from committed manifests with SHA-256 checks. Manifests and generator specs are committed under `specs/`, and only the small baked output under `assets/gen/` is committed. `.gitattributes` keeps `* text=auto eol=lf`, marks images, meshes, sounds, fonts and archives `binary`, sets Rust, Python and Markdown diff drivers, and marks `assets/gen/` and `scenarios/goldens/` `linguist-generated`.

## Consequences

- No quota can turn a clone into pointer files.
- A fresh clone rebuilds every asset from the manifests, as long as the CC0 sources stay online; a changed or missing source fails its SHA-256 check.
- Baked output must stay small; a large binary committed to plain git stays in history for good.

## Alternatives considered

- LFS for the baked output only: spends the owner's bandwidth on every clone, fork and CI run.
- LFS for raw downloads and baked output: heaviest on both quotas.
