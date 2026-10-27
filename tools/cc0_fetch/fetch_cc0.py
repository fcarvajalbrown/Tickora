import argparse
import hashlib
import json
import sys
import urllib.request
import zipfile
from pathlib import Path


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def download(url, target):
    target.parent.mkdir(parents=True, exist_ok=True)
    request = urllib.request.Request(url, headers={"User-Agent": "cc0-fetch/1.0"})
    with urllib.request.urlopen(request) as response, open(target, "wb") as handle:
        while block := response.read(1 << 20):
            handle.write(block)
    return target


def unpack(archive, dest):
    root = dest.resolve()
    written = []
    with zipfile.ZipFile(archive) as handle:
        for info in handle.infolist():
            if info.is_dir():
                continue
            target = (root / info.filename).resolve()
            if not target.is_relative_to(root):
                continue
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(handle.read(info))
            written.append(target)
    return written


def record(manifest, name, url, page, licence, archive):
    rows = json.loads(manifest.read_text(encoding="utf-8")) if manifest.exists() else []
    rows = [row for row in rows if row["name"] != name]
    rows.append({"name": name, "url": url, "page": page, "licence": licence, "sha256": sha256(archive)})
    manifest.parent.mkdir(parents=True, exist_ok=True)
    manifest.write_text(json.dumps(rows, indent=1) + "\n", encoding="utf-8")


def main(argv=None):
    parser = argparse.ArgumentParser(description="Download a CC0 archive, unpack it safely and record source, licence and SHA-256")
    parser.add_argument("url")
    parser.add_argument("--name", required=True)
    parser.add_argument("--page", required=True)
    parser.add_argument("--licence", default="CC0-1.0")
    parser.add_argument("--dest", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, default=Path("specs/cc0/manifest.json"))
    args = parser.parse_args(argv)
    archive = download(args.url, args.dest / "archives" / Path(args.url).name)
    written = unpack(archive, args.dest / args.name) if zipfile.is_zipfile(archive) else [archive]
    record(args.manifest, args.name, args.url, args.page, args.licence, archive)
    print(f"{args.name}: {len(written)} files -> {args.dest / args.name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
