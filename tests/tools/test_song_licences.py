import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools" / "radio_songs"))

import song_licences


class LicenceTest(unittest.TestCase):
    def test_creative_commons_urls_are_recognised(self):
        self.assertTrue(song_licences.is_creative_commons("https://creativecommons.org/publicdomain/zero/1.0/"))
        self.assertTrue(song_licences.is_creative_commons("http://creativecommons.org/licenses/by-nc-nd/3.0/"))
        self.assertFalse(song_licences.is_creative_commons(None))
        self.assertFalse(song_licences.is_creative_commons(""))
        self.assertFalse(song_licences.is_creative_commons("https://example.org/licence"))

    def test_licence_names_read_from_the_url(self):
        self.assertEqual(song_licences.licence_name("https://creativecommons.org/publicdomain/zero/1.0/"), "CC0 1.0")
        self.assertEqual(song_licences.licence_name("http://creativecommons.org/licenses/by-nc-nd/3.0/"), "CC BY-NC-ND 3.0")
        self.assertEqual(song_licences.licence_name("https://creativecommons.org/licenses/by-sa/4.0/deed.es"), "CC BY-SA 4.0")
        self.assertEqual(song_licences.licence_name(None), "none stated")


class SelectTest(unittest.TestCase):
    def test_songs_filter_by_pattern_and_duration(self):
        songs = [
            {"id": "a", "kind": "song", "title": "Cueca larga", "collection": "X", "duration": 300.0, "source": "https://archive.org/details/x"},
            {"id": "b", "kind": "song", "title": "La cueca", "collection": "X", "duration": 120.0, "source": "https://archive.org/details/x"},
            {"id": "c", "kind": "song", "title": "Vals", "collection": "Cuecas varias", "duration": 100.0, "source": "https://archive.org/details/y"},
            {"id": "d", "kind": "talk", "title": "cueca", "collection": "", "duration": 100.0, "source": "https://archive.org/details/z"},
        ]
        picked = song_licences.select(songs, "cueca", 90.0, 180.0)
        self.assertEqual([song["id"] for song in picked], ["b", "c"])

    def test_item_comes_from_the_source_url(self):
        self.assertEqual(song_licences.item_of({"source": "https://archive.org/details/Quilapayun.pueblo"}), "Quilapayun.pueblo")

    def test_the_audio_file_closest_in_length_and_name_is_chosen(self):
        files = [
            {"name": "01 Otra.mp3", "length": "120.0"},
            {"name": "02 La cueca.mp3", "length": "121.0"},
            {"name": "02 La cueca.ogg", "length": "121.0"},
            {"name": "cover.jpg"},
        ]
        self.assertEqual(song_licences.audio_file(files, "La cueca", 121.0), "02 La cueca.mp3")
        self.assertEqual(song_licences.audio_file([{"name": "a.mp3", "length": "01:15"}], "x", 75.0), "a.mp3")
        self.assertIsNone(song_licences.audio_file([{"name": "a.mp3", "length": "300"}], "x", 75.0))

    def test_a_matching_name_wins_over_a_listed_length_that_is_off(self):
        files = [
            {"name": "05 Que sacaran con querer (Cueca).mp3", "length": "103.0"},
            {"name": "06 Estando con llave el pecho (Cueca).mp3", "length": "110.0"},
        ]
        self.assertEqual(song_licences.audio_file(files, "Estando con llave el pecho (Cueca)", 103.0), "06 Estando con llave el pecho (Cueca).mp3")


if __name__ == "__main__":
    unittest.main()
