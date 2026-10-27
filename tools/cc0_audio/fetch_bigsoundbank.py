import argparse
import hashlib
import json
import re
import sys
import urllib.request
from pathlib import Path

SITE = "https://bigsoundbank.com"
WAV = SITE + "/UPLOAD/bwf-en/{number}.wav"
USER_AGENT = "tickora-cc0-fetch"
CC0_MARK = "CC0 (public domain)"
AUTHOR = "Joseph Sardin"


def request(url):
    return urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": USER_AGENT}), timeout=120)


def sound_number(page_url):
    found = re.search(r"-s(\d{4})\.html$", page_url)
    if not found:
        raise ValueError(f"{page_url} is not a BigSoundBank sound page")
    return found.group(1)


def read_page(html):
    title = re.search(r"<meta property='og:title' content=\"([^\"]+)\">", html)
    return {"title": title.group(1).split(" — ")[0].strip() if title else "", "cc0": CC0_MARK in html}


def fetch(page_url, dest):
    number = sound_number(page_url)
    page = read_page(request(page_url).read().decode("utf-8", errors="replace"))
    if not page["cc0"]:
        raise ValueError(f"{page_url} does not state {CC0_MARK}")
    dest.mkdir(parents=True, exist_ok=True)
    target = dest / f"bsb_{number}.wav"
    url = WAV.format(number=number)
    if not target.exists():
        target.write_bytes(request(url).read())
    digest = hashlib.sha256(target.read_bytes()).hexdigest()
    return {"id": number, "title": page["title"], "author": AUTHOR, "page": page_url, "url": url, "licence": "CC0 1.0", "sha256": digest, "file": target.name}


def main(argv=None):
    parser = argparse.ArgumentParser(description="Download CC0 sounds from BigSoundBank and record source, licence and SHA-256.")
    parser.add_argument("pages", nargs="+", help="sound page URLs, ending in -sNNNN.html")
    parser.add_argument("--dest", type=Path, default=Path("work/bigsoundbank"))
    parser.add_argument("--manifest", type=Path, default=Path("specs/audio/bigsoundbank.json"))
    args = parser.parse_args(argv)
    records = [fetch(page, args.dest) for page in args.pages]
    manifest = args.manifest
    manifest.parent.mkdir(parents=True, exist_ok=True)
    existing = json.loads(manifest.read_text(encoding="utf-8")) if manifest.exists() else []
    merged = {record["id"]: record for record in existing}
    merged.update({record["id"]: record for record in records})
    manifest.write_text(json.dumps(sorted(merged.values(), key=lambda record: record["id"]), indent=1, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    for record in records:
        print(f"{record['id']} {record['title']}: sha256 {record['sha256'][:12]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
