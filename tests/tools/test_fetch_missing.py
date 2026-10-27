import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools" / "cc0_textures"))

import fetch_missing

SPEC = {"source_root": "src", "materials": {
    "bark": {"asset": "pine_bark", "tile_m": 1.0},
    "steel": {"asset": "Metal001", "tile_m": 1.0},
    "painted": {"generated": True, "tile_m": 1.0}}}
MANIFEST = [
    {"asset": "pine_bark", "attribute": "polyhaven-1k-jpg", "sha256": "aa"},
    {"asset": "Metal001", "attribute": "1K-JPG", "sha256": "bb"}]


def recording_fetchers(calls, digests=None):
    def make(source):
        def fetch(record, dest):
            calls.append((source, record["asset"]))
            (dest / record["asset"]).mkdir(parents=True, exist_ok=True)
            return {"asset": record["asset"], "sha256": (digests or {}).get(record["asset"], record["sha256"])}
        return fetch
    return {"ambientcg": make("ambientcg"), "polyhaven": make("polyhaven")}


class FetchMissingTest(unittest.TestCase):
    def test_only_missing_sources_are_fetched_from_their_own_site(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "src" / "Metal001").mkdir(parents=True)
            calls = []
            fetched = fetch_missing.fetch_missing(SPEC, MANIFEST, root, recording_fetchers(calls))
            self.assertEqual(fetched, ["pine_bark"])
            self.assertEqual(calls, [("polyhaven", "pine_bark")])
            self.assertTrue((root / "src" / "manifest.json").exists())

    def test_nothing_is_fetched_when_every_source_is_present(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            for asset in ("pine_bark", "Metal001"):
                (root / "src" / asset).mkdir(parents=True)
            calls = []
            self.assertEqual(fetch_missing.fetch_missing(SPEC, MANIFEST, root, recording_fetchers(calls)), [])
            self.assertEqual(calls, [])

    def test_a_download_that_does_not_match_the_manifest_stops_the_build(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            with self.assertRaises(SystemExit):
                fetch_missing.fetch_missing(SPEC, MANIFEST, root, recording_fetchers([], {"Metal001": "zz"}))

    def test_an_asset_missing_from_the_manifest_stops_the_build(self):
        with tempfile.TemporaryDirectory() as folder:
            spec = {"source_root": "src", "materials": {"odd": {"asset": "Unknown001", "tile_m": 1.0}}}
            with self.assertRaises(SystemExit):
                fetch_missing.fetch_missing(spec, MANIFEST, Path(folder), recording_fetchers([]))

    def test_polyhaven_attribute_splits_into_resolution_and_kind(self):
        self.assertEqual(fetch_missing.source_of({"attribute": "polyhaven-1k-jpg"}), "polyhaven")
        self.assertEqual(fetch_missing.source_of({"attribute": "1K-JPG"}), "ambientcg")


if __name__ == "__main__":
    unittest.main()
