import argparse
import hashlib
import json
import sys
import urllib.request
from pathlib import Path

API = "https://api.polyhaven.com/files/{asset}"
INFO = "https://api.polyhaven.com/info/{asset}"
USER_AGENT = "tickora-cc0-fetch"
MAPS = {"Diffuse": "Color", "AO": "AmbientOcclusion", "Displacement": "Displacement", "Rough": "Roughness"}


def request(url):
    return urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": USER_AGENT}), timeout=120)


def map_urls(files, resolution, kind):
    found = {}
    for source, role in MAPS.items():
        entry = files.get(source, {}).get(resolution, {}).get(kind)
        if entry:
            found[role] = entry["url"]
    if "Color" not in found:
        raise KeyError(f"no {resolution} {kind} diffuse map")
    return found


def file_name(asset, resolution, kind, role):
    return f"{asset}_{resolution}_{role}.{kind}"


def fetch(asset, resolution, kind, dest):
    info = json.load(request(INFO.format(asset=asset)))
    if info.get("type") != 1:
        raise KeyError(f"{asset} is not a Poly Haven texture")
    urls = map_urls(json.load(request(API.format(asset=asset))), resolution, kind)
    folder = dest / asset
    folder.mkdir(parents=True, exist_ok=True)
    digest = hashlib.sha256()
    names = []
    for role in sorted(urls):
        target = folder / file_name(asset, resolution, kind, role)
        if not target.exists():
            target.write_bytes(request(urls[role]).read())
        digest.update(target.read_bytes())
        names.append(target.name)
    return {"asset": asset, "attribute": f"polyhaven-{resolution}-{kind}", "url": urls["Color"], "page": f"https://polyhaven.com/a/{asset}", "licence": "CC0 1.0",
            "author": ", ".join(sorted(info.get("authors", {}))), "sha256": digest.hexdigest(), "files": sorted(names)}


def merge_manifest(manifest, records):
    manifest.parent.mkdir(parents=True, exist_ok=True)
    existing = json.loads(manifest.read_text(encoding="utf-8")) if manifest.exists() else []
    merged = {record["asset"]: record for record in existing}
    merged.update({record["asset"]: record for record in records})
    manifest.write_text(json.dumps(sorted(merged.values(), key=lambda record: record["asset"]), indent=1) + "\n", encoding="utf-8", newline="\n")


def main(argv=None):
    parser = argparse.ArgumentParser(description="Download CC0 textures from Poly Haven (colour, occlusion, height, roughness maps) and record source, licence, author and SHA-256 in the same manifest as fetch_ambientcg.py.")
    parser.add_argument("assets", nargs="+")
    parser.add_argument("--dest", type=Path, default=Path("work/cc0"))
    parser.add_argument("--manifest", type=Path, default=Path("specs/textures/cc0_manifest.json"))
    parser.add_argument("--resolution", default="1k")
    parser.add_argument("--kind", default="jpg")
    args = parser.parse_args(argv)
    records = [fetch(asset, args.resolution, args.kind, args.dest) for asset in args.assets]
    merge_manifest(args.manifest, records)
    for record in records:
        print(f"{record['asset']}: {len(record['files'])} images, sha256 {record['sha256'][:12]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
