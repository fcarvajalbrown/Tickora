import argparse
import json
import sys
from pathlib import Path
from urllib.parse import urlsplit

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "radio_songs"))

from song_licences import licence_name


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description="Build the game's credits file from a JSON spec: fixed entries plus entries read from asset manifests, merged by section title.")
    parser.add_argument("--spec", type=Path, required=True)
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--out", type=Path, required=True)
    return parser.parse_args(argv)


def radio_songs(data):
    return [{"name": song["title"], "lines": [song["album"], song["licence"], song["page"]]} for song in data["songs"]]


def archive_manifest(data):
    files = ", ".join(Path(row["file"]).stem for row in data["files"])
    return [{"name": data["title"], "lines": [files, licence_name(data.get("licenseurl")), data["page"]]}]


def page_site(url):
    return urlsplit(url).netloc.removeprefix("www.")


def named_sources(data, skip_sites=()):
    return [{"name": row.get("title") or row["name"], "lines": ([row["author"]] if "author" in row else []) + [row["licence"], row["page"]]} for row in data if page_site(row["page"]) not in skip_sites]


def synth_sources(data, skip_authors=(), skip_urls=()):
    seen = set(skip_urls)
    entries = []
    for name, source in data.get("sources", {}).items():
        if source.get("author") in skip_authors or source["url"] in seen:
            continue
        seen.add(source["url"])
        entries.append({"name": source.get("name", name), "lines": ([source["author"]] if "author" in source else []) + [source["licence"], source["url"]]})
    return entries


def textures(data, skip_sites=()):
    sites = {}
    for row in data:
        site = row["page"].split("/")[2]
        if site in skip_sites:
            continue
        sites.setdefault(site, {"assets": [], "licences": []})
        sites[site]["assets"].append(row["asset"])
        if row["licence"] not in sites[site]["licences"]:
            sites[site]["licences"].append(row["licence"])
    names = {"ambientcg.com": "ambientCG", "polyhaven.com": "Poly Haven"}
    keepers = {"ambientcg.com": "Lennart Demes"}
    return [{"name": names.get(site, site), "lines": ([keepers[site]] if site in keepers else []) + [", ".join(entry["assets"]), ", ".join(entry["licences"]), f"https://{site}"]} for site, entry in sites.items()]


READERS = {
    "radio_songs": radio_songs,
    "archive_manifest": archive_manifest,
    "named_sources": named_sources,
    "synth_sources": synth_sources,
    "textures": textures,
}


PUBLIC_DOMAIN_LICENCES = ("CC0", "Public Domain")


def public_domain(entry):
    return any(line.startswith(PUBLIC_DOMAIN_LICENCES) for line in entry["lines"])


def entry_sites(entry):
    return {line.split("/")[2].removeprefix("www.") for line in entry["lines"] if line.startswith("http")}


def credited(entry, own_sites):
    return not public_domain(entry) or bool(entry_sites(entry) & set(own_sites))


def build(spec, root):
    sections = []
    own_sites = spec.get("public_domain_own_sites")
    for section in spec["sections"]:
        if "from" in section:
            reader = READERS.get(section["from"])
            if reader is None:
                raise SystemExit(f"unknown credits reader {section['from']}")
            entries = reader(json.loads((root / section["path"]).read_text(encoding="utf-8")), **section.get("options", {}))
            if own_sites is not None:
                entries = [entry for entry in entries if credited(entry, own_sites)]
        else:
            entries = section["entries"]
        existing = next((item for item in sections if item["title"] == section["title"]), None)
        if existing is None:
            sections.append({"title": section["title"], "entries": list(entries)})
        else:
            existing["entries"].extend(entries)
    return {"header": spec["header"], "sections": sections}


def main(argv=None):
    args = parse_args(argv)
    credits = build(json.loads(args.spec.read_text(encoding="utf-8")), args.root)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(credits, indent=1, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    print(f"{sum(len(section['entries']) for section in credits['sections'])} entries in {len(credits['sections'])} sections -> {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
