import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "cc0_audio"))
import fetch_freesound

PAGE = '''<meta property="og:title" content="Wooden Floor Body Slams by Project_Trident"
<a href="https://creativecommons.org/publicdomain/zero/1.0/">Creative Commons 0</a>
<audio src="https://cdn.freesound.org/previews/128/128103_2338419-hq.mp3"></audio>
<audio src="https://cdn.freesound.org/previews/447/447922_9159316-hq.mp3"></audio>'''


class FetchFreesoundTest(unittest.TestCase):
    def test_sound_ids_come_from_both_page_forms(self):
        self.assertEqual(fetch_freesound.sound_id("https://freesound.org/people/Project_Trident/sounds/128103/"), "128103")
        self.assertEqual(fetch_freesound.sound_id("https://freesound.org/s/734629/"), "734629")

    def test_a_page_gives_its_own_preview_title_author_and_licence(self):
        page = fetch_freesound.read_page(PAGE, "128103")
        self.assertEqual(page["preview"], "https://cdn.freesound.org/previews/128/128103_2338419-hq.mp3")
        self.assertEqual((page["title"], page["author"]), ("Wooden Floor Body Slams", "Project_Trident"))
        self.assertEqual(page["licences"], [fetch_freesound.CC0_LINK])

    def test_an_attribution_licence_is_seen(self):
        page = fetch_freesound.read_page('<a href="https://creativecommons.org/licenses/by/4.0/">', "1")
        self.assertEqual(page["licences"], ["creativecommons.org/licenses/by/4.0"])


if __name__ == "__main__":
    unittest.main()
