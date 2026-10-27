import json
import math
import struct
from pathlib import Path

import numpy as np

PALETTE = {
    "bark": (88, 66, 52),
    "foliage": (44, 88, 52),
    "wood": (150, 115, 80),
    "shingle": (98, 72, 54),
    "calamina": (150, 155, 160),
    "clay": (168, 108, 78),
    "stone": (120, 118, 112),
    "dirt": (128, 100, 70),
    "grass": (92, 120, 62),
    "dark": (40, 30, 24),
    "metal": (62, 64, 68),
    "straw": (200, 175, 100),
    "cloth": (150, 70, 60),
    "wall": (160, 125, 88),
}
UV_TILE_METRES = 2.0
MIN_TRIANGLE_AREA = 1e-6


def rotation_x(angle):
    c, s = math.cos(angle), math.sin(angle)
    return np.array([[1, 0, 0], [0, c, -s], [0, s, c]])


def rotation_y(angle):
    c, s = math.cos(angle), math.sin(angle)
    return np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]])


def rotation_z(angle):
    c, s = math.cos(angle), math.sin(angle)
    return np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]])


def face_normals(triangles):
    normals = np.cross(triangles[:, 1] - triangles[:, 0], triangles[:, 2] - triangles[:, 0])
    lengths = np.linalg.norm(normals, axis=1, keepdims=True)
    lengths[lengths == 0] = 1.0
    return normals / lengths


def orient_away(triangles, anchors):
    normals = face_normals(triangles)
    outward = np.einsum("ij,ij->i", normals, triangles.mean(axis=1) - anchors) >= 0
    flipped = triangles.copy()
    flipped[~outward] = flipped[~outward][:, ::-1]
    return flipped


def box_triangles(size, centre=(0, 0, 0)):
    half = np.array(size, dtype=float) / 2
    corners = np.array([[sx, sy, sz] for sx in (-1, 1) for sy in (-1, 1) for sz in (-1, 1)]) * half + np.array(centre)
    quads = [(0, 1, 3, 2), (4, 6, 7, 5), (0, 4, 5, 1), (2, 3, 7, 6), (0, 2, 6, 4), (1, 5, 7, 3)]
    triangles = []
    for a, b, c, d in quads:
        triangles += [(corners[a], corners[b], corners[c]), (corners[a], corners[c], corners[d])]
    triangles = np.array(triangles)
    return orient_away(triangles, np.tile(np.array(centre, dtype=float), (len(triangles), 1)))


def frustum_triangles(radius_bottom, radius_top, height, z0=0.0, sections=8, capped=True):
    angles = np.linspace(0, 2 * math.pi, sections, endpoint=False)
    bottom = np.stack([radius_bottom * np.cos(angles), radius_bottom * np.sin(angles), np.full(sections, z0)], axis=1)
    top = np.stack([radius_top * np.cos(angles), radius_top * np.sin(angles), np.full(sections, z0 + height)], axis=1)
    triangles = []
    anchors = []
    for i in range(sections):
        j = (i + 1) % sections
        triangles += [(bottom[i], bottom[j], top[j]), (bottom[i], top[j], top[i])]
        anchors += [(0, 0, z0 + height / 2)] * 2
        if capped:
            triangles.append((np.array([0, 0, z0]), bottom[j], bottom[i]))
            anchors.append((0, 0, z0 + height / 2))
            if radius_top > 0:
                triangles.append((np.array([0, 0, z0 + height]), top[i], top[j]))
                anchors.append((0, 0, z0 + height / 2))
    return orient_away(np.array(triangles), np.array(anchors, dtype=float))


def signed_area(points):
    return 0.5 * sum(points[i][0] * points[(i + 1) % len(points)][1] - points[(i + 1) % len(points)][0] * points[i][1] for i in range(len(points)))


def inside_triangle(point, a, b, c):
    def sign(p, q, r):
        return (p[0] - r[0]) * (q[1] - r[1]) - (q[0] - r[0]) * (p[1] - r[1])

    d1, d2, d3 = sign(point, a, b), sign(point, b, c), sign(point, c, a)
    negative = d1 < 0 or d2 < 0 or d3 < 0
    positive = d1 > 0 or d2 > 0 or d3 > 0
    return not (negative and positive)


