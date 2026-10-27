import argparse
import json
import struct
import sys
from pathlib import Path

INDEX_FORMATS = {5121: "B", 5123: "H", 5125: "I"}


def read_glb(data):
    json_length = struct.unpack_from("<I", data, 12)[0]
    document = json.loads(data[20:20 + json_length])
    binary_start = 20 + json_length + 8
    return document, binary_start


def canonical_triangles(indices):
    triangles = []
    for start in range(0, len(indices) - len(indices) % 3, 3):
        a, b, c = indices[start:start + 3]
        rotations = [(a, b, c), (b, c, a), (c, a, b)]
        triangles.append(min(rotations))
    triangles.sort()
    return [index for triangle in triangles for index in triangle]


def canonicalise(data):
    data = bytearray(data)
    document, binary_start = read_glb(data)
    seen = set()
    for mesh in document.get("meshes", []):
        for primitive in mesh["primitives"]:
            accessor_index = primitive.get("indices")
            if accessor_index is None or accessor_index in seen or primitive.get("mode", 4) != 4:
                continue
            seen.add(accessor_index)
            accessor = document["accessors"][accessor_index]
            view = document["bufferViews"][accessor["bufferView"]]
            code = INDEX_FORMATS[accessor["componentType"]]
            offset = binary_start + view.get("byteOffset", 0) + accessor.get("byteOffset", 0)
            fmt = f"<{accessor['count']}{code}"
            indices = list(struct.unpack_from(fmt, data, offset))
            struct.pack_into(fmt, data, offset, *canonical_triangles(indices))
    return bytes(data)


def main(argv=None):
    parser = argparse.ArgumentParser(description="Rewrite a GLB's triangle index buffers in a canonical order (each triangle rotated to its lowest index, winding kept, triangles sorted) so identical geometry gives identical bytes.")
    parser.add_argument("glb", nargs="+", type=Path)
    args = parser.parse_args(argv)
    for path in args.glb:
        path.write_bytes(canonicalise(path.read_bytes()))
    return 0


if __name__ == "__main__":
    sys.exit(main())
