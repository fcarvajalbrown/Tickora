import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "shot_baseline"))
import shot_baseline


def write(base: Path, name: str, data: bytes) -> None:
    path = base / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


class ShotBaselineTest(unittest.TestCase):
    def test_compare_sorts_shots_into_same_changed_and_missing(self):
        with tempfile.TemporaryDirectory() as temp:
            before = Path(temp) / "before"
            after = Path(temp) / "after"
            write(before, "scenarios/goldens/a/x.png", b"one")
            write(after, "scenarios/goldens/a/x.png", b"one")
            write(before, "scenarios/goldens/b/x.png", b"two")
            write(after, "scenarios/goldens/b/x.png", b"three")
            write(before, "scenarios/goldens/c/x.png", b"gone")
            write(after, "scenarios/goldens/d/x.png", b"new")
            report = shot_baseline.compare(before, after)
            self.assertEqual(report["same"], ["scenarios/goldens/a/x.png"])
            self.assertEqual(report["changed"], ["scenarios/goldens/b/x.png"])
            self.assertEqual(report["only_before"], ["scenarios/goldens/c/x.png"])
            self.assertEqual(report["only_after"], ["scenarios/goldens/d/x.png"])

    def test_new_shots_fail_only_without_allow_new(self):
        with tempfile.TemporaryDirectory() as temp:
            before = Path(temp) / "before"
            after = Path(temp) / "after"
            write(before, "a.png", b"one")
            write(after, "a.png", b"one")
            write(after, "b.png", b"new")
            self.assertEqual(shot_baseline.main(["compare", str(before), str(after)]), 1)
            self.assertEqual(shot_baseline.main(["compare", str(before), str(after), "--allow-new"]), 0)


if __name__ == "__main__":
    unittest.main()
