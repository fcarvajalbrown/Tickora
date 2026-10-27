import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np
import trimesh
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "uv_check"))

import uv_check
from auto_uv import fill_collapsed_uv


def material():
    return trimesh.visual.material.PBRMaterial(name="test", baseColorFactor=[200, 200, 200, 255])


def export(meshes, folder, name):
    scene = trimesh.Scene()
    for index, mesh in enumerate(meshes):
        scene.add_geometry(mesh, node_name=f"part_{index}", geom_name=f"part_{index}")
    path = Path(folder) / f"{name}.glb"
    scene.export(path)
    return path


def flat_uv_box():
    box = trimesh.creation.box(extents=(0.4, 0.3, 0.2))
    uv = np.column_stack((box.vertices[:, 0] + 0.5, box.vertices[:, 1] + 0.5))
    box.visual = trimesh.visual.TextureVisuals(uv=uv, material=material())
    return box


class UvCheckTest(unittest.TestCase):
    def test_front_projected_box_fails_on_its_sides(self):
        with tempfile.TemporaryDirectory() as folder:
            report = uv_check.check(export([flat_uv_box()], folder, "flat"))
        self.assertFalse(report["clean"])
        self.assertEqual(report["zero_area"], 8)

    def test_fill_collapsed_uv_fixes_the_sides_and_keeps_the_shape(self):
        box = flat_uv_box()
        fixed = fill_collapsed_uv(box, material())
        with tempfile.TemporaryDirectory() as folder:
            report = uv_check.check(export([fixed], folder, "fixed"))
        self.assertTrue(report["clean"])
        self.assertTrue(np.allclose(np.sort(box.triangles.reshape(12, -1), axis=0), np.sort(fixed.triangles.reshape(12, -1), axis=0)))

    def test_fill_collapsed_uv_keeps_good_uvs(self):
        box = flat_uv_box()
        front = np.nonzero(box.face_normals[:, 2] > 0.5)[0]
        before = box.visual.uv[box.faces[front]]
        fixed = fill_collapsed_uv(box, material())
        self.assertTrue(np.allclose(before, fixed.visual.uv[fixed.faces[front]]))

    def test_textured_primitive_without_uvs_needs_them(self):
        bare = trimesh.creation.box(extents=(1.0, 1.0, 1.0))
        textured = trimesh.visual.material.PBRMaterial(name="textured", baseColorTexture=Image.new("RGB", (2, 2)))
        bare.visual = trimesh.visual.TextureVisuals(material=textured)
        report = uv_check.check_primitive("bare", bare)
        self.assertTrue(report["needs_uv"])
        self.assertFalse(report["has_uv"])
        with tempfile.TemporaryDirectory() as folder:
            fixed_report = uv_check.check(export([fill_collapsed_uv(bare, textured)], folder, "bare_fixed"))
        self.assertTrue(fixed_report["clean"])

    def test_untextured_vertex_colour_primitive_needs_no_uvs(self):
        tab = trimesh.creation.box(extents=(0.05, 0.05, 0.001))
        tab.visual = trimesh.visual.ColorVisuals(tab, vertex_colors=[230, 200, 40, 255])
        with tempfile.TemporaryDirectory() as folder:
            report = uv_check.check(export([fill_collapsed_uv(flat_uv_box(), material()), tab], folder, "tabbed"))
        self.assertEqual(report["missing_uv"], 0)
        self.assertTrue(report["clean"])

    def test_tiling_uvs_pass_unless_square_is_asked(self):
        box = fill_collapsed_uv(trimesh.creation.box(extents=(3.0, 2.0, 1.0)), material(), 2.0)
        with tempfile.TemporaryDirectory() as folder:
            path = export([box], folder, "tiled")
            self.assertTrue(uv_check.check(path)["clean"])
            self.assertFalse(uv_check.check(path, square=True)["clean"])

    def test_degenerate_triangles_are_left_to_check_mesh(self):
        sliver = trimesh.Trimesh(vertices=[[0, 0, 0], [1, 0, 0], [0.5, 1e-9, 0]], faces=[[0, 1, 2]], process=False)
        sliver.visual = trimesh.visual.TextureVisuals(uv=np.zeros((3, 2)), material=material())
        with tempfile.TemporaryDirectory() as folder:
            report = uv_check.check(export([flat_uv_box(), sliver], folder, "sliver"))
        self.assertEqual(report["zero_area"], 8)


if __name__ == "__main__":
    unittest.main()
