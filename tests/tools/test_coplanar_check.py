import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "coplanar_check"))
import coplanar_check



class CoplanarCheckTest(unittest.TestCase):
    def test_two_boxes_on_one_plane_clash(self):
        parts = [
            {"t": "box", "size": [1.0, 1.0, 1.0], "at": [0.0, 0.0, 0.5]},
            {"t": "box", "size": [0.2, 0.2, 2.0], "at": [0.4, 0.4, 1.0]},
        ]
        self.assertEqual(len(coplanar_check.clashes(parts)), 2)

    def test_boxes_a_centimetre_apart_pass(self):
        parts = [
            {"t": "box", "size": [1.0, 1.0, 1.0], "at": [0.0, 0.0, 0.5]},
            {"t": "box", "size": [0.2, 0.2, 2.0], "at": [0.41, 0.41, 1.0]},
        ]
        self.assertEqual(coplanar_check.clashes(parts), [])

    def test_back_to_back_faces_pass(self):
        parts = [
            {"t": "box", "size": [1.0, 1.0, 1.0], "at": [0.0, 0.0, 0.5]},
            {"t": "box", "size": [1.0, 1.0, 1.0], "at": [0.0, 0.0, 1.5]},
        ]
        self.assertEqual(coplanar_check.clashes(parts), [])

    def test_bottoms_resting_on_the_ground_pass(self):
        parts = [
            {"t": "box", "size": [1.0, 1.0, 1.0], "at": [0.0, 0.0, 0.5]},
            {"t": "box", "size": [0.2, 0.2, 0.3], "at": [0.0, 0.0, 0.15]},
        ]
        self.assertEqual(coplanar_check.clashes(parts), [])

    def test_a_frame_flush_with_a_wall_reveal_clashes(self):
        parts = [
            {"t": "wall", "length": 3.0, "height": 2.0, "thickness": 0.1,
             "openings": [{"at": 1.5, "width": 1.0, "sill": 0, "head": 1.5}], "at": [0, 0, 0]},
            {"t": "box", "size": [0.06, 0.1, 1.5], "at": [0.97, 0, 0.75]},
        ]
        self.assertTrue(coplanar_check.clashes(parts))


if __name__ == "__main__":
    unittest.main()
