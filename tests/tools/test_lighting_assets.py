import json
import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "checker_maps"))
sys.path.insert(0, str(ROOT / "tools" / "interior_kit"))
import build_interior
import checker_maps

SPECS = ROOT / "specs" / "lighting"
LIGHTING = ROOT / "assets" / "gen" / "lighting"


class LightingAssetsTest(unittest.TestCase):
    def test_committed_gobos_are_what_the_spec_draws(self):
        spec = json.loads((SPECS / "gobos.json").read_text(encoding="utf-8"))
        expected = {}
        for name, entry in spec["gobos"].items():
            frames = checker_maps.gobo_frames(entry)
            if len(frames) == 1:
                expected[f"{name}.png"] = frames[0]
            else:
                expected.update({f"{name}_{index}.png": frame for index, frame in enumerate(frames)})
        gobos = ROOT / spec["out"]
        self.assertEqual(sorted(path.name for path in gobos.glob("*.png")), sorted(expected))
        for name, pixels in expected.items():
            with Image.open(gobos / name) as image:
                self.assertEqual(image.mode, "RGB", name)
                self.assertTrue(np.array_equal(np.asarray(image), pixels), name)

    def test_every_gobo_frame_shares_one_power_of_two_size(self):
        spec = json.loads((SPECS / "gobos.json").read_text(encoding="utf-8"))
        sizes = {int(entry["size"]) for entry in spec["gobos"].values()}
        self.assertEqual(len(sizes), 1)
        size = sizes.pop()
        self.assertTrue(16 <= size <= 256 and size & (size - 1) == 0, size)

    def test_committed_room_is_what_interior_kit_builds(self):
        with tempfile.TemporaryDirectory() as folder:
            out = Path(folder)
            build_interior.build(SPECS, out)
            built = sorted(path.relative_to(out).as_posix() for path in out.rglob("*") if path.is_file())
            committed = sorted(path.relative_to(LIGHTING).as_posix() for path in LIGHTING.rglob("*") if path.is_file() and path.parent.name != "gobos")
            self.assertEqual(built, committed)
            for name in built:
                self.assertEqual((out / name).read_bytes(), (LIGHTING / name).read_bytes(), name)


if __name__ == "__main__":
    unittest.main()
