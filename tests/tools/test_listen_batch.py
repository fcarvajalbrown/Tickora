import sys
import unittest
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools" / "listen_batch"))
import listen_batch


class ListenBatchTest(unittest.TestCase):
    def test_each_sound_follows_its_number_and_a_gap(self):
        voices = [np.full(10, 0.1), np.full(10, 0.2)]
        sounds = [np.full(5, 0.5), np.full(5, 0.6)]
        out = listen_batch.join(voices, sounds, 0.001)
        short = int(0.4 * listen_batch.RATE)
        gap = int(0.001 * listen_batch.RATE)
        self.assertEqual(out.size, 2 * (10 + short + 5 + gap))
        second = 10 + short + 5 + gap
        self.assertTrue(np.allclose(out[:10], 0.1))
        self.assertTrue(np.allclose(out[10 + short:10 + short + 5], 0.5))
        self.assertTrue(np.allclose(out[second:second + 10], 0.2))

    def test_the_voice_is_set_to_its_peak(self):
        voice = listen_batch.peak_to(np.array([0.0, 0.5, -0.25]), -6.0)
        self.assertAlmostEqual(float(np.max(np.abs(voice))), 10 ** (-6 / 20))

    def test_every_sound_can_be_set_to_one_rms(self):
        sound = listen_batch.rms_to(np.array([0.1, -0.1, 0.1, -0.1]), -20.0)
        self.assertAlmostEqual(float(np.sqrt(np.mean(sound ** 2))), 10 ** (-20 / 20))


if __name__ == "__main__":
    unittest.main()
