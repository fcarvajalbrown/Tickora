import math

import numpy as np

import meshkit as mk

SIDING_PITCH = 0.14
SIDING_LAP = 0.02


def local_mesh():
    return mk.Mesh()


def make_box(p):
    return mk.Mesh().box(p["m"], p["size"])


def make_cyl(p):
    radius = p["r"]
    return mk.Mesh().frustum(p["m"], radius, p.get("r_top", radius), p["h"], 0.0, p.get("sections", 8))


def make_extrude(p):
    return mk.Mesh().extrude(p["m"], p["profile"], p["length"], p.get("axis", "z"))


def make_tube(p):
    return mk.Mesh().tube(p["m"], p["path"], p["radii"], p.get("sections", 5))


def wall_profile(z0, z1, thickness, siding):
    half = thickness / 2
    points = [(-half, z0), (half, z0), (half, z1), (-half, z1)]
    if siding != "outside":
        return points
    steps = int(z1 / SIDING_PITCH)
    bounds = sorted({z0, z1} | {k * SIDING_PITCH for k in range(1, steps + 1) if z0 < k * SIDING_PITCH < z1})
    for index in range(len(bounds) - 1, 0, -1):
        top, bottom = bounds[index], bounds[index - 1]
        points += [(-half, top), (-half - SIDING_LAP, bottom), (-half, bottom)]
    return points


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


def make_wall(p):
    mesh = mk.Mesh()
    thickness = p["thickness"]
    extend = thickness / 2 if p.get("extend") else 0.0
    for x0, x1, z0, z1 in wall_segments(p["length"], p["height"], p.get("openings", []), extend):
        profile = wall_profile(z0, z1, thickness, p.get("siding", "none"))
        mesh.extrude(p["m"], profile, x1 - x0, "x", (x0, 0.0, 0.0))
    return mesh


def shingle_profile(slope, thickness, exposure, lap):
    rows = max(int(round(slope / exposure)), 1)
    stops = [slope * i / rows for i in range(rows + 1)]
    points = [(0.0, 0.0), (slope, 0.0), (slope, thickness)]
    for k in range(rows - 1, -1, -1):
        points += [(stops[k + 1], thickness), (stops[k], thickness + lap)]
    return points


def corrugated_profile(width, thickness, rib, depth):
    count = max(int(round(width / rib)), 1)
    left = -width / 2
    points = [(left, 0.0), (-left, 0.0), (-left, thickness)]
    for i in range(count, -1, -1):
        points.append((left + width * i / count, thickness + (0.0 if i % 2 else depth)))
    return points


def make_roof(p):
    pitch = math.radians(p["pitch"])
    slope = p["run"] / math.cos(pitch)
    mesh = mk.Mesh()
    if p.get("style") == "corrugated":
        profile = corrugated_profile(p["length"], p["thickness"], p["rib"], p["rib_depth"])
        flat = mk.Mesh().extrude(p["m"], profile, slope, "yp")
    else:
        profile = shingle_profile(slope, p["thickness"], p["exposure"], p["lap"])
        flat = mk.Mesh().extrude(p["m"], profile, p["length"], "x", (-p["length"] / 2, 0.0, 0.0))
    return mesh.include(flat, mk.rotation_x(pitch))


BUILDERS = {
    "box": make_box,
    "cyl": make_cyl,
    "extrude": make_extrude,
    "tube": make_tube,
    "wall": make_wall,
    "roof_slope": make_roof,
}


def build_part(p):
    if p["t"] == "repeat":
        combined = mk.Mesh()
        for index in range(p["count"]):
            step = [component * index for component in p["step"]]
            combined.include(build_part(p["part"]), offset=step)
        return combined
    mesh = BUILDERS[p["t"]](p)
    rotation = list(p.get("rot", [0, 0, 0]))
    rotation[2] += p.get("yaw", 0)
    matrix = mk.euler_matrix(rotation) @ np.diag([float(value) for value in p.get("scale", [1, 1, 1])])
    placed = mk.Mesh()
    return placed.include(mesh, matrix, p.get("at", [0, 0, 0]))


def build_piece(spec):
    groups = {}
    for part in spec["parts"]:
        groups.setdefault(part.get("group", "main"), mk.Mesh()).include(build_part(part))
    return groups
