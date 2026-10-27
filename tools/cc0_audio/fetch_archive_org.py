import argparse
import hashlib
import json
import re
import time
import urllib.error
import urllib.parse
import urllib.request
from datetime import date
from pathlib import Path


def parse_args():
    parser = argparse.ArgumentParser(description="Download the files of one archive.org item, check each against the item's SHA-1 and write a manifest with SHA-256 for the licence record.")
    parser.add_argument("item")
    parser.add_argument("--dest", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, help="defaults to specs/audio/archive_org/<item>.json")
    parser.add_argument("--ext", default=".wav", help="comma-separated extensions to fetch")
    parser.add_argument("--match", default="", help="case-insensitive regular expression a file name must match")
    return parser.parse_args()


def fetch(url, attempts=5):
    for attempt in range(attempts):
        try:
            with urllib.request.urlopen(url, timeout=120) as response:
                return response.read()
        except urllib.error.HTTPError as error:
            if error.code < 500 or attempt == attempts - 1:
                raise
            time.sleep(3 * (attempt + 1))


def main():
    args = parse_args()
    metadata = json.loads(fetch(f"https://archive.org/metadata/{args.item}"))
    extensions = tuple(e.strip().lower() for e in args.ext.split(","))
    pattern = re.compile(args.match, re.IGNORECASE)
    args.dest.mkdir(parents=True, exist_ok=True)
    rows = []
    for entry in metadata["files"]:
        if not entry["name"].lower().endswith(extensions) or not pattern.search(entry["name"]):
            continue
        quoted = urllib.parse.quote(entry["name"])
        mirrors = [f"https://archive.org/download/{args.item}/{quoted}"] + [f"https://{server}{metadata['dir']}/{quoted}" for server in metadata.get("workable_servers", [])]
        data, url, sha1 = None, None, None
        for candidate in mirrors:
            try:
                body = fetch(candidate, attempts=2)
            except urllib.error.URLError as error:
                print(f"{entry['name']}: {candidate} failed ({error})")
                continue
            if hashlib.sha1(body).hexdigest() == entry.get("sha1"):
                data, url, sha1 = body, candidate, entry["sha1"]
                break
            print(f"{entry['name']}: {candidate} gave a SHA-1 mismatch")
        if data is None:
            raise SystemExit(f"{entry['name']}: no mirror returned the archive.org SHA-1 {entry.get('sha1')}")
        target = args.dest / entry["name"]
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        rows.append({"file": entry["name"], "source": entry.get("source"), "url": url, "bytes": len(data), "sha1": sha1, "sha256": hashlib.sha256(data).hexdigest()})
        print(f"{entry['name']}: {len(data)} bytes, SHA-1 matches")
    manifest = {
        "item": args.item,
        "page": f"https://archive.org/details/{args.item}",
        "title": metadata["metadata"].get("title"),
        "licenseurl": metadata["metadata"].get("licenseurl"),
        "uploader": metadata["metadata"].get("uploader"),
        "downloaded": date.today().isoformat(),
        "files": rows,
    }
    manifest_path = args.manifest or Path("specs/audio/archive_org") / f"{args.item}.json"
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(json.dumps(manifest, indent=1) + "
", encoding="utf-8", newline="
")
    print(f"{len(rows)} files, manifest {manifest_path}")


if __name__ == "__main__":
    main()
