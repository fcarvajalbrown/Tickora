import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np
import trimesh

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "check_mesh"))
import check_mesh


def write_glb(mesh, folder, name):
    path = Path(folder) / name
    path.write_bytes(trimesh.Scene(mesh).export(file_type="glb"))
    return path


def grid_box(low, high):
    box = trimesh.creation.box(bounds=[low, high])
    box.vertices = np.round(box.vertices, 6)
    return box


class SizeAwareCheckTest(unittest.TestCase):
    def test_small_prop_passes_with_its_own_grid(self):
        with tempfile.TemporaryDirectory() as folder:
            pad = grid_box([0.0, 0.0, 0.0], [0.07, 0.11, 0.025])
            path = write_glb(pad, folder, "pad.glb")
            report = check_mesh.check(path, grid=0.005)
            self.assertAlmostEqual(report["volume_m3"], 0.0001925, delta=1e-6)
            self.assertTrue(report["clean"])

    def test_two_decilitre_prop_passes_on_the_default_grid(self):
        with tempfile.TemporaryDirectory() as folder:
            stamp = grid_box([0.0, 0.0, 0.0], [0.1, 0.05, 0.04])
            report = check_mesh.check(write_glb(stamp, folder, "stamp.glb"))
            self.assertAlmostEqual(report["volume_m3"], 0.0002, places=6)
            self.assertTrue(report["clean"])

    def test_degenerate_mesh_still_fails(self):
        with tempfile.TemporaryDirectory() as folder:
            box = grid_box([0.0, 0.0, 0.0], [0.1, 0.05, 0.04])
            flat = trimesh.Trimesh(vertices=np.vstack([box.vertices, [[0.0, 0.0, 0.0], [0.1, 0.0, 0.0], [0.2, 0.0, 0.0]]]), faces=np.vstack([box.faces, [[8, 9, 10]]]), process=False)
            report = check_mesh.check(write_glb(flat, folder, "flat.glb"), grid=0.005)
            self.assertGreater(report["degenerate_triangles"] + report["open_edges"], 0)
            self.assertFalse(report["clean"])

    def test_default_grid_is_one_centimetre(self):
        with tempfile.TemporaryDirectory() as folder:
            half = grid_box([0.0, 0.0, 0.0], [0.105, 0.05, 0.04])
            path = write_glb(half, folder, "half.glb")
            default = check_mesh.check(path)
            self.assertEqual(default["grid_m"], 0.01)
            self.assertGreater(default["vertices_off_grid"], 0)
            self.assertFalse(default["clean"])
            self.assertTrue(check_mesh.check(path, grid=0.005)["clean"])


if __name__ == "__main__":
    unittest.main()
