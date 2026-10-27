import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools" / "mesh_gate"))

import mesh_gate


class MeshGateTest(unittest.TestCase):
    def test_touching_parts_and_round_parts_pass(self):
        report = {"winding_consistent": True, "open_edges": 0, "degenerate_triangles": 0, "non_manifold_edges": 80, "vertices_off_grid": 28}
        self.assertEqual(mesh_gate.failures(report), [])

    def test_open_edges_and_bad_winding_fail(self):
        report = {"winding_consistent": False, "open_edges": 3, "degenerate_triangles": 0}
        self.assertEqual(mesh_gate.failures(report), ["winding_consistent", "open_edges"])


if __name__ == "__main__":
    unittest.main()
