import argparse
import json
from pathlib import Path

import numpy as np
from PIL import Image


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description="Bake ambientCG maps into PSX albedo textures: colour times occlusion and height shading, tinted, alpha from the opacity map, downsized to one power-of-two size.")
    parser.add_argument("--spec", type=Path, required=True)
    parser.add_argument("--root", type=Path, default=Path("."))
    return parser.parse_args(argv)


def find_map(folder, suffix):
    found = sorted(folder.glob(f"*_{suffix}.*"))
    return found[0] if found else None


def grey(path, size):
    return np.asarray(Image.open(path).convert("L").resize((size, size), Image.Resampling.LANCZOS), dtype=np.float32) / 255.0


def bake(folder, entry, size):
    colour_path = find_map(folder, "Color")
    if colour_path is None:
        raise SystemExit(f"{folder}: no colour map")
    rgb = np.asarray(Image.open(colour_path).convert("RGB").resize((size, size), Image.Resampling.LANCZOS), dtype=np.float32) / 255.0
    shade = np.ones((size, size), dtype=np.float32)
    ao_path = find_map(folder, "AmbientOcclusion")
    if ao_path is not None and entry.get("ao", 0.0) > 0.0:
        shade *= 1.0 - entry["ao"] * (1.0 - grey(ao_path, size))
    height_path = find_map(folder, "Displacement")
    if height_path is not None and entry.get("height_shade", 0.0) > 0.0:
        height = grey(height_path, size)
        span = max(float(height.max() - height.min()), 1e-6)
        shade *= 1.0 - entry["height_shade"] * (1.0 - (height - height.min()) / span)
    contrast = float(entry.get("contrast", 1.0))
    if contrast != 1.0:
        rgb = rgb.mean(axis=(0, 1)) + (rgb - rgb.mean(axis=(0, 1))) * contrast
    saturation = float(entry.get("saturation", 1.0))
    if saturation != 1.0:
        luma = rgb @ np.asarray([0.2126, 0.7152, 0.0722], dtype=np.float32)
        rgb = luma[..., None] + (rgb - luma[..., None]) * saturation
    rgb = np.clip(rgb * shade[..., None] * np.asarray(entry.get("tint", [1, 1, 1]), dtype=np.float32), 0.0, 1.0)
    pixels = (rgb * 255.0 + 0.5).astype(np.uint8)
    if entry.get("alpha", False):
        opacity_path = find_map(folder, "Opacity")
        if opacity_path is None:
            raise SystemExit(f"{folder}: alpha asked but no opacity map")
        alpha = (grey(opacity_path, size) >= 0.5).astype(np.uint8) * 255
        return Image.fromarray(np.dstack([pixels, alpha]), "RGBA")
    return Image.fromarray(pixels, "RGB")


def main(argv=None):
    args = parse_args(argv)
    spec = json.loads(args.spec.read_text(encoding="utf-8"))
    out = args.root / spec["out"]
    out.mkdir(parents=True, exist_ok=True)
    for name, entry in sorted(spec["materials"].items()):
        if entry.get("generated", False):
            continue
        image = bake(args.root / spec["source_root"] / entry["asset"], entry, int(spec["size"]))
        image.save(out / f"{name}.png", optimize=False)
        print(f"{name}: {entry['asset']} -> {out / (name + '.png')}")
    table = {}
    for name, entry in spec["materials"].items():
        table[name] = {"tile_m": float(entry["tile_m"]), "alpha": bool(entry.get("alpha", False))}
        if "opacity" in entry:
            table[name]["opacity"] = float(entry["opacity"])
    (out / "materials.json").write_text(json.dumps(table, indent=1, sort_keys=True) + "\n", encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
