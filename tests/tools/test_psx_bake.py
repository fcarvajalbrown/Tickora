import json
import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools" / "cc0_textures"))

import psx_bake


class PsxBakeTest(unittest.TestCase):
    def test_the_table_carries_opacity_only_where_set(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "src" / "Sheet").mkdir(parents=True)
            Image.fromarray(np.full((8, 8, 3), 160, dtype=np.uint8)).save(root / "src" / "Sheet" / "Sheet_Color.png")
            spec = {"source_root": "src", "out": "out", "size": 4, "materials": {
                "clear": {"asset": "Sheet", "tile_m": 1.0, "opacity": 0.5},
                "solid": {"asset": "Sheet", "tile_m": 2.0}}}
            (root / "spec.json").write_text(json.dumps(spec), encoding="utf-8")
            psx_bake.main(["--spec", str(root / "spec.json"), "--root", str(root)])
            table = json.loads((root / "out" / "materials.json").read_text(encoding="utf-8"))
            self.assertEqual(table["clear"]["opacity"], 0.5)
            self.assertNotIn("opacity", table["solid"])

    def test_generated_looks_are_listed_but_not_baked(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            spec = {"source_root": "src", "out": "out", "size": 4, "materials": {"painted": {"generated": True, "tile_m": 1.0}}}
            (root / "spec.json").write_text(json.dumps(spec), encoding="utf-8")
            psx_bake.main(["--spec", str(root / "spec.json"), "--root", str(root)])
            table = json.loads((root / "out" / "materials.json").read_text(encoding="utf-8"))
            self.assertEqual(table["painted"]["tile_m"], 1.0)
            self.assertFalse((root / "out" / "painted.png").exists())


if __name__ == "__main__":
    unittest.main()
