import argparse
import json
import re
import urllib.request


def item_files(identifier):
    with urllib.request.urlopen(f"https://archive.org/metadata/{identifier}") as response:
        metadata = json.load(response)
    licence = metadata.get("metadata", {}).get("licenseurl", "")
    return licence, [entry for entry in metadata.get("files", []) if entry.get("source") == "original"]


def main():
    parser = argparse.ArgumentParser(description="List the original files of archive.org items whose names or titles match keywords.")
    parser.add_argument("items", nargs="+", help="archive.org identifiers")
    parser.add_argument("--match", required=True, help="case-insensitive regular expression tested against file name and title")
    parser.add_argument("--extension", default=".wav", help="only files with this extension")
    args = parser.parse_args()
    pattern = re.compile(args.match, re.IGNORECASE)
    for identifier in args.items:
        licence, files = item_files(identifier)
        hits = [entry for entry in files if entry["name"].lower().endswith(args.extension) and pattern.search(entry["name"] + " " + entry.get("title", ""))]
        print(f"{identifier} | {licence} | {len(hits)} of {len(files)} files match")
        for entry in hits:
            seconds = float(entry.get("length", 0) or 0)
            print(f"  {entry['name']} | {entry.get('title', '')} | {seconds:.1f} s | {int(entry.get('size', 0)) // 1000} kB")


if __name__ == "__main__":
    main()
