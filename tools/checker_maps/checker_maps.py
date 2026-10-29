import argparse
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description="Write checker test patterns as ambientCG-style colour maps (<out>/<Name>/<Name>_Color.png), so psx_bake can bake them like a fetched material, and gobo patterns as <out>/<name>.png or one <out>/<name>_<frame>.png per frame.")
    parser.add_argument("--spec", type=Path, required=True)
    parser.add_argument("--root", type=Path, default=Path("."))
    return parser.parse_args(argv)


def checker(size, cells, colours):
    if size % cells:
        raise SystemExit(f"size {size} is not a multiple of cells {cells}")
    cell = size // cells
    index = (np.arange(size)[:, None] // cell + np.arange(size)[None, :] // cell) % len(colours)
    return np.asarray(colours, dtype=np.uint8)[index]


def bar_mask(size, panes, bar, border):
    if panes < 1 or bar < 1 or border < 0 or 2 * border >= size:
        raise SystemExit(f"panes {panes}, bar {bar} and border {border} do not fit {size} pixels")
    position = np.arange(size)
    mask = (position < border) | (position >= size - border)
    for index in range(1, panes):
        start = index * size // panes - bar // 2
        mask |= (position >= start) & (position < start + bar)
    return mask


def grid(size, panes, bar, border, light, dark):
    dark_columns = bar_mask(size, int(panes[0]), bar, border)
    dark_rows = bar_mask(size, int(panes[1]), bar, border)
    mask = dark_rows[:, None] | dark_columns[None, :]
    colours = np.asarray([light, dark], dtype=np.uint8)
    return colours[mask.astype(np.uint8)]


def levels(size, values):
    return [np.full((size, size, 3), value, dtype=np.uint8) for value in values]


def gobo_frames(entry):
    size = int(entry["size"])
    if entry["kind"] == "grid":
        return [grid(size, entry["panes"], int(entry["bar"]), int(entry["border"]), entry["light"], entry["dark"])]
    if entry["kind"] == "levels":
        return levels(size, [int(value) for value in entry["levels"]])
    raise SystemExit(f"unknown gobo kind {entry['kind']!r}")


def write_patterns(out, patterns):
    for name, entry in sorted(patterns.items()):
        folder = out / name
        folder.mkdir(parents=True, exist_ok=True)
        path = folder / f"{name}_Color.png"
        Image.fromarray(checker(int(entry["size"]), int(entry["cells"]), entry["colours"]), "RGB").save(path, optimize=False)
        print(f"{name}: {path}")


def write_gobos(out, gobos):
    out.mkdir(parents=True, exist_ok=True)
    for name, entry in sorted(gobos.items()):
        frames = gobo_frames(entry)
        names = [f"{name}.png"] if len(frames) == 1 else [f"{name}_{index}.png" for index in range(len(frames))]
        for file_name, pixels in zip(names, frames):
            path = out / file_name
            Image.fromarray(pixels, "RGB").save(path, optimize=False)
            print(f"{name}: {path}")


def main(argv=None):
    args = parse_args(argv)
    spec = json.loads(args.spec.read_text(encoding="utf-8"))
    out = args.root / spec["out"]
    write_patterns(out, spec.get("patterns", {}))
    write_gobos(out, spec.get("gobos", {}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
