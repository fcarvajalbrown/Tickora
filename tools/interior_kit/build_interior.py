import argparse
import csv
import json
import math
import random
import sys
from pathlib import Path

CM = 100.0
COLUMNS = ("piece", "x_cm", "y_cm", "z_cm", "yaw_deg", "scale", "scale_x", "folder", "instance")


def meshkit_modules():
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "meshkit"))
    import meshkit
    import parts
    return meshkit, parts


def load(spec_dir, name):
    return json.loads((spec_dir / f"{name}.json").read_text(encoding="utf-8"))


def facing_yaw(at, target, forward_deg):
    return math.degrees(math.atan2(target[1] - at[1], target[0] - at[0])) - forward_deg


def anchor_rows(anchors, forward_deg):
    rows = []
    for name in sorted(anchors):
        anchor = anchors[name]
        yaw = facing_yaw(anchor["at"], anchor["face"], forward_deg) if "face" in anchor else anchor.get("yaw", 0.0)
        rows.append({"piece": f"anchor_{name}", "at": anchor["at"], "yaw": yaw, "folder": "Anchors", "instance": False})
    return rows


def placement_rows(placements, instanced):
    rows = []
    for row in placements["rows"]:
        at = row["at"] if len(row["at"]) == 3 else [row["at"][0], row["at"][1], 0.0]
        yaw = facing_yaw(at, row["face"], placements.get("forward_deg", -90.0)) if "face" in row else row.get("yaw", 0.0)
        rows.append({"piece": row["piece"], "at": at, "yaw": yaw, "folder": row.get("folder", "Props"), "instance": instanced.get(row["piece"], False)})
    return rows


def csv_row(row):
    x, y, z = row["at"]
    return [row["piece"], round(x * CM, 1) + 0.0, round(-y * CM, 1) + 0.0, round(z * CM, 1) + 0.0, round(-row["yaw"], 2) + 0.0, 1.0, 1.0, row["folder"], int(row["instance"])]


def dent_parts(part):
    rng = random.Random(int(part["seed"]))
    found = []
    region = part["region"]
    for _ in range(rng.randint(int(part["count"][0]), int(part["count"][1]))):
        width = rng.uniform(*part["width"])
        height = rng.uniform(*part["height"])
        depth = rng.uniform(*part["depth"])
        x = rng.uniform(region["x"][0] + width / 2.0, region["x"][1] - width / 2.0)
        z = rng.uniform(region["z"][0] + height / 2.0, region["z"][1] - height / 2.0)
        half = width / 2.0
        slope = math.hypot(half, depth)
        angle = math.degrees(math.atan2(depth, half))
        y = float(part["sheet_y"]) - depth / 2.0
        for side in (-1.0, 1.0):
            found.append({
                "t": "box", "m": part["m"], "size": [round(slope, 4), float(part["thickness"]), round(height, 4)],
                "rot": [0, 0, round(angle * side, 3)], "at": [round(x + side * half / 2.0, 4), round(y, 4), round(z, 4)],
            })
    return found


def expanded(spec):
    parts = []
    for part in spec["parts"]:
        parts.extend(dent_parts(part) if part["t"] == "dents" else [part])
    return {**spec, "parts": parts}


def build(spec_dir, out):
    meshkit, parts = meshkit_modules()
    pieces = load(spec_dir, "pieces")
    placements = load(spec_dir, "placements")
    (out / "meshes").mkdir(parents=True, exist_ok=True)
    meshes, piece_meshes = {}, {}
    for piece in sorted(pieces):
        spec = pieces[piece]
        piece_meshes[piece] = []
        for group, mesh in sorted(parts.build_piece(expanded(spec)).items()):
            name = piece if group == "main" else f"{piece}_{group}"
            meshkit.write_glb(mesh, out / "meshes" / f"{name}.glb")
            meshes[name] = {
                "name": name, "files": [f"meshes/{name}.glb"], "piece": piece, "collision": spec.get("collision", "none"),
                "instance": spec.get("instance", False), "materials": sorted(mesh.merged().keys()), "triangles": mesh.triangle_count(),
            }
            piece_meshes[piece].append(name)
    instanced = {piece: spec.get("instance", False) for piece, spec in pieces.items()}
    rows = placement_rows(placements, instanced) + anchor_rows(placements.get("anchors", {}), placements.get("forward_deg", -90.0))
    unknown = sorted({row["piece"] for row in rows if not row["piece"].startswith("anchor_") and row["piece"] not in pieces})
    if unknown:
        raise SystemExit(f"placements name unknown pieces: {unknown}")
    with (out / "layout.csv").open("w", newline="", encoding="utf-8") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(COLUMNS)
        for row in rows:
            writer.writerow(csv_row(row))
    manifest = {"meshes": meshes, "pieces": piece_meshes}
    (out / "manifest.json").write_text(json.dumps(manifest, indent=1, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
    return meshes, rows


def main(argv=None):
    parser = argparse.ArgumentParser(description="Build an interior from meshkit pieces and a placement list: GLBs, layout.csv (meshkit's Unreal convention) and manifest.json.")
    parser.add_argument("--spec", type=Path, required=True, help="folder with pieces.json and placements.json")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args(argv)
    meshes, rows = build(args.spec, args.out)
    for name, entry in sorted(meshes.items(), key=lambda item: -item[1]["triangles"]):
        print(f"{name:28s} {entry['triangles']:6d} tris")
    print(f"{len(meshes)} meshes, {sum(e['triangles'] for e in meshes.values())} tris, {len(rows)} rows")
    return 0


if __name__ == "__main__":
    sys.exit(main())
