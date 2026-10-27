import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools" / "interior_kit"))
import build_interior


class InteriorKitTest(unittest.TestCase):
    def test_dents_expand_seeded_into_tilted_patch_pairs(self):
        dents = {"t": "dents", "m": "steel", "seed": 3, "count": [2, 3], "region": {"x": [0.0, 5.0], "z": [0.0, 3.0]}, "width": [0.5, 0.9], "height": [0.4, 0.8], "depth": [0.03, 0.06], "sheet_y": 0.04, "thickness": 0.01}
        first = build_interior.expanded({"parts": [dents]})["parts"]
        self.assertEqual(first, build_interior.expanded({"parts": [dents]})["parts"])
        self.assertIn(len(first), (4, 6))
        for left, right in zip(first[0::2], first[1::2]):
            self.assertEqual(left["rot"][2], -right["rot"][2])
            self.assertLess(left["at"][1], 0.04)
            self.assertGreaterEqual(left["at"][0] - left["size"][0] / 2.0, -0.01)
            self.assertLessEqual(right["at"][0] + right["size"][0] / 2.0, 5.01)

    def test_facing_yaw_turns_default_front_to_target(self):
        self.assertAlmostEqual(build_interior.facing_yaw([0, 0], [0, -1], -90.0), 0.0)
        self.assertAlmostEqual(build_interior.facing_yaw([0, 0], [1, 0], -90.0), 90.0)

    def test_csv_row_uses_meshkit_unreal_convention(self):
        row = {"piece": "desk", "at": [1.0, 2.0, 0.5], "yaw": 30.0, "folder": "Booth", "instance": False}
        self.assertEqual(build_interior.csv_row(row), ["desk", 100.0, -200.0, 50.0, -30.0, 1.0, 1.0, "Booth", 0])

    def test_anchor_rows_are_prefixed_and_sorted(self):
        rows = build_interior.anchor_rows({"b": {"at": [0, 0, 0], "yaw": 5}, "a": {"at": [0, 0, 0], "face": [0, -2]}}, -90.0)
        self.assertEqual([row["piece"] for row in rows], ["anchor_a", "anchor_b"])
        self.assertAlmostEqual(rows[0]["yaw"], 0.0)


if __name__ == "__main__":
    unittest.main()