def triangulate(points):
    remaining = list(range(len(points)))
    result = []
    guard = 0
    while len(remaining) > 3 and guard < 10000:
        guard += 1
        for k in range(len(remaining)):
            i0, i1, i2 = remaining[k - 1], remaining[k], remaining[(k + 1) % len(remaining)]
            a, b, c = points[i0], points[i1], points[i2]
            if (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) <= 1e-12:
                continue
            if any(inside_triangle(points[j], a, b, c) for j in remaining if j not in (i0, i1, i2)):
                continue
            result.append((i0, i1, i2))
            remaining.pop(k)
            break
        else:
            break
    if len(remaining) == 3:
        result.append(tuple(remaining))
    return result


def clean_profile(points):
    cleaned = []
    for point in points:
        if not cleaned or math.hypot(point[0] - cleaned[-1][0], point[1] - cleaned[-1][1]) > 1e-9:
            cleaned.append(tuple(point))
    if len(cleaned) > 1 and math.hypot(cleaned[0][0] - cleaned[-1][0], cleaned[0][1] - cleaned[-1][1]) < 1e-9:
        cleaned.pop()
    return cleaned


def extrude_triangles(profile, length):
    points = clean_profile(profile)
    if signed_area(points) < 0:
        points.reverse()
    triangles = []
    for i0, i1, i2 in triangulate(points):
        a, b, c = points[i0], points[i1], points[i2]
        triangles.append(((*a, length), (*b, length), (*c, length)))
        triangles.append(((*a, 0.0), (*c, 0.0), (*b, 0.0)))
    for i in range(len(points)):
        j = (i + 1) % len(points)
        a0, b0 = (*points[i], 0.0), (*points[j], 0.0)
        a1, b1 = (*points[i], length), (*points[j], length)
        triangles += [(a0, b0, b1), (a0, b1, a1)]
    return np.array(triangles, dtype=float)


MAPPINGS = {
    "x": np.array([[0, 0, 1], [1, 0, 0], [0, 1, 0]], dtype=float),
    "yp": np.array([[-1, 0, 0], [0, 0, 1], [0, 1, 0]], dtype=float),
    "z": np.eye(3),
    "y": np.array([[1, 0, 0], [0, 0, -1], [0, 1, 0]], dtype=float),
}


def euler_matrix(rotation_degrees):
    rx, ry, rz = (math.radians(v) for v in rotation_degrees)
    return rotation_z(rz) @ rotation_y(ry) @ rotation_x(rx)


def tube_triangles(path, radii, sections=5, capped=True, cap_start=True):
    path = np.array(path, dtype=float)
    count = len(path)
    angles = np.linspace(0, 2 * math.pi, sections, endpoint=False)
    rings = []
    for index in range(count):
        ahead = path[min(index + 1, count - 1)] - path[max(index - 1, 0)]
        axis = ahead / (np.linalg.norm(ahead) or 1.0)
        helper = np.array([0.0, 0.0, 1.0]) if abs(axis[2]) < 0.9 else np.array([1.0, 0.0, 0.0])
        side = np.cross(axis, helper)
        side /= np.linalg.norm(side)
        lift = np.cross(axis, side)
        rings.append(np.array([path[index] + radii[index] * (math.cos(a) * side + math.sin(a) * lift) for a in angles]))
    triangles = []
    anchors = []
    for index in range(count - 1):
        for i in range(sections):
            j = (i + 1) % sections
            triangles += [(rings[index][i], rings[index][j], rings[index + 1][j]), (rings[index][i], rings[index + 1][j], rings[index + 1][i])]
            anchors += [(path[index] + path[index + 1]) / 2] * 2
    if capped:
        for end, ring in ((0, rings[0]), (count - 1, rings[-1])):
            if end == 0 and not cap_start:
                continue
            for i in range(sections):
                j = (i + 1) % sections
                triangles.append((path[end], ring[i], ring[j]))
                anchors.append(path[end] - (path[min(end + 1, count - 1)] - path[max(end - 1, 0)]) * 0.5)
    return orient_away(np.array(triangles), np.array(anchors, dtype=float))


