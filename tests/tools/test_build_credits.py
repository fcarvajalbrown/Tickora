import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "credits"))

import build_credits


class ReaderTest(unittest.TestCase):
    def test_radio_songs_carry_title_album_licence_and_source(self):
        rows = {"songs": [{"title": "La cueca", "album": "Disco", "licence": "CC0 1.0", "page": "https://archive.org/details/x"}]}
        self.assertEqual(build_credits.radio_songs(rows), [{"name": "La cueca", "lines": ["Disco", "CC0 1.0", "https://archive.org/details/x"]}])

    def test_archive_manifest_lists_each_file_under_the_item(self):
        rows = {"title": "Red Library: Impacts", "page": "https://archive.org/details/R", "licenseurl": "https://creativecommons.org/publicdomain/zero/1.0/", "files": [{"file": "a.wav"}, {"file": "b.wav"}]}
        self.assertEqual(build_credits.archive_manifest(rows), [{"name": "Red Library: Impacts", "lines": ["a, b", "CC0 1.0", "https://archive.org/details/R"]}])

    def test_named_sources_and_textures(self):
        self.assertEqual(build_credits.named_sources([{"name": "pack_one", "page": "https://p", "licence": "CC0-1.0"}]), [{"name": "pack_one", "lines": ["CC0-1.0", "https://p"]}])
        textures = [{"asset": "Cardboard001", "page": "https://ambientcg.com/view?id=Cardboard001", "licence": "CC0 1.0"}, {"asset": "Metal002", "page": "https://ambientcg.com/view?id=Metal002", "licence": "CC0 1.0"}]
        self.assertEqual(build_credits.textures(textures), [{"name": "ambientCG", "lines": ["Lennart Demes", "Cardboard001, Metal002", "CC0 1.0", "https://ambientcg.com"]}])

    def test_a_synth_source_can_carry_a_name_and_an_author(self):
        sfx = {"sources": {"bsb_buzzer": {"name": "BigSoundBank: Buzzer", "author": "Joseph Sardin", "licence": "CC0 1.0", "url": "https://bigsoundbank.com/b"}}}
        self.assertEqual(build_credits.synth_sources(sfx), [{"name": "BigSoundBank: Buzzer", "lines": ["Joseph Sardin", "CC0 1.0", "https://bigsoundbank.com/b"]}])

    def test_solo_maintainers_are_skipped_when_the_section_names_them(self):
        sfx = {"sources": {"a": {"author": "Joseph Sardin", "licence": "CC0 1.0", "url": "u1"}, "b": {"author": "Other", "licence": "CC0 1.0", "url": "u2"}}}
        self.assertEqual([entry["name"] for entry in build_credits.synth_sources(sfx, skip_authors=["Joseph Sardin"])], ["b"])
        rows = [{"asset": "A", "page": "https://ambientcg.com/view?id=A", "licence": "CC0 1.0"}, {"asset": "B", "page": "https://polyhaven.com/a/B", "licence": "CC0 1.0"}]
        self.assertEqual([entry["name"] for entry in build_credits.textures(rows, skip_sites=["ambientcg.com"])], ["Poly Haven"])

    def test_named_sources_drop_per_asset_rows_from_a_skipped_site(self):
        rows = [{"name": "bsb_0448", "page": "https://bigsoundbank.com/iphone-camera-s0448.html", "licence": "CC0-1.0"}, {"name": "www_bsb", "page": "https://www.bigsoundbank.com/x-s1.html", "licence": "CC0-1.0"}, {"name": "kept", "page": "https://opengameart.org/content/k", "licence": "CC0-1.0"}]
        self.assertEqual([entry["name"] for entry in build_credits.named_sources(rows, skip_sites=["bigsoundbank.com"])], ["kept"])

    def test_section_options_reach_the_reader(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "s.json").write_text(json.dumps({"sources": {"a": {"author": "X", "licence": "L", "url": "u"}}}), encoding="utf-8")
            spec = {"header": [], "sections": [{"title": "Sound", "from": "synth_sources", "path": "s.json", "options": {"skip_authors": ["X"]}}]}
            self.assertEqual(build_credits.build(spec, root)["sections"][0]["entries"], [])

    def test_a_named_source_shows_its_title_and_author_when_it_has_them(self):
        row = {"name": "pack_one", "title": "Pack One", "author": "Someone", "page": "https://p", "licence": "CC0-1.0"}
        self.assertEqual(build_credits.named_sources([row]), [{"name": "Pack One", "lines": ["Someone", "CC0-1.0", "https://p"]}])

    def test_a_fetched_sound_with_a_title_and_no_name_is_credited(self):
        row = {"id": "177958", "title": "Water Dripping in Cave.wav", "author": "Sclolex", "page": "https://freesound.org/people/Sclolex/sounds/177958/", "licence": "CC0 1.0"}
        self.assertEqual(build_credits.named_sources([row])[0]["name"], "Water Dripping in Cave.wav")

    def test_synth_sources_drop_a_repeated_or_skipped_url(self):
        sfx = {"sources": {"a": {"licence": "L", "url": "u1"}, "a_down": {"licence": "L", "url": "u1"}, "b": {"licence": "L", "url": "u2"}, "c": {"licence": "L", "url": "u3"}}}
        self.assertEqual([entry["name"] for entry in build_credits.synth_sources(sfx, skip_urls=["u3"])], ["a", "b"])

    def test_synth_sources_name_the_recording_and_its_licence(self):
        sfx = {"sources": {"storm": {"licence": "Public Domain Mark 1.0", "url": "https://archive.org/details/s"}}}
        self.assertEqual(build_credits.synth_sources(sfx), [{"name": "storm", "lines": ["Public Domain Mark 1.0", "https://archive.org/details/s"]}])


