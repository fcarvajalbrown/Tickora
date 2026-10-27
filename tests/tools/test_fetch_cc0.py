import json
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools" / "cc0_fetch"))

import fetch_cc0


class FetchCc0Test(unittest.TestCase):
    def make_zip(self, folder, names):
        archive = Path(folder) / "pack.zip"
        with zipfile.ZipFile(archive, "w") as handle:
            for name in names:
                handle.writestr(name, b"data")
        return archive

    def test_unpack_keeps_files_inside_the_destination(self):
        with tempfile.TemporaryDirectory() as folder:
            archive = self.make_zip(folder, ["a/hit.wav", "../escape.wav"])
            dest = Path(folder) / "out"
            written = fetch_cc0.unpack(archive, dest)
            self.assertEqual([p.relative_to(dest.resolve()).as_posix() for p in written], ["a/hit.wav"])
            self.assertFalse((Path(folder) / "escape.wav").exists())

    def test_record_appends_source_licence_and_hash(self):
        with tempfile.TemporaryDirectory() as folder:
            archive = self.make_zip(folder, ["hit.wav"])
            manifest = Path(folder) / "manifest.json"
            fetch_cc0.record(manifest, "pack", "https://example.org/pack.zip", "https://example.org/pack", "CC0-1.0", archive)
            fetch_cc0.record(manifest, "pack", "https://example.org/pack.zip", "https://example.org/pack", "CC0-1.0", archive)
            rows = json.loads(manifest.read_text(encoding="utf-8"))
            self.assertEqual(len(rows), 1)
            self.assertEqual(rows[0]["licence"], "CC0-1.0")
            self.assertEqual(len(rows[0]["sha256"]), 64)


if __name__ == "__main__":
    unittest.main()
