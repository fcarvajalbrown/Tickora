import argparse
import html
import json
import re
import sys
import urllib.parse
import urllib.request
from pathlib import Path

AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36"
COMMONS_AGENT = "Tickora-cc0-fetch/1.0 (https://github.com/fcarvajalbrown/Tickora)"
BIGSOUNDBANK = "https://bigsoundbank.com/search?q={}"
FREESOUND = "https://freesound.org/search/?q={}&f=license:%22Creative+Commons+0%22"
OPENGAMEART = "https://opengameart.org/art-search-advanced?keys={}&field_art_type_tid%5B%5D=13&field_art_license_tid%5B%5D=4"
COMMONS = "https://commons.wikimedia.org/w/api.php"
RED_LIBRARY_LIST = "https://archive.org/advancedsearch.php?q=identifier%3ARed_Library_*&fl[]=identifier&rows=100&output=json"
OGA_SKIP = ("/content/faq", "-art-challenge-")


def get(url, agent=AGENT, timeout=60):
    with urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": agent}), timeout=timeout) as response:
        return response.read().decode("utf-8", "replace")


def bigsoundbank(query):
    page = get(BIGSOUNDBANK.format(urllib.parse.quote_plus(query)))
    seen = {}
    for slug in re.findall(r"href=['\"]/([a-z0-9-]+-s\d+\.html)['\"]", page):
        seen.setdefault(slug, {"title": re.sub(r"-s\d+\.html$", "", slug).replace("-", " "), "page": f"https://bigsoundbank.com/{slug}"})
    return list(seen.values())


def freesound(query):
    page = get(FREESOUND.format(urllib.parse.quote_plus(query)))
    seen = {}
    for path, title in re.findall(r'href="(/people/[^"]+/sounds/\d+/)"[^>]*title="([^"]*)"', page):
        seen.setdefault(path, {"title": html.unescape(title), "page": f"https://freesound.org{path}"})
    return list(seen.values())


def opengameart(query):
    page = get(OPENGAMEART.format(urllib.parse.quote_plus(query)))
    seen = {}
    for path, title in re.findall(r'href="(/content/[^"]+)"[^>]*>([^<]+)<', page):
        if any(skip in path for skip in OGA_SKIP):
            continue
        seen.setdefault(path, {"title": html.unescape(title).strip(), "page": f"https://opengameart.org{path}"})
    return list(seen.values())


def commons(query, limit=20):
    params = {"action": "query", "list": "search", "srsearch": f"{query} filetype:audio", "srnamespace": 6, "srlimit": limit, "format": "json"}
    data = json.loads(get(COMMONS + "?" + urllib.parse.urlencode(params), COMMONS_AGENT))
    return [{"title": hit["title"], "page": "https://commons.wikimedia.org/wiki/" + urllib.parse.quote(hit["title"].replace(" ", "_"))} for hit in data.get("query", {}).get("search", [])]


def red_library_catalog(cache):
    if cache.exists():
        return json.loads(cache.read_text(encoding="utf-8"))
    items = [doc["identifier"] for doc in json.loads(get(RED_LIBRARY_LIST))["response"]["docs"]]
    catalog = {}
    for identifier in items:
        metadata = json.loads(get(f"https://archive.org/metadata/{identifier}", timeout=120))
        catalog[identifier] = [{"name": entry["name"], "title": entry.get("title", ""), "length": entry.get("length", "")} for entry in metadata.get("files", []) if entry.get("source") == "original" and entry["name"].lower().endswith(".wav")]
    cache.parent.mkdir(parents=True, exist_ok=True)
    cache.write_text(json.dumps(catalog, indent=1), encoding="utf-8")
    return catalog


def red_library(query, catalog):
    words = [re.escape(word) for word in query.split()]
    pattern = re.compile("".join(f"(?=.*{word})" for word in words), re.IGNORECASE)
    hits = []
    for identifier, files in catalog.items():
        for entry in files:
            if pattern.search(entry["name"] + " " + entry["title"]):
                hits.append({"title": f"{entry['name']} | {entry['title']}", "page": f"https://archive.org/details/{identifier}", "item": identifier})
    return hits


def main(argv=None):
    parser = argparse.ArgumentParser(description="Search every approved CC0 sound source (BigSoundBank, Freesound CC0, the Red Library on archive.org, Wikimedia Commons, OpenGameArt CC0 sound effects) with each query, print the hits per source and write a JSON report of the sources and queries searched.")
    parser.add_argument("queries", nargs="+", help="search terms, one per argument")
    parser.add_argument("--out", type=Path, required=True, help="JSON report path")
    parser.add_argument("--cache", type=Path, default=Path("work/cc0_cache/red_library.json"), help="Red Library file catalog, fetched once")
    parser.add_argument("--show", type=int, default=12, help="hits printed per source and query")
    args = parser.parse_args(argv)
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    catalog = red_library_catalog(args.cache)
    sources = {"bigsoundbank": bigsoundbank, "freesound": freesound, "red_library": lambda query: red_library(query, catalog), "commons": commons, "opengameart": opengameart}
    report = {"queries": args.queries, "sources": {}}
    for name, search in sources.items():
        report["sources"][name] = {}
        for query in args.queries:
            try:
                hits = search(query)
            except Exception as error:
                hits = [{"error": str(error)}]
            report["sources"][name][query] = hits
            print(f"{name} | {query} | {len(hits)} hits")
            for hit in hits[:args.show]:
                print(f"  {hit.get('title', hit.get('error'))} | {hit.get('page', '')}")
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, indent=1, ensure_ascii=False), encoding="utf-8")


if __name__ == "__main__":
    main()