class Mesh:
    def __init__(self):
        self.parts = {}

    def add(self, material, triangles):
        triangles = np.asarray(triangles, dtype=float)
        if len(triangles):
            areas = np.linalg.norm(np.cross(triangles[:, 1] - triangles[:, 0], triangles[:, 2] - triangles[:, 0]), axis=1)
            triangles = triangles[areas > MIN_TRIANGLE_AREA]
        if len(triangles):
            self.parts.setdefault(material, []).append(triangles)
        return self

    def box(self, material, size, centre=(0, 0, 0)):
        return self.add(material, box_triangles(size, centre))

    def frustum(self, material, radius_bottom, radius_top, height, z0=0.0, sections=8, capped=True):
        return self.add(material, frustum_triangles(radius_bottom, radius_top, height, z0, sections, capped))

    def extrude(self, material, profile, length, axis="z", at=(0, 0, 0)):
        triangles = extrude_triangles(profile, length) @ MAPPINGS[axis].T
        return self.add(material, triangles + np.array(at, dtype=float))

    def tube(self, material, path, radii, sections=5, capped=True, cap_start=True):
        return self.add(material, tube_triangles(path, radii, sections, capped, cap_start))

    def include(self, other, matrix=None, offset=(0, 0, 0), scale=1.0):
        for material, chunks in other.parts.items():
            for chunk in chunks:
                moved = chunk * scale
                if matrix is not None:
                    moved = moved @ matrix.T
                self.add(material, moved + np.array(offset, dtype=float))
        return self

    def merged(self):
        return {material: np.concatenate(chunks) for material, chunks in self.parts.items()}

    def triangle_count(self):
        return sum(len(chunk) for chunks in self.parts.values() for chunk in chunks)

    def bounds(self):
        stacked = np.concatenate([c.reshape(-1, 3) for chunks in self.parts.values() for c in chunks])
        return stacked.min(axis=0), stacked.max(axis=0)


def box_uv(triangles):
    normals = face_normals(triangles)
    axis = np.abs(normals).argmax(axis=1)
    uv = np.zeros((len(triangles), 3, 2))
    for index, dominant in enumerate(axis):
        pair = [(1, 2), (0, 2), (0, 1)][dominant]
        uv[index] = triangles[index][:, pair] / UV_TILE_METRES
    return uv


def write_glb(mesh, path):
    merged = mesh.merged()
    binary = bytearray()
    views, accessors, primitives, materials = [], [], [], []

    def push(array, target, component, kind, minmax=False):
        while len(binary) % 4:
            binary.append(0)
        offset = len(binary)
        binary.extend(array.tobytes())
        views.append({"buffer": 0, "byteOffset": offset, "byteLength": array.nbytes, "target": target})
        accessor = {"bufferView": len(views) - 1, "componentType": component, "count": len(array), "type": kind}
        if minmax:
            accessor["min"] = array.min(axis=0).tolist()
            accessor["max"] = array.max(axis=0).tolist()
        accessors.append(accessor)
        return len(accessors) - 1

    for index, (material, triangles) in enumerate(sorted(merged.items())):
        yup = np.stack([triangles[..., 0], triangles[..., 2], -triangles[..., 1]], axis=-1)
        normals = np.repeat(face_normals(yup)[:, None, :], 3, axis=1).reshape(-1, 3).astype(np.float32)
        uv = box_uv(triangles).reshape(-1, 2).astype(np.float32)
        positions = yup.reshape(-1, 3).astype(np.float32)
        indices = np.arange(len(positions), dtype=np.uint32)
        attributes = {
            "POSITION": push(positions, 34962, 5126, "VEC3", True),
            "NORMAL": push(normals, 34962, 5126, "VEC3"),
            "TEXCOORD_0": push(uv, 34962, 5126, "VEC2"),
        }
        indices_accessor = push(indices, 34963, 5125, "SCALAR")
        colour = [c / 255 for c in PALETTE.get(material, (180, 180, 180))] + [1.0]
        materials.append({"name": material, "pbrMetallicRoughness": {"baseColorFactor": colour, "metallicFactor": 0.0, "roughnessFactor": 0.9}})
        primitives.append({"attributes": attributes, "indices": indices_accessor, "material": index})

    document = {
        "asset": {"version": "2.0", "generator": "meshkit"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0, "name": Path(path).stem}],
        "meshes": [{"name": Path(path).stem, "primitives": primitives}],
        "materials": materials,
        "buffers": [{"byteLength": len(binary)}],
        "bufferViews": views,
        "accessors": accessors,
    }
    body = json.dumps(document, separators=(",", ":")).encode()
    body += b" " * (-len(body) % 4)
    while len(binary) % 4:
        binary.append(0)
    total = 12 + 8 + len(body) + 8 + len(binary)
    with open(path, "wb") as handle:
        handle.write(struct.pack("<4sII", b"glTF", 2, total))
        handle.write(struct.pack("<I4s", len(body), b"JSON"))
        handle.write(body)
        handle.write(struct.pack("<I4s", len(binary), b"BIN\x00"))
        handle.write(bytes(binary))
