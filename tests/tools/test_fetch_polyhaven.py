import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools" / "cc0_textures"))
import fetch_polyhaven

FILES = {
    "Diffuse": {"1k": {"jpg": {"url": "https://example/diff.jpg"}}},
    "AO": {"1k": {"jpg": {"url": "https://example/ao.jpg"}}},
    "nor_gl": {"1k": {"jpg": {"url": "https://example/nor.jpg"}}},
}


class FetchPolyHavenTest(unittest.TestCase):
    def test_maps_are_named_for_psx_bake(self):
        urls = fetch_polyhaven.map_urls(FILES, "1k", "jpg")
        self.assertEqual(sorted(urls), ["AmbientOcclusion", "Color"])
        self.assertEqual(fetch_polyhaven.file_name("pine_bark", "1k", "jpg", "Color"), "pine_bark_1k_Color.jpg")

    def test_missing_diffuse_is_refused(self):
        with self.assertRaises(KeyError):
            fetch_polyhaven.map_urls({"AO": FILES["AO"]}, "1k", "jpg")

    def test_manifest_merges_by_asset(self):
        with tempfile.TemporaryDirectory() as folder:
            manifest = Path(folder) / "manifest.json"
            manifest.write_text(json.dumps([{"asset": "Bark002", "sha256": "a"}, {"asset": "pine_bark", "sha256": "old"}]), encoding="utf-8")
            fetch_polyhaven.merge_manifest(manifest, [{"asset": "pine_bark", "sha256": "new"}])
            records = json.loads(manifest.read_text(encoding="utf-8"))
        self.assertEqual([record["asset"] for record in records], ["Bark002", "pine_bark"])
        self.assertEqual(records[1]["sha256"], "new")


if __name__ == "__main__":
    unittest.main()
