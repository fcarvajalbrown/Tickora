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


if __name__ == "__main__":
    unittest.main()
