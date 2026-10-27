import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import fetch_ambientcg
import fetch_polyhaven

POLYHAVEN_PREFIX = "polyhaven-"


def default_fetchers():
    return {
        "ambientcg": lambda record, dest: fetch_ambientcg.fetch(record["asset"], record["attribute"], dest),
        "polyhaven": lambda record, dest: fetch_polyhaven.fetch(record["asset"], *record["attribute"][len(POLYHAVEN_PREFIX):].split("-", 1), dest),
    }


def source_of(record):
    return "polyhaven" if record["attribute"].startswith(POLYHAVEN_PREFIX) else "ambientcg"


def missing_assets(spec, root):
    source_root = root / spec["source_root"]
    wanted = sorted({entry["asset"] for entry in spec["materials"].values() if not entry.get("generated", False)})
    return [asset for asset in wanted if not (source_root / asset).is_dir()]


def fetch_missing(spec, manifest, root, fetchers):
    records = {record["asset"]: record for record in manifest}
    dest = root / spec["source_root"]
    fetched = []
    for asset in missing_assets(spec, root):
        if asset not in records:
            raise SystemExit(f"{asset}: not in the CC0 manifest, fetch it with fetch_ambientcg.py or fetch_polyhaven.py first")
        record = records[asset]
        result = fetchers[source_of(record)](record, dest)
        if result["sha256"] != record["sha256"]:
            raise SystemExit(f"{asset}: sha256 {result['sha256']} does not match the manifest {record['sha256']}")
        fetched.append(result)
    if fetched:
        fetch_polyhaven.merge_manifest(dest / "manifest.json", fetched)
    return [record["asset"] for record in fetched]


def main(argv=None):
    parser = argparse.ArgumentParser(description="Fetch every CC0 source a PSX bake spec needs and is missing, from ambientCG or Poly Haven as the manifest records it, and check each download's SHA-256 against the manifest.")
    parser.add_argument("--spec", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, default=Path("specs/textures/cc0_manifest.json"))
    parser.add_argument("--root", type=Path, default=Path("."))
    args = parser.parse_args(argv)
    spec = json.loads(args.spec.read_text(encoding="utf-8"))
    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    for asset in fetch_missing(spec, manifest, args.root, default_fetchers()):
        print(f"{asset}: fetched")
    return 0


if __name__ == "__main__":
    sys.exit(main())
