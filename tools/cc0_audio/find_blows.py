import argparse
from pathlib import Path

import numpy as np
import soundfile


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description="List the onsets of single impacts in a recording: time, peak dBFS and decay time to 20 dB under the peak.")
    parser.add_argument("files", type=Path, nargs="+")
    parser.add_argument("--window-ms", type=float, default=5.0)
    parser.add_argument("--min-gap-ms", type=float, default=120.0)
    parser.add_argument("--threshold-db", type=float, default=-30.0)
    return parser.parse_args(argv)


def envelope(mono, rate, window_ms):
    window = max(1, int(rate * window_ms / 1000.0))
    return np.sqrt(np.convolve(mono ** 2, np.ones(window) / window, mode="same"))


def blows(mono, rate, args):
    env = envelope(mono, rate, args.window_ms)
    top = env.max()
    floor = top * 10 ** (args.threshold_db / 20.0)
    gap = int(rate * args.min_gap_ms / 1000.0)
    found = []
    index = 0
    while index < len(env):
        if env[index] >= floor:
            end = min(len(env), index + gap)
            peak_at = index + int(np.argmax(env[index:end]))
            sample_peak = float(np.abs(mono[index:end]).max())
            found.append((index / rate, 20 * np.log10(max(sample_peak, 1e-9)), peak_at))
            index = end
            while index < len(env) and env[index] >= floor:
                index += 1
        else:
            index += 1
    return found


def decay_ms(mono, rate, start):
    env = envelope(mono, rate, 5.0)
    peak = env[start:start + int(rate * 0.3)].max()
    below = np.nonzero(env[start:] < peak * 0.1)[0]
    return 1000.0 * (below[0] if below.size else len(env) - start) / rate


def main(argv=None):
    args = parse_args(argv)
    for path in args.files:
        audio, rate = soundfile.read(str(path), always_2d=True)
        mono = audio.mean(axis=1)
        print(f"{path.name}: {len(mono) / rate:.2f} s, {rate} Hz, peak {20 * np.log10(np.abs(audio).max()):.1f} dBFS")
        for time_s, peak_db, at in blows(mono, rate, args):
            print(f"  blow at {time_s:.3f} s, peak {peak_db:.1f} dBFS, decay to -20 dB {decay_ms(mono, rate, at):.0f} ms")


if __name__ == "__main__":
    main()
