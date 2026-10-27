import argparse
import json
import sys
from pathlib import Path

PLANE_TOLERANCE_M = 0.005
MIN_OVERLAP_M = 0.0001


def wall_segments(length, height, openings, extend):
    segments = []
    cursor = -extend
    for opening in sorted(openings, key=lambda o: o["at"]):
        x0 = opening["at"] - opening["width"] / 2
        x1 = opening["at"] + opening["width"] / 2
        if x0 > cursor:
            segments.append((cursor, x0, 0.0, height))
        if opening["sill"] > 0:
            segments.append((x0, x1, 0.0, opening["sill"]))
        if opening["head"] < height:
            segments.append((x0, x1, opening["head"], height))
        cursor = x1
    if length > cursor:
        segments.append((cursor, length, 0.0, height))
    return segments


def part_boxes(part):
    if part.get("rot"):
        return []
    at = part.get("at", [0.0, 0.0, 0.0])
    if part["t"] == "box":
        low = [at[axis] - part["size"][axis] / 2 for axis in range(3)]
        high = [at[axis] + part["size"][axis] / 2 for axis in range(3)]
        return [(low, high)]
    if part["t"] == "wall":
        half = part["thickness"] / 2
        extend = half if part.get("extend") else 0.0
        return [
            ([at[0] + x0, at[1] - half, at[2] + z0], [at[0] + x1, at[1] + half, at[2] + z1])
            for x0, x1, z0, z1 in wall_segments(part["length"], part["height"], part.get("openings", []), extend)
        ]
    return []


def box_faces(low, high):
    faces = []
    for axis in range(3):
        others = [other for other in range(3) if other != axis]
        rect = [(low[other], high[other]) for other in others]
        faces.append((axis, -1, low[axis], rect))
        faces.append((axis, 1, high[axis], rect))
    return faces


def overlap(first, second):
    return [min(a[1], b[1]) - max(a[0], b[0]) for a, b in zip(first, second)]


def on_ground(face, ground_m):
    return face[0] == 2 and face[1] == -1 and abs(face[2] - ground_m) <= PLANE_TOLERANCE_M


def clashes(parts, ground_m=0.0):
    faces = []
    for index, part in enumerate(parts):
        for low, high in part_boxes(part):
            faces.extend((index, face) for face in box_faces(low, high) if not on_ground(face, ground_m))
    found = []
    for i, (part_a, face_a) in enumerate(faces):
        for part_b, face_b in faces[i + 1:]:
            if part_a == part_b:
                continue
            if face_a[0] != face_b[0] or face_a[1] != face_b[1]:
                continue
            if abs(face_a[2] - face_b[2]) > PLANE_TOLERANCE_M:
                continue
            spans = overlap(face_a[3], face_b[3])
            if all(span > MIN_OVERLAP_M for span in spans):
                found.append({"parts": [part_a, part_b], "axis": "xyz"[face_a[0]], "side": face_a[1],
                              "plane": [round(face_a[2], 4), round(face_b[2], 4)],
                              "overlap": [round(span, 4) for span in spans]})
    return found


def main():
    parser = argparse.ArgumentParser(description="Report same-facing coplanar faces between axis-aligned parts of meshkit pieces")
    parser.add_argument("spec", type=Path)
    parser.add_argument("--pieces", nargs="*")
    args = parser.parse_args()
    pieces = json.loads(args.spec.read_text(encoding="utf-8"))
    names = args.pieces or sorted(pieces)
    total = 0
    for name in names:
        for clash in clashes(pieces[name]["parts"]):
            print(json.dumps({"piece": name, **clash}))
            total += 1
    print(f"{total} coplanar clashes in {len(names)} pieces", file=sys.stderr)
    return 1 if total else 0


if __name__ == "__main__":
    sys.exit(main())
