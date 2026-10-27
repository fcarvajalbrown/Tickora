import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools" / "cc0_audio"))

import fetch_bigsoundbank


class FetchTest(unittest.TestCase):
    def test_the_number_comes_from_the_page_url(self):
        self.assertEqual(fetch_bigsoundbank.sound_number("https://bigsoundbank.com/low-frequency-continuous-buzzer-s0593.html"), "0593")

    def test_a_url_that_is_not_a_sound_page_is_refused(self):
        with self.assertRaises(ValueError):
            fetch_bigsoundbank.sound_number("https://bigsoundbank.com/search?q=buzzer")

    def test_the_page_gives_its_title_and_licence(self):
        html = "<meta property='og:title' content=\"Low frequency continuous buzzer — Free Sound Effect Download | BigSoundBank\"><p>CC0 (public domain): Free and royalty-free</p>"
        self.assertEqual(fetch_bigsoundbank.read_page(html), {"title": "Low frequency continuous buzzer", "cc0": True})

    def test_either_title_suffix_is_dropped(self):
        html = "<meta property='og:title' content=\"Industrial Doorbell #1 — Free Download WAV & MP3 | BigSoundBank\">"
        self.assertEqual(fetch_bigsoundbank.read_page(html)["title"], "Industrial Doorbell #1")

    def test_a_page_without_the_cc0_mark_is_flagged(self):
        self.assertFalse(fetch_bigsoundbank.read_page("<p>All rights reserved</p>")["cc0"])


if __name__ == "__main__":
    unittest.main()
