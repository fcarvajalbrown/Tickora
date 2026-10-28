import hashlib
import json
import sys
import unittest
from pathlib import Path

import soundfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "credits"))
import build_credits

AUDIO = ROOT / "assets" / "gen" / "audio"


class AudioAssetsTest(unittest.TestCase):
    def test_every_baked_sound_matches_its_manifest_and_format(self):
        manifest = json.loads((AUDIO / "manifest.json").read_text(encoding="utf-8"))
        spec = json.loads((ROOT / "specs" / "audio" / "bake.json").read_text(encoding="utf-8"))
        self.assertEqual(sorted(manifest), sorted(spec["sounds"]))
        for name, row in manifest.items():
            path = AUDIO / row["file"]
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), row["sha256"], name)
            info = soundfile.info(str(path))
            self.assertEqual((info.samplerate, info.channels, info.subtype), (48000, 1, "PCM_16"), name)

    def test_no_wav_is_left_outside_the_manifest(self):
        manifest = json.loads((AUDIO / "manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(sorted(path.name for path in AUDIO.glob("*.wav")), sorted(row["file"] for row in manifest.values()))

    def test_credits_are_what_build_credits_writes_from_the_spec(self):
        spec = json.loads((ROOT / "specs" / "credits.json").read_text(encoding="utf-8"))
        expected = build_credits.markdown(build_credits.build(spec, ROOT))
        self.assertEqual((AUDIO / "CREDITS.md").read_text(encoding="utf-8"), expected)


if __name__ == "__main__":
    unittest.main()
