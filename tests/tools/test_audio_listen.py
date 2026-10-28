import argparse
import json
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "audio_listen"))
import listen

TOLERANCE = 0.11
WIDE_TOLERANCE = {"centroid_hz": 1, "sub": 1, "low": 1, "mid": 1, "presence": 1, "air": 1}
NOT_MEASURES = {"file", "channels", "subtype"}
TICK_COLUMNS = [
    re.compile(r"  tick +\d+(?=  \+)"),
    re.compile(r"  ticks +\d+- +\d+"),
    re.compile(r" \(ticks \d+ to \d+\)"),
]


def rust_hearing(wav):
    command = ["cargo", "run", "-q", "--release", "-p", "tickora-mcp", "--",
               "scenario", "--json", "--listen", "--wav", str(wav), "scenarios/arena.json"]
    out = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=True).stdout
    return json.loads(out)[0]["hearing"]


def differences(python, rust, path=""):
    if isinstance(python, dict):
        found = []
        for key, value in python.items():
            if key not in rust:
                found.append(f"{path}{key}: missing in the port")
            else:
                found += differences(value, rust[key], f"{path}{key}.")
        return found
    if isinstance(python, list):
        if len(python) != len(rust):
            return [f"{path[:-1]}: {len(python)} items against {len(rust)}"]
        return [line for index, (a, b) in enumerate(zip(python, rust)) for line in differences(a, b, f"{path}{index}.")]
    if isinstance(python, (int, float)) and not isinstance(python, bool) and rust is not None:
        tolerance = WIDE_TOLERANCE.get(path[:-1].rsplit(".", 1)[-1], TOLERANCE)
        return [] if abs(python - rust) <= tolerance else [f"{path[:-1]}: {python} against {rust}"]
    return [] if python == rust else [f"{path[:-1]}: {python!r} against {rust!r}"]


@unittest.skipUnless(shutil.which("cargo"), "needs cargo to run the Rust port")
class AudioListenCrossCheckTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.folder = tempfile.TemporaryDirectory()
        wav = Path(cls.folder.name) / "arena.wav"
        cls.rust = rust_hearing(wav)
        options = argparse.Namespace(event_db=10.0, max_events=15, segments=12, loop_search=None, png=None, marks=None)
        cls.python = listen.analyse(wav, options)

    @classmethod
    def tearDownClass(cls):
        cls.folder.cleanup()

    def test_every_measure_matches_listen_py(self):
        python = {key: value for key, value in self.python.items() if key not in NOT_MEASURES}
        self.assertEqual(differences(python, self.rust["analysis"]), [])

    def test_the_reading_is_listen_py_summary_with_ticks_added(self):
        rust = self.rust["reading"].splitlines()[1:]
        for pattern in TICK_COLUMNS:
            rust = [pattern.sub("", line) for line in rust]
        self.assertEqual(rust, listen.summary(self.python).splitlines()[1:])

    def test_the_run_is_heard_as_more_than_silence(self):
        self.assertIsNotNone(self.rust["analysis"]["integrated_lufs"])
        self.assertGreater(len(self.rust["sounds"]), 0)


if __name__ == "__main__":
    unittest.main()
