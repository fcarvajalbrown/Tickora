import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools" / "cc0_audio"))
import fetch_commons


def meta(code, name):
    return {"License": {"value": code}, "LicenseShortName": {"value": name}}


class LicenceTest(unittest.TestCase):
    def test_cc0_and_public_domain_are_accepted(self):
        self.assertTrue(fetch_commons.is_free(meta("cc0", "CC0")))
        self.assertTrue(fetch_commons.is_free(meta("pd", "Public domain")))
        self.assertTrue(fetch_commons.is_free(meta("", "Public domain")))
        self.assertTrue(fetch_commons.is_free(meta("pd-self", "PD-self")))

    def test_attribution_and_share_alike_are_refused(self):
        self.assertFalse(fetch_commons.is_free(meta("cc-by-sa-4.0", "CC BY-SA 4.0")))
        self.assertFalse(fetch_commons.is_free(meta("cc-by-3.0", "CC BY 3.0")))
        self.assertFalse(fetch_commons.is_free(meta("gfdl", "GFDL")))
        self.assertFalse(fetch_commons.is_free(meta("", "")))

    def test_html_in_metadata_is_stripped(self):
        self.assertEqual(fetch_commons.plain("<span>Public <b>domain</b></span>"), "Public domain")


if __name__ == "__main__":
    unittest.main()
