# Licence records

CC0 and public-domain assets fetched by the tools in `tools/cc0_textures/`, `tools/cc0_audio/` and `tools/cc0_fetch/` are recorded in their manifests under `specs/` (source page, download URL, licence, SHA-256), and need no file here.

Every asset that is not CC0 or public domain gets one Markdown file here, named after its author and pack, with these lines:

- Used for: where Tickora uses it.
- Taken from: where the file came from, with a commit or version when it came from another project.
- Author: the name to credit, with a link.
- Source: the store or page it was obtained from, or how it was obtained when there is no page.
- Licence: the licence and version as stated by the source, and the attribution wording it asks for.
- Original archive: file name, size in bytes and SHA-256.

`tools/credits/build_credits.py` builds the credits from the manifests and from fixed entries in a credits spec.
