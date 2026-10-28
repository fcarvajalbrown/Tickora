import argparse
import subprocess
import tempfile
from pathlib import Path

import numpy as np
import soundfile
from scipy.signal import resample_poly

RATE = 48000


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description="Join wav files into one listening file, each preceded by its spoken number, so a listener can name what he hears by number.")
    parser.add_argument("wavs", nargs="+", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--gap-s", type=float, default=1.5)
    parser.add_argument("--voice-db", type=float, default=-12.0)
    parser.add_argument("--rms-db", type=float, default=None, help="set every sound to this RMS in dBFS, so quiet and loud takes are judged alike")
    return parser.parse_args(argv)


def speak(text, path):
    script = f"Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; $s.SetOutputToWaveFile('{path}'); $s.Speak('{text}'); $s.Dispose()"
    subprocess.run(["powershell", "-NoProfile", "-Command", script], check=True, stdin=subprocess.DEVNULL)


def mono_at_rate(path):
    audio, rate = soundfile.read(str(path), always_2d=True)
    mono = audio.mean(axis=1)
    if rate != RATE:
        mono = resample_poly(mono, RATE, rate)
    return mono


def peak_to(signal, db):
    peak = float(np.max(np.abs(signal))) if signal.size else 0.0
    return signal * (10.0 ** (db / 20.0) / peak) if peak > 0.0 else signal


def rms_to(signal, db):
    rms = float(np.sqrt(np.mean(signal ** 2))) if signal.size else 0.0
    return signal * (10.0 ** (db / 20.0) / rms) if rms > 0.0 else signal


def join(voices, sounds, gap_s):
    gap = np.zeros(int(gap_s * RATE))
    short = np.zeros(int(0.4 * RATE))
    parts = []
    for voice, sound in zip(voices, sounds):
        parts += [voice, short, sound, gap]
    return np.concatenate(parts) if parts else np.zeros(0)


def main(argv=None):
    args = parse_args(argv)
    with tempfile.TemporaryDirectory() as work:
        voices = []
        for number in range(1, len(args.wavs) + 1):
            voice_path = Path(work) / f"voice_{number}.wav"
            speak(str(number), voice_path)
            voices.append(peak_to(mono_at_rate(voice_path), args.voice_db))
    sounds = [mono_at_rate(path) for path in args.wavs]
    if args.rms_db is not None:
        sounds = [rms_to(sound, args.rms_db) for sound in sounds]
    out = np.clip(join(voices, sounds, args.gap_s), -1.0, 1.0)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    soundfile.write(str(args.out), out, RATE, subtype="PCM_16")
    for number, path in enumerate(args.wavs, 1):
        print(f"{number}  {path.name}")


if __name__ == "__main__":
    main()
