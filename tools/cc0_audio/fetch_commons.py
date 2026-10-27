import argparse
import hashlib
import html
import json
import re
import subprocess
import sys
import urllib.parse
import urllib.request
from pathlib import Path

USER_AGENT = "Tickora-cc0-fetch/1.0 (https://github.com/fcarvajalbrown/Tickora)"
API = "https://commons.wikimedia.org/w/api.php"
FREE_CODES = {"cc0", "pd"}
FREE_NAMES = ("cc0", "public domain", "pd-")


def request(params):
    url = API + "?" + urllib.parse.urlencode(dict(params, format="json"))
    with urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": USER_AGENT}), timeout=60) as response:
        return json.loads(response.read().decode("utf-8"))


def download(url, path):
    with urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": USER_AGENT}), timeout=120) as response:
        path.write_bytes(response.read())


def plain(value):
    return html.unescape(re.sub(r"<[^>]+>", "", str(value or ""))).strip()


def is_free(meta):
    code = plain(meta.get("License", {}).get("value", "")).lower()
    name = plain(meta.get("LicenseShortName", {}).get("value", "")).lower()
    if code in FREE_CODES or code.startswith("cc0") or code.startswith("pd"):
        return True
    return name.startswith(FREE_NAMES)


def file_info(titles):
    data = request({"action": "query", "titles": "|".join(titles), "prop": "imageinfo", "iiprop": "url|sha1|extmetadata|mime"})
    infos = []
    for page in data.get("query", {}).get("pages", {}).values():
        info = (page.get("imageinfo") or [{}])[0]
        meta = info.get("extmetadata", {})
        infos.append({"title": page.get("title", ""), "pageid": page.get("pageid", -1), "url": info.get("url", ""), "page": info.get("descriptionurl", ""), "mime": info.get("mime", ""), "licence": plain(meta.get("LicenseShortName", {}).get("value", "")), "author": plain(meta.get("Artist", {}).get("value", "")), "description": plain(meta.get("ImageDescription", {}).get("value", ""))[:200], "free": is_free(meta)})
    return infos


def search(query, limit):
    data = request({"action": "query", "list": "search", "srsearch": f"{query} filetype:audio", "srnamespace": 6, "srlimit": limit})
    titles = [hit["title"] for hit in data.get("query", {}).get("search", [])]
    rows = []
    for start in range(0, len(titles), 40):
        rows.extend(file_info(titles[start:start + 40]))
    return rows


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fetch(title, dest):
    infos = file_info([title])
    if not infos or not infos[0]["url"]:
        raise ValueError(f"{title} not found")
    info = infos[0]
    if not info["free"]:
        raise ValueError(f"{title} licence is {info['licence']!r}, not CC0 or public domain")
    dest.mkdir(parents=True, exist_ok=True)
    original = dest / f"commons_{info['pageid']}{Path(urllib.parse.urlparse(info['url']).path).suffix}"
    if not original.exists():
        download(info["url"], original)
    wav = dest / f"commons_{info['pageid']}.wav"
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(original), "-ac", "1", "-ar", "48000", "-c:a", "pcm_s16le", "-bitexact", str(wav)], check=True)
    return {"id": info["pageid"], "title": info["title"], "author": info["author"], "page": info["page"], "url": info["url"], "licence": info["licence"], "original_sha256": sha256(original), "file": wav.name, "sha256": sha256(wav)}


def main(argv=None):
    parser = argparse.ArgumentParser(description="Search Wikimedia Commons audio, or download CC0 and public-domain files only, decode them to 48 kHz mono WAV, and record source, author, licence and SHA-256.")
    parser.add_argument("titles", nargs="*", help="file titles, e.g. 'File:Creaky wooden casket.ogg'")
    parser.add_argument("--search", help="list audio files matching this query with their licences, download nothing")
    parser.add_argument("--limit", type=int, default=30)
    parser.add_argument("--dest", type=Path, default=Path("work/commons"))
    parser.add_argument("--manifest", type=Path, default=Path("specs/audio/commons.json"))
    args = parser.parse_args(argv)
    if args.search:
        for row in search(args.search, args.limit):
            print(f"{'FREE' if row['free'] else 'no  '}  {row['licence'][:24]:24s}  {row['title']}  |  {row['description'][:90]}")
        return 0
    manifest = args.manifest
    manifest.parent.mkdir(parents=True, exist_ok=True)
    existing = json.loads(manifest.read_text(encoding="utf-8")) if manifest.exists() else []
    merged = {record["id"]: record for record in existing}
    failed = 0
    for title in args.titles:
        try:
            record = fetch(title, args.dest)
            merged[record["id"]] = record
            print(f"{record['id']} {record['title']} by {record['author']}: {record['licence']}, sha256 {record['sha256'][:12]}")
        except Exception as error:
            failed += 1
            print(f"refused {title}: {error}")
    args.dest.mkdir(parents=True, exist_ok=True)
    manifest.write_text(json.dumps(sorted(merged.values(), key=lambda record: record["id"]), indent=1, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