class BuildTest(unittest.TestCase):
    def test_sections_with_one_title_merge_and_the_header_leads(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "a.json").write_text(json.dumps([{"name": "n", "page": "p", "licence": "CC0-1.0"}]), encoding="utf-8")
            spec = {"header": ["Game"], "sections": [
                {"title": "Sound", "entries": [{"name": "fixed", "lines": ["x"]}]},
                {"title": "Sound", "from": "named_sources", "path": "a.json"},
            ]}
            credits = build_credits.build(spec, root)
        self.assertEqual(credits["header"], ["Game"])
        self.assertEqual([section["title"] for section in credits["sections"]], ["Sound"])
        self.assertEqual([entry["name"] for entry in credits["sections"][0]["entries"]], ["fixed", "n"])

    def test_unknown_reader_is_refused(self):
        with self.assertRaises(SystemExit):
            build_credits.build({"header": [], "sections": [{"title": "X", "from": "nope", "path": "x"}]}, Path("."))

    def test_public_domain_entries_are_credited_only_from_their_makers_own_sites(self):
        spec = {"header": [], "public_domain_own_sites": ["polyhaven.com"], "sections": [{"title": "Sound", "from": "named_sources", "path": "rows.json"}]}
        rows = [
            {"name": "a", "licence": "CC0 1.0", "page": "https://freesound.org/people/x/sounds/1/"},
            {"name": "b", "licence": "Public Domain Mark 1.0", "page": "https://archive.org/details/y"},
            {"name": "c", "licence": "CC0 1.0", "page": "https://polyhaven.com/a/z"},
            {"name": "d", "licence": "Creative Commons Attribution", "page": "https://archive.org/details/w"},
        ]
        with tempfile.TemporaryDirectory() as folder:
            (Path(folder) / "rows.json").write_text(json.dumps(rows), encoding="utf-8")
            credits = build_credits.build(spec, Path(folder))
        self.assertEqual([entry["name"] for entry in credits["sections"][0]["entries"]], ["c", "d"])


if __name__ == "__main__":
    unittest.main()
