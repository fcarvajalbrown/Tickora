import argparse
import difflib
import json
import re
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

AUDIO_EXTENSIONS = (".mp3", ".ogg", ".flac", ".wav")
LENGTH_TOLERANCE_S = 3.0
NAME_MATCH = 0.75


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description="Read the licence of every archive.org item behind a song list and write one row per song: item, licence URL and name, whether it is Creative Commons, and the item's audio file that matches the song.")
    parser.add_argument("songs", type=Path, help="JSON song list: a list, or an object holding one, of entries with kind, title, collection, duration and an archive.org source URL")
    parser.add_argument("--match", default="", help="case-insensitive regular expression tested against title and collection")
    parser.add_argument("--min-s", type=float, default=0.0)
    parser.add_argument("--max-s", type=float, default=1e9)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--cache", type=Path, help="folder keeping each item's metadata so a rerun needs no network")
    return parser.parse_args(argv)


def is_creative_commons(url):
    return bool(url) and "creativecommons.org/" in url


def licence_name(url):
    if not url:
        return "none stated"
    found = re.search(r"creativecommons\.org/(licenses|publicdomain)/([a-z\-]+)/([\d.]+)", url)
    if not found:
        return url
    kind, version = found.group(2), found.group(3)
    if found.group(1) == "publicdomain":
        return f"CC0 {version}" if kind == "zero" else f"Public Domain Mark {version}"
    return f"CC {kind.upper()} {version}"


def load_songs(path):
    data = json.loads(path.read_text(encoding="utf-8"))
    if isinstance(data, dict):
        data = next(value for value in data.values() if isinstance(value, list))
    return data


def select(songs, pattern, min_s, max_s):
    regex = re.compile(pattern, re.IGNORECASE)
    return [song for song in songs if song.get("kind") == "song" and min_s <= float(song.get("duration", 0.0)) <= max_s and regex.search(f"{song.get('title', '')} {song.get('collection', '')}")]


def item_of(song):
    return song["source"].rstrip("/").rsplit("/", 1)[-1]


def seconds(length):
    if length is None:
        return None
    text = str(length)
    if ":" in text:
        total = 0.0
        for part in text.split(":"):
            total = total * 60 + float(part)
        return total
    return float(text)


def audio_file(files, title, duration):
    best = None
    for entry in files:
        name = entry["name"]
        if not name.lower().endswith(AUDIO_EXTENSIONS):
            continue
        length = seconds(entry.get("length"))
        gap = abs(length - duration) if length is not None else float("inf")
        named = difflib.SequenceMatcher(None, title.lower(), re.sub(r"^[\d\s.\-]+", "", Path(name).stem.lower())).ratio()
        if named < NAME_MATCH and gap > LENGTH_TOLERANCE_S:
            continue
        score = (named >= NAME_MATCH, named, name.lower().endswith(".mp3"), -gap)
        if best is None or score > best[0]:
            best = (score, name)
    return best[1] if best else None


def metadata(item, cache):
    cached = cache / f"{item}.json" if cache else None
    if cached and cached.is_file():
        return json.loads(cached.read_text(encoding="utf-8"))
    for attempt in range(4):
        try:
            with urllib.request.urlopen(f"https://archive.org/metadata/{item}", timeout=60) as response:
                data = json.loads(response.read())
            break
        except urllib.error.URLError:
            if attempt == 3:
                raise
            time.sleep(3 * (attempt + 1))
    if cached:
        cached.parent.mkdir(parents=True, exist_ok=True)
        cached.write_text(json.dumps(data), encoding="utf-8")
    return data


def main(argv=None):
    args = parse_args(argv)
    rows = []
    for song in select(load_songs(args.songs), args.match, args.min_s, args.max_s):
        item = item_of(song)
        meta = metadata(item, args.cache)
        url = meta.get("metadata", {}).get("licenseurl")
        rows.append({
            "id": song.get("id"),
            "title": song.get("title"),
            "collection": song.get("collection"),
            "duration": song.get("duration"),
            "item": item,
            "file": audio_file(meta.get("files", []), song.get("title", ""), float(song.get("duration", 0.0))),
            "licenseurl": url,
            "licence": licence_name(url),
            "creative_commons": is_creative_commons(url),
        })
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(rows, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    licensed = sum(row["creative_commons"] for row in rows)
    print(f"{len(rows)} songs, {licensed} under Creative Commons -> {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
