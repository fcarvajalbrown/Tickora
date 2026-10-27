import argparse
import hashlib
import json
import sys
import urllib.parse
import urllib.request
from pathlib import Path

SEARCH = "https://texture.ninja/api/textures?search={term}&tag={term}&q={term}"
FILES = "https://texture-ninja.nyc3.cdn.digitaloceanspaces.com/{name}"
USER_AGENT = "tickora-cc0-fetch"


def request(url):
    return urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": USER_AGENT}), timeout=120)


def search(term):
    return json.load(request(SEARCH.format(term=urllib.parse.quote(term))))


def fetch(name, dest):
    folder = dest / "texture_ninja"
    folder.mkdir(parents=True, exist_ok=True)
    target = folder / name
    url = FILES.format(name=urllib.parse.quote(name))
    if not target.exists():
        target.write_bytes(request(url).read())
    return {"asset": f"texture_ninja/{name}", "attribute": "full", "url": url, "page": "https://texture.ninja/", "licence": "CC0 1.0",
            "author": "Joost Vanhoutte", "sha256": hashlib.sha256(target.read_bytes()).hexdigest(), "files": [f"texture_ninja/{name}"]}


def merge_manifest(manifest, records):
    manifest.parent.mkdir(parents=True, exist_ok=True)
    existing = json.loads(manifest.read_text(encoding="utf-8")) if manifest.exists() else []
    merged = {record["asset"]: record for record in existing}
    merged.update({record["asset"]: record for record in records})
    manifest.write_text(json.dumps(sorted(merged.values(), key=lambda record: record["asset"]), indent=1) + "\n", encoding="utf-8", newline="\n")


def main(argv=None):
    parser = argparse.ArgumentParser(description="Search or download CC0 photo textures from Texture.ninja (Joost Vanhoutte) and record source, licence, author and SHA-256 in the same manifest as fetch_ambientcg.py.")
    parser.add_argument("names", nargs="*", help="file names as the search lists them, e.g. BloodSpatter01.png")
    parser.add_argument("--search", help="list the textures matching a term or tag with their tags, and download nothing")
    parser.add_argument("--dest", type=Path, default=Path("work/cc0"))
    parser.add_argument("--manifest", type=Path, default=Path("specs/textures/cc0_manifest.json"))
    args = parser.parse_args(argv)
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    if args.search:
        for texture in search(args.search):
            print(texture["key"], f"{texture['width']}x{texture['height']}", ",".join(tag["tag"] for tag in texture["tags"]))
        return 0
    records = [fetch(name, args.dest) for name in args.names]
    merge_manifest(args.manifest, records)
    for record in records:
        print(f"{record['asset']}: sha256 {record['sha256'][:12]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
