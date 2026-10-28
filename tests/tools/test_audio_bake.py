import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np
import soundfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "audio_bake"))
import bake

SPEC = {"sample_rate": 48000, "reference_lufs": -18.0, "momentary_window_s": 0.4, "momentary_hop_s": 0.1, "fade_in_s": 0.002, "fade_out_s": 0.02}


class PeakMeter:
    def __init__(self):
        self.lengths = []

    def short_term_lufs_max(self, audio, rate, window_s, hop_s):
        self.lengths.append(len(audio))
        return bake.peak_dbfs(np.asarray(audio))


def tone(seconds, rate, amplitude):
    return amplitude * np.sin(2 * np.pi * 440 * np.arange(int(seconds * rate)) / rate)


class AudioBakeTest(unittest.TestCase):
    def test_levelling_moves_the_loudest_window_to_the_reference_plus_trim(self):
        levelled, measured = bake.level(PeakMeter(), tone(1.0, 48000, 0.1), 48000, SPEC, -2.0)
        self.assertAlmostEqual(bake.peak_dbfs(levelled), -20.0, places=6)
        self.assertAlmostEqual(measured["gain_db"], -0.0, places=1)
        self.assertEqual(measured["lufs_after"], -20.0)

    def test_a_short_sound_is_padded_to_one_window(self):
        meter = PeakMeter()
        bake.level(meter, tone(0.05, 48000, 0.1), 48000, SPEC, 0.0)
        self.assertTrue(all(length >= int(0.4 * 48000) for length in meter.lengths))

    def test_a_gain_that_would_clip_is_refused(self):
        with self.assertRaises(SystemExit):
            bake.level(PeakMeter(), tone(1.0, 48000, 0.1), 48000, dict(SPEC, reference_lufs=3.0), 0.0)

    def test_silence_is_refused(self):
        with self.assertRaises(SystemExit):
            bake.level(PeakMeter(), np.zeros(48000), 48000, SPEC, 0.0)

    def test_fades_start_and_end_at_zero(self):
        faded = bake.fade(np.ones(4800), 48000, 0.002, 0.02)
        self.assertEqual((faded[0], faded[-1]), (0.0, 0.0))
        self.assertEqual(faded[2400], 1.0)

    def test_a_range_outside_the_source_is_refused(self):
        with self.assertRaises(SystemExit):
            bake.cut(np.zeros((480, 1)), 48000, 0.0, 0.5)

    def test_a_manifest_source_is_cut_folded_resampled_and_written_as_16_bit_mono(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "work").mkdir()
            stereo = np.stack([tone(1.0, 24000, 0.2), tone(1.0, 24000, 0.2)], axis=1)
            soundfile.write(str(root / "work" / "fs_1.wav"), stereo, 24000, subtype="PCM_16")
            digest = hashlib.sha256((root / "work" / "fs_1.wav").read_bytes()).hexdigest()
            (root / "freesound.json").write_text(json.dumps([{"id": "1", "file": "fs_1.wav", "sha256": digest}]), encoding="utf-8")
            sound = {"source": {"manifest": "freesound.json", "id": "1", "folder": "work"}, "start": 0.25, "end": 0.75}
            row = bake.bake("hit_01", sound, SPEC, PeakMeter(), root, root / "out")
            info = soundfile.info(str(root / "out" / "hit_01.wav"))
            self.assertEqual((info.samplerate, info.channels, info.subtype, info.frames), (48000, 1, "PCM_16", 24000))
            self.assertEqual(row["sha256"], hashlib.sha256((root / "out" / "hit_01.wav").read_bytes()).hexdigest())
            self.assertEqual(row["source"], {"manifest": "freesound.json", "id": "1", "sha256": digest})

    def test_a_changed_manifest_source_is_refused(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            soundfile.write(str(root / "fs_1.wav"), tone(1.0, 48000, 0.2), 48000, subtype="PCM_16")
            (root / "freesound.json").write_text(json.dumps([{"id": "1", "file": "fs_1.wav", "sha256": "0" * 64}]), encoding="utf-8")
            with self.assertRaises(SystemExit):
                bake.manifest_source({"manifest": "freesound.json", "id": "1", "folder": "."}, root)

    def test_a_synth_source_renders_through_synth_py(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            spec = {"sounds": {"spawn": {"seconds": 0.3, "layers": [{"type": "tone", "freq": [600, 900], "decay": 0.1}]}}}
            (root / "synth.json").write_text(json.dumps(spec), encoding="utf-8")
            row = bake.bake("spawn", {"source": {"synth": "synth.json", "id": "spawn"}}, SPEC, PeakMeter(), root, root / "out")
            self.assertEqual(row["source"], {"synth": "synth.json", "id": "spawn"})
            self.assertEqual(soundfile.info(str(root / "out" / "spawn.wav")).frames, int(0.3 * 48000))


if __name__ == "__main__":
    unittest.main()
