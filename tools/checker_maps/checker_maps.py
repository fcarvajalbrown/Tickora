import argparse
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description="Write checker test patterns as ambientCG-style colour maps (<out>/<Name>/<Name>_Color.png), so psx_bake can bake them like a fetched material.")
    parser.add_argument("--spec", type=Path, required=True)
    parser.add_argument("--root", type=Path, default=Path("."))
    return parser.parse_args(argv)


def checker(size, cells, colours):
    if size % cells:
        raise SystemExit(f"size {size} is not a multiple of cells {cells}")
    cell = size // cells
    index = (np.arange(size)[:, None] // cell + np.arange(size)[None, :] // cell) % len(colours)
    return np.asarray(colours, dtype=np.uint8)[index]


def main(argv=None):
    args = parse_args(argv)
    spec = json.loads(args.spec.read_text(encoding="utf-8"))
    out = args.root / spec["out"]
    for name, entry in sorted(spec["patterns"].items()):
        folder = out / name
        folder.mkdir(parents=True, exist_ok=True)
        path = folder / f"{name}_Color.png"
        Image.fromarray(checker(int(entry["size"]), int(entry["cells"]), entry["colours"]), "RGB").save(path, optimize=False)
        print(f"{name}: {path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
