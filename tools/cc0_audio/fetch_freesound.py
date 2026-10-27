import argparse
import hashlib
import html
import json
import re
import subprocess
import sys
import urllib.request
from pathlib import Path

USER_AGENT = "Mozilla/5.0 tickora-cc0-fetch"
CC0_LINK = "creativecommons.org/publicdomain/zero/1.0"
PREVIEW = re.compile(r"https://cdn\.freesound\.org/previews/\d+/(\d+)_\d+-hq\.mp3")
TITLE = re.compile(r'<meta property="og:title" content="([^"]+)"')


def request(url):
    return urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": USER_AGENT}), timeout=120)


def sound_id(page_url):
    found = re.search(r"/sounds?/(\d+)/?$", page_url) or re.search(r"/s/(\d+)/?$", page_url)
    if not found:
        raise ValueError(f"{page_url} is not a Freesound sound page")
    return found.group(1)


def read_page(text, number):
    licences = set(re.findall(r"creativecommons\.org/(?:publicdomain|licenses)/[a-z-]+/[0-9.]+", text))
    previews = [match.group(0) for match in PREVIEW.finditer(text) if match.group(1) == number]
    title = TITLE.search(text)
    name, _, author = html.unescape(title.group(1)).rpartition(" by ") if title else ("", "", "")
    return {"licences": sorted(licences), "preview": previews[0] if previews else None, "title": name, "author": author}


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fetch(page_url, dest):
    number = sound_id(page_url)
    page = read_page(request(page_url).read().decode("utf-8", errors="replace"), number)
    if page["licences"] != [CC0_LINK]:
        raise ValueError(f"{page_url} licence is {page['licences']}, not only CC0")
    if page["preview"] is None:
        raise ValueError(f"{page_url} shows no HQ preview")
    dest.mkdir(parents=True, exist_ok=True)
    mp3 = dest / Path(page["preview"]).name
    if not mp3.exists():
        mp3.write_bytes(request(page["preview"]).read())
    wav = dest / f"fs_{number}.wav"
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(mp3), "-ac", "1", "-ar", "48000", "-c:a", "pcm_s16le", "-bitexact", str(wav)], check=True)
    return {"id": number, "title": page["title"], "author": page["author"], "page": page_url, "url": page["preview"], "licence": "CC0 1.0", "preview_sha256": sha256(mp3), "file": wav.name, "sha256": sha256(wav)}


def main(argv=None):
    parser = argparse.ArgumentParser(description="Download CC0 sounds' HQ previews from Freesound, decode them to 48 kHz mono WAV, and record source, licence and SHA-256.")
    parser.add_argument("pages", nargs="+", help="sound page URLs")
    parser.add_argument("--dest", type=Path, default=Path("work/freesound"))
    parser.add_argument("--manifest", type=Path, default=Path("specs/audio/freesound.json"))
    args = parser.parse_args(argv)
    records = [fetch(page, args.dest) for page in args.pages]
    manifest = args.manifest
    manifest.parent.mkdir(parents=True, exist_ok=True)
    existing = json.loads(manifest.read_text(encoding="utf-8")) if manifest.exists() else []
    merged = {record["id"]: record for record in existing}
    merged.update({record["id"]: record for record in records})
    manifest.write_text(json.dumps(sorted(merged.values(), key=lambda record: record["id"]), indent=1, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    for record in records:
        print(f"{record['id']} {record['title']} by {record['author']}: sha256 {record['sha256'][:12]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
