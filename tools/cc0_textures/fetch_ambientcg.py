import argparse
import hashlib
import json
import sys
import urllib.request
import zipfile
from pathlib import Path

API = "https://ambientcg.com/api/v2/full_json?id={asset}&include=downloadData"
USER_AGENT = "tickora-cc0-fetch"
IMAGE_SUFFIXES = (".jpg", ".jpeg", ".png")


def request(url):
    return urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": USER_AGENT}), timeout=120)


def download_url(asset, attribute):
    data = json.load(request(API.format(asset=asset)))
    found = data.get("foundAssets", [])
    if not found:
        raise KeyError(f"{asset} not found on ambientCG")
    zips = found[0]["downloadFolders"]["default"]["downloadFiletypeCategories"]["zip"]["downloads"]
    for entry in zips:
        if entry["attribute"] == attribute:
            return entry["fullDownloadPath"]
    raise KeyError(f"{asset} has no {attribute} download")


def fetch(asset, attribute, dest):
    url = download_url(asset, attribute)
    folder = dest / asset
    folder.mkdir(parents=True, exist_ok=True)
    archive = folder / f"{asset}_{attribute}.zip"
    if not archive.exists():
        archive.write_bytes(request(url).read())
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    extracted = []
    with zipfile.ZipFile(archive) as bundle:
        for member in bundle.namelist():
            name = Path(member).name
            if name and name.lower().endswith(IMAGE_SUFFIXES):
                (folder / name).write_bytes(bundle.read(member))
                extracted.append(name)
    return {"asset": asset, "attribute": attribute, "url": url, "page": f"https://ambientcg.com/view?id={asset}", "licence": "CC0 1.0", "sha256": digest, "files": sorted(extracted)}


def main(argv=None):
    parser = argparse.ArgumentParser(description="Download CC0 materials from ambientCG and record source, licence and SHA-256.")
    parser.add_argument("assets", nargs="+")
    parser.add_argument("--dest", type=Path, default=Path("work/cc0"))
    parser.add_argument("--attribute", default="1K-JPG")
    parser.add_argument("--manifest", type=Path, default=Path("specs/textures/cc0_manifest.json"))
    args = parser.parse_args(argv)
    records = [fetch(asset, args.attribute, args.dest) for asset in args.assets]
    manifest = args.manifest
    manifest.parent.mkdir(parents=True, exist_ok=True)
    existing = json.loads(manifest.read_text(encoding="utf-8")) if manifest.exists() else []
    merged = {record["asset"]: record for record in existing}
    merged.update({record["asset"]: record for record in records})
    manifest.write_text(json.dumps(sorted(merged.values(), key=lambda record: record["asset"]), indent=1) + "\n", encoding="utf-8", newline="\n")
    for record in records:
        print(f"{record['asset']}: {len(record['files'])} images, sha256 {record['sha256'][:12]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
