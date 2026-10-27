# .gitattributes and Git LFS for Tickora's assets

Question: how to store CC0 downloads, manifests and baked meshes, textures and sounds (phase 5), and what `.gitattributes` the repo needs, with Git LFS weighed carefully.

## Current state

- `.gitattributes` holds one line, `* text=auto eol=lf`.
- The repo tracks 8 PNG goldens and no other binaries; `git count-objects` reports 429 KiB loose.
- Corpofobia's `.gitattributes`: `* text=auto eol=lf` plus `binary` on png, jpg, ico, glb, wav, ogg, ogv, ttf, mid, whl; no LFS. Raw CC0 downloads live in a git-ignored `work/`, manifests in `specs/`, baked output in `game/assets/gen/`.

## Findings

Git attributes (git-scm.com/docs/gitattributes):
- `binary` is the built-in macro `-diff -merge -text`: no line-ending conversion, no text diff, no text merge.
- When several patterns match, the later line wins per attribute, so `* text=auto eol=lf` goes first and the `binary` lines after it.
- `diff=rust`, `diff=python`, `diff=markdown` select built-in hunk headers, so diffs name the enclosing function or heading.
- New or changed attributes need `git add --renormalize .` to apply to files already in the index.

GitHub Linguist (github-linguist/linguist docs/overrides.md):
- `linguist-generated` removes paths from the language bar and collapses them in diffs; `linguist-vendored` and `linguist-documentation` remove them from the language bar only. Attributes take effect once `.gitattributes` is committed. The language bar matters for discoverability (PRD section 4).

Git LFS on GitHub (docs.github.com, Git LFS billing):
- Free accounts include 10 GiB storage and 10 GiB bandwidth.
- Bandwidth is charged to the repository owner for every LFS download, including GitHub Actions checkouts, source archives that include LFS objects, and forks and pulls of the repository.
- Each push of a changed LFS file adds its full size to storage.
- Over the bandwidth quota with no payment method, LFS is disabled on the account until the next month; over storage, clones get pointer files only and pushes of new files fail.

Moving to LFS later (git-lfs-migrate man page, Iceshrimp's migration write-up, Gregory Szorc "Why You Shouldn't Use Git LFS"):
- `git lfs migrate import` rewrites history and changes every commit hash, so existing clones and forks diverge; `--no-rewrite` avoids that but leaves old blobs in history.
- Adopting LFS commits the project to an LFS server for good, or to a later history rewrite to leave it.
- Game-dev guides agree on one point: decide before the first asset commit, because a binary committed without LFS stays in history.

## Consequences for Tickora

- A public repo whose goal is stars and forks draws clone, fork and CI traffic, all charged to Felipe's 10 GiB LFS bandwidth; going over disables LFS and leaves cloners with pointer files instead of assets.
- The PSX-size baked outputs ADR 0003 calls for are small, and raw CC0 downloads can be re-fetched from manifests with SHA-256 checks, so nothing large has to enter git.

## Sources

- https://git-scm.com/docs/gitattributes
- https://github.com/github-linguist/linguist/blob/main/docs/overrides.md
- https://docs.github.com/en/billing/concepts/product-billing/git-lfs
- https://www.mankier.com/1/git-lfs-migrate
- https://iceshrimp.dev/iceshrimp/iceshrimp/wiki/Git-LFS
- https://gregoryszorc.com/blog/category/git
- https://uhiyama-lab.com/en/notes/ue/git-lfs-project-management/
- https://bugnet.io/blog/how-to-stop-game-assets-from-bloating-your-git-repository
