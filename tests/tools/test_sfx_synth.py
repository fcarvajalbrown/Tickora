import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "sfx_synth"))
import synth

SPEC = {"seconds": 0.5, "layers": [
    {"type": "noise", "colour": "brown", "attack": 0.001, "decay": 0.05, "lowpass": 800},
    {"type": "tone", "freq": [300, 150], "gain_db": -6, "decay": 0.1, "repeat": {"count": 2, "every_s": 0.2}},
]}


class SynthTest(unittest.TestCase):
    def test_same_spec_same_samples(self):
        first = synth.render("CUE", SPEC, {}, 48000)
        second = synth.render("CUE", SPEC, {}, 48000)
        self.assertTrue(np.array_equal(first, second))

    def test_peak_is_normalised_and_dc_removed(self):
        signal = synth.render("CUE", SPEC, {}, 48000)
        self.assertAlmostEqual(float(np.max(np.abs(signal))), synth.db(synth.PEAK_DBFS), places=6)
        self.assertLess(abs(float(np.mean(signal))), 0.01)

    def test_seamless_loop_keeps_length_minus_crossfade(self):
        signal = np.sin(np.linspace(0, 40 * np.pi, 4800))
        looped = synth.seamless(signal, 48000, 0.01)
        self.assertEqual(len(looped), 4800 - 480)

    def test_changed_source_is_refused(self):
        with self.assertRaises(SystemExit):
            synth.verify_sources({"x": {"path": str(Path(__file__)), "sha256": "0" * 64}})

    def test_stereo_channels_are_decorrelated_and_left_matches_mono(self):
        stereo = dict(SPEC, stereo=True)
        mono = synth.render("CUE", SPEC, {}, 48000)
        both = synth.render("CUE", stereo, {}, 48000)
        self.assertEqual(both.shape, (len(mono), 2))
        self.assertLess(abs(float(np.corrcoef(both[:, 0], both[:, 1])[0, 1])), 0.9)
        left = both[:, 0] / np.max(np.abs(both[:, 0]))
        self.assertTrue(np.allclose(left, mono / np.max(np.abs(mono))))


    def test_room_leaves_the_dry_hit_and_adds_a_decaying_tail(self):
        rate = 8000
        hit = np.zeros(rate)
        hit[0] = 1.0
        room = {"decay_s": 0.5, "wet_db": -10, "predelay_s": 0.01}
        wet = synth.with_room(hit, rate, room)
        self.assertAlmostEqual(float(wet[0]), 1.0, places=9)
        self.assertLess(float(np.max(np.abs(wet[1:int(0.01 * rate)]))), 1e-9)
        early = np.sum(wet[int(0.01 * rate):int(0.1 * rate)] ** 2)
        late = np.sum(wet[int(0.4 * rate):int(0.5 * rate)] ** 2)
        self.assertGreater(early, late * 100)
        self.assertLess(float(np.max(np.abs(wet[int(0.5 * rate) + 1:]))), 1e-9)

    def test_a_written_sound_is_the_same_bytes_twice(self):
        with tempfile.TemporaryDirectory() as folder:
            paths = [Path(folder) / f"{run}.wav" for run in ("first", "second")]
            for path in paths:
                synth.write(path, synth.render("CUE", SPEC, {}, 48000), 48000, 1)
            self.assertEqual(paths[0].read_bytes(), paths[1].read_bytes())


if __name__ == "__main__":
    unittest.main()
