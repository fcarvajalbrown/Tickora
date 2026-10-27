import json
import struct
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "canonical_glb"))
import canonical_glb


def make_glb(indices):
    binary = struct.pack(f"<{len(indices)}H", *indices)
    binary += b"\0" * (-len(binary) % 4)
    document = {
        "asset": {"version": "2.0"},
        "buffers": [{"byteLength": len(binary)}],
        "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": len(indices) * 2}],
        "accessors": [{"bufferView": 0, "componentType": 5123, "count": len(indices), "type": "SCALAR"}],
        "meshes": [{"primitives": [{"attributes": {}, "indices": 0}]}],
    }
    text = json.dumps(document).encode()
    text += b" " * (-len(text) % 4)
    body = struct.pack("<II", len(text), 0x4E4F534A) + text + struct.pack("<II", len(binary), 0x004E4942) + binary
    return struct.pack("<III", 0x46546C67, 2, 12 + len(body)) + body


def indices_of(glb):
    document, start = canonical_glb.read_glb(glb)
    count = document["accessors"][0]["count"]
    return list(struct.unpack_from(f"<{count}H", glb, start))


class CanonicalGlbTest(unittest.TestCase):
    def test_same_triangles_in_any_order_give_identical_bytes(self):
        first = make_glb([0, 1, 2, 2, 3, 0, 4, 5, 6])
        shuffled = make_glb([5, 6, 4, 3, 0, 2, 1, 2, 0])
        self.assertEqual(canonical_glb.canonicalise(first), canonical_glb.canonicalise(shuffled))

    def test_rotation_keeps_winding(self):
        self.assertEqual(indices_of(canonical_glb.canonicalise(make_glb([2, 0, 1]))), [0, 1, 2])
        self.assertEqual(indices_of(canonical_glb.canonicalise(make_glb([2, 1, 0]))), [0, 2, 1])


if __name__ == "__main__":
    unittest.main()
