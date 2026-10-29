import json
import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools" / "checker_maps"))

import checker_maps


class CheckerMapsTest(unittest.TestCase):
    def test_checker_alternates_cells_from_the_top_left(self):
        image = checker_maps.checker(4, 2, [[10, 20, 30], [200, 100, 50]])
        self.assertEqual(image.shape, (4, 4, 3))
        self.assertEqual(image[0, 0].tolist(), [10, 20, 30])
        self.assertEqual(image[1, 1].tolist(), [10, 20, 30])
        self.assertEqual(image[0, 2].tolist(), [200, 100, 50])
        self.assertEqual(image[2, 0].tolist(), [200, 100, 50])
        self.assertEqual(image[3, 3].tolist(), [10, 20, 30])

    def test_size_must_split_into_whole_cells(self):
        with self.assertRaises(SystemExit):
            checker_maps.checker(5, 2, [[0, 0, 0], [255, 255, 255]])

    def test_writes_an_ambientcg_style_colour_map(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            spec = {"out": "src", "patterns": {"Checker": {"size": 8, "cells": 2, "colours": [[255, 0, 0], [0, 0, 255]]}}}
            (root / "spec.json").write_text(json.dumps(spec), encoding="utf-8")
            checker_maps.main(["--spec", str(root / "spec.json"), "--root", str(root)])
            with Image.open(root / "src" / "Checker" / "Checker_Color.png") as image:
                self.assertEqual(image.mode, "RGB")
                pixels = np.asarray(image)
            self.assertEqual(pixels[0, 0].tolist(), [255, 0, 0])
            self.assertEqual(pixels[0, 7].tolist(), [0, 0, 255])

    def test_bar_mask_darkens_the_border_and_one_bar_between_panes(self):
        mask = checker_maps.bar_mask(16, 2, 2, 1)
        self.assertEqual(np.flatnonzero(mask).tolist(), [0, 7, 8, 15])
        self.assertEqual(np.flatnonzero(checker_maps.bar_mask(16, 1, 2, 0)).tolist(), [])

    def test_bar_mask_rejects_borders_that_fill_the_gobo(self):
        with self.assertRaises(SystemExit):
            checker_maps.bar_mask(16, 2, 2, 8)

    def test_grid_crosses_column_and_row_bars(self):
        image = checker_maps.grid(16, [2, 1], 2, 1, [255, 255, 255], [0, 0, 0])
        self.assertEqual(image.shape, (16, 16, 3))
        self.assertEqual(image[0, 4].tolist(), [0, 0, 0])
        self.assertEqual(image[4, 7].tolist(), [0, 0, 0])
        self.assertEqual(image[4, 4].tolist(), [255, 255, 255])
        self.assertEqual(image[4, 12].tolist(), [255, 255, 255])

    def test_levels_make_one_grey_frame_per_value(self):
        frames = checker_maps.gobo_frames({"kind": "levels", "size": 16, "levels": [255, 90]})
        self.assertEqual(len(frames), 2)
        self.assertTrue((frames[0] == 255).all())
        self.assertTrue((frames[1] == 90).all())

    def test_unknown_gobo_kind_is_an_error(self):
        with self.assertRaises(SystemExit):
            checker_maps.gobo_frames({"kind": "spiral", "size": 16})

    def test_gobos_write_one_file_or_one_per_frame_byte_stable(self):
        spec = {"out": "gobos", "gobos": {
            "bars": {"kind": "grid", "size": 16, "panes": [2, 2], "bar": 2, "border": 1, "light": [255, 255, 255], "dark": [0, 0, 0]},
            "flicker": {"kind": "levels", "size": 16, "levels": [255, 128, 64]},
        }}
        written = []
        for _ in range(2):
            with tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                (root / "spec.json").write_text(json.dumps(spec), encoding="utf-8")
                checker_maps.main(["--spec", str(root / "spec.json"), "--root", str(root)])
                files = sorted(path.name for path in (root / "gobos").iterdir())
                written.append({name: (root / "gobos" / name).read_bytes() for name in files})
        self.assertEqual(sorted(written[0]), ["bars.png", "flicker_0.png", "flicker_1.png", "flicker_2.png"])
        self.assertEqual(written[0], written[1])


if __name__ == "__main__":
    unittest.main()
