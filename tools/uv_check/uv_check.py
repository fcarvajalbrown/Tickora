import argparse
import json
import re
import sys
from pathlib import Path

import numpy as np
import trimesh

RASTER = 256
DEGENERATE_M2 = 1e-8
SCAN_SUFFIXES = {".gd", ".tscn", ".tres", ".json", ".cfg"}
SKIP_DIRS = {".godot", "probes"}
TEXTURE_SLOTS = ("image", "baseColorTexture", "normalTexture", "metallicRoughnessTexture", "occlusionTexture", "emissiveTexture")


def referenced_glbs(game_root):
    texts = []
    for path in game_root.rglob("*"):
        if path.suffix not in SCAN_SUFFIXES or SKIP_DIRS & set(path.relative_to(game_root).parts):
            continue
        texts.append(path.read_text(encoding="utf-8", errors="ignore"))
    corpus = "\n".join(texts)
    found = []
    for glb in sorted((game_root / "assets" / "gen").rglob("*.glb")):
        if re.search(r"[\"/]" + re.escape(glb.stem) + r"(\.glb)?\"", corpus):
            found.append(glb)
    return found


def coverage(uv, faces):
    counts = np.zeros((RASTER, RASTER), dtype=np.int32)
    centres = (np.arange(RASTER) + 0.5) / RASTER
    for tri in uv[faces]:
        lo = np.clip(np.floor(tri.min(axis=0) * RASTER).astype(int), 0, RASTER - 1)
        hi = np.clip(np.ceil(tri.max(axis=0) * RASTER).astype(int), 0, RASTER)
        if np.any(hi <= lo):
            continue
        xs, ys = np.meshgrid(centres[lo[0]:hi[0]], centres[lo[1]:hi[1]], indexing="ij")
        a, b, c = tri
        d = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1])
        if abs(d) < 1e-14:
            continue
        w0 = ((b[1] - c[1]) * (xs - c[0]) + (c[0] - b[0]) * (ys - c[1])) / d
        w1 = ((c[1] - a[1]) * (xs - c[0]) + (a[0] - c[0]) * (ys - c[1])) / d
        inside = (w0 >= 0) & (w1 >= 0) & (w0 + w1 <= 1)
        counts[lo[0]:hi[0], lo[1]:hi[1]] += inside
    used = counts > 0
    return int(used.sum()), int((counts > 1).sum())


def samples_a_texture(mesh):
    material = getattr(mesh.visual, "material", None)
    return any(getattr(material, slot, None) is not None for slot in TEXTURE_SLOTS)


def check_primitive(name, mesh):
    uv = getattr(mesh.visual, "uv", None)
    report = {
        "primitive": name,
        "triangles": int(len(mesh.faces)),
        "has_uv": uv is not None and len(uv) == len(mesh.vertices),
        "needs_uv": samples_a_texture(mesh),
    }
    if not report["has_uv"]:
        return report
    uv = np.asarray(uv, dtype=np.float64)
    corners = uv[mesh.faces]
    signed = (corners[:, 1, 0] - corners[:, 0, 0]) * (corners[:, 2, 1] - corners[:, 0, 1]) - (corners[:, 2, 0] - corners[:, 0, 0]) * (corners[:, 1, 1] - corners[:, 0, 1])
    solid = mesh.area_faces >= DEGENERATE_M2
    outside = np.any((corners < -1e-4) | (corners > 1 + 1e-4), axis=(1, 2))
    inside_faces = mesh.faces[~outside]
    used, overlapped = coverage(uv, inside_faces) if len(inside_faces) else (0, 0)
    report.update({
        "uv_min": [round(float(v), 3) for v in uv.min(axis=0)],
        "uv_max": [round(float(v), 3) for v in uv.max(axis=0)],
        "outside_square": int(outside.sum()),
        "zero_area": int(np.sum((np.abs(signed) < 1e-12) & solid, keepdims=False)),
        "flipped": int(min(np.sum(signed < -1e-12), np.sum(signed > 1e-12))),
        "used_share": round(used / RASTER ** 2, 3),
        "overlap_share": round(overlapped / max(used, 1), 3),
    })
    return report


def check(path, square=False):
    scene = trimesh.load_scene(path, process=False)
    primitives = [check_primitive(name, geometry) for name, geometry in scene.geometry.items() if isinstance(geometry, trimesh.Trimesh)]
    missing = sum(p["needs_uv"] and not p["has_uv"] for p in primitives)
    outside = sum(p.get("outside_square", 0) for p in primitives)
    zero = sum(p.get("zero_area", 0) for p in primitives)
    overlap = max((p.get("overlap_share", 0.0) for p in primitives), default=0.0)
    return {
        "file": path.as_posix(),
        "primitives": primitives,
        "missing_uv": missing,
        "outside_square": outside,
        "zero_area": zero,
        "worst_overlap_share": overlap,
        "clean": missing == 0 and zero == 0 and (not square or (outside == 0 and overlap == 0.0)),
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description="Check GLB UVs per primitive. A mesh fails when a primitive whose material samples a texture (base colour, normal, metallic-roughness, occlusion or emissive) has no UVs, or a triangle with real area has a zero-area UV triangle; triangles under 1 mm2 are check_mesh's degenerate triangles and are skipped. With --square it also fails on UVs outside the 0-1 square or overlapping UV islands (rasterised), which tiling textures use on purpose. Prints one JSON line per file and exits 1 when any file is not clean.")
    parser.add_argument("paths", nargs="*", type=Path)
    parser.add_argument("--game-root", type=Path, help="check every GLB under <root>/assets/gen whose name a game script, scene or data file references")
    parser.add_argument("--square", action="store_true", help="also require UVs inside the 0-1 square with no overlaps")
    parser.add_argument("--summary", action="store_true", help="print a one-line table row per file instead of JSON")
    args = parser.parse_args(argv)
    paths = list(args.paths) + (referenced_glbs(args.game_root) if args.game_root else [])
    if not paths:
        parser.error("give GLB paths or --game-root")
    failed = 0
    for path in paths:
        report = check(path, args.square)
        failed += not report["clean"]
        if args.summary:
            status = "ok" if report["clean"] else "FAIL"
            print(f"{path.name:40s} prims {len(report['primitives']):3d}  no-uv {report['missing_uv']:3d}  outside {report['outside_square']:6d}  zero {report['zero_area']:5d}  overlap {report['worst_overlap_share']:.3f}  {status}")
        else:
            print(json.dumps(report))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
