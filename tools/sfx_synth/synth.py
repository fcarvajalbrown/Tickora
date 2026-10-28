import argparse
import hashlib
import json
import sys
from pathlib import Path

import numpy as np
import soundfile
from scipy.signal import fftconvolve, lfilter

PEAK_DBFS = -1.0
DEFAULT_MIX_FILTER = {"highpass": 60}


def db(value):
    return 10.0 ** (value / 20.0)


def envelope(length, rate, layer):
    t = np.arange(length) / rate
    attack = max(float(layer.get("attack", 0.002)), 1e-4)
    decay = layer.get("decay")
    shape = np.clip(t / attack, 0.0, 1.0)
    if decay is not None:
        shape = shape * np.exp(-np.maximum(t - attack, 0.0) / max(float(decay), 1e-4))
    hold = layer.get("hold")
    if hold is not None:
        release = float(layer.get("release", 0.05))
        shape = shape * np.clip(1.0 - (t - float(hold)) / release, 0.0, 1.0)
    return shape


def one_pole_lowpass(signal, rate, cutoff):
    alpha = 1.0 - np.exp(-2.0 * np.pi * float(cutoff) / rate)
    return lfilter([alpha], [1.0, alpha - 1.0], signal)


def filtered(signal, rate, layer):
    if "lowpass" in layer:
        for _ in range(int(layer.get("poles", 1))):
            signal = one_pole_lowpass(signal, rate, layer["lowpass"])
    if "highpass" in layer:
        signal = signal - one_pole_lowpass(signal, rate, layer["highpass"])
    return signal


def coloured_noise(rng, length, colour):
    white = rng.standard_normal(length)
    if colour == "white":
        return white
    spectrum = np.fft.rfft(white)
    frequencies = np.fft.rfftfreq(length)
    frequencies[0] = frequencies[1] if length > 1 else 1.0
    spectrum = spectrum / (np.sqrt(frequencies) if colour == "pink" else frequencies)
    spectrum[0] = 0.0
    noise = np.fft.irfft(spectrum, length)
    return noise / (np.std(noise) or 1.0)


def oscillator(wave, phase):
    if wave == "sine":
        return np.sin(phase)
    if wave == "square":
        return np.sign(np.sin(phase))
    if wave == "saw":
        return 2.0 * ((phase / (2 * np.pi)) % 1.0) - 1.0
    if wave == "triangle":
        return 2.0 * np.abs(2.0 * ((phase / (2 * np.pi)) % 1.0) - 1.0) - 1.0
    raise ValueError(f"unknown wave {wave}")


def tone(rng, length, rate, layer):
    t = np.arange(length) / rate
    start, end = float(layer["freq"][0]), float(layer["freq"][-1])
    duration = max(length / rate, 1e-6)
    frequency = start * (end / start) ** (t / duration) if start > 0 and end > 0 else np.full(length, start)
    if "vibrato" in layer:
        depth, speed = layer["vibrato"]
        frequency = frequency * (1.0 + float(depth) * np.sin(2 * np.pi * float(speed) * t))
    phase = 2 * np.pi * np.cumsum(frequency) / rate
    signal = oscillator(layer.get("wave", "sine"), phase)
    for harmonic, gain in layer.get("harmonics", []):
        signal = signal + float(gain) * oscillator(layer.get("wave", "sine"), phase * float(harmonic))
    return signal


def clicks(rng, length, rate, layer):
    out = np.zeros(length)
    period = 1.0 / float(layer["rate"])
    jitter = float(layer.get("jitter", 0.0))
    burst = int(float(layer.get("click_s", 0.01)) * rate)
    decay = np.exp(-np.arange(burst) / max(burst / 5.0, 1.0))
    position = 0.0
    while True:
        start = int((position + rng.uniform(-jitter, jitter) * period) * rate)
        if start >= length:
            break
        if start >= 0:
            chunk = rng.standard_normal(burst) * decay * rng.uniform(0.5, 1.0)
            stop = min(start + burst, length)
            out[start:stop] += chunk[:stop - start]
        position += period
    return out


def sample(rng, length, rate, layer, sources, cache):
    key = layer["source"]
    if key not in cache:
        data, source_rate = soundfile.read(sources[key]["path"], always_2d=True)
        cache[key] = (data.mean(axis=1), source_rate)
    data, source_rate = cache[key]
    pitch = float(layer.get("pitch", 1.0))
    start = int(float(layer.get("start", 0.0)) * source_rate)
    step = source_rate / rate * pitch
    positions = start + np.arange(length) * step
    positions = positions[positions < len(data) - 1]
    signal = np.interp(positions, np.arange(len(data)), data)
    out = np.zeros(length)
    out[:len(signal)] = signal
    peak = np.max(np.abs(out)) or 1.0
    return out / peak


def verify_sources(sources):
    for key, source in sources.items():
        digest = hashlib.sha256(Path(source["path"]).read_bytes()).hexdigest()
        if digest != source["sha256"]:
            raise SystemExit(f"source {key} changed: {source['path']}")


def render_layer(rng, length, rate, layer, sources, cache):
    kind = layer["type"]
    offset = int(float(layer.get("at", 0.0)) * rate)
    span = length - offset
    if span <= 0:
        return np.zeros(length)
    if kind == "noise":
        signal = coloured_noise(rng, span, layer.get("colour", "white"))
    elif kind == "tone":
        signal = tone(rng, span, rate, layer)
    elif kind == "clicks":
        signal = clicks(rng, span, rate, layer)
    elif kind == "sample":
        signal = sample(rng, span, rate, layer, sources, cache)
    else:
        raise ValueError(f"unknown layer type {kind}")
    signal = filtered(signal, rate, layer)
    if layer.get("envelope", True):
        signal = signal * envelope(span, rate, layer)
    if "tremolo" in layer:
        depth, speed = layer["tremolo"]
        t = np.arange(span) / rate
        signal = signal * (1.0 - float(depth) * 0.5 * (1.0 + np.sin(2 * np.pi * float(speed) * t)))
    signal = signal * db(float(layer.get("gain_db", 0.0)))
    out = np.zeros(length)
    repeat = layer.get("repeat", {"count": 1, "every_s": 0.0})
    for index in range(int(repeat["count"])):
        start = offset + int(index * float(repeat["every_s"]) * rate)
        if start >= length:
            break
        stop = min(length, start + len(signal))
        out[start:stop] += signal[:stop - start]
    return out


def room_impulse(rate, room):
    decay = max(float(room["decay_s"]), 1e-3)
    rng = np.random.default_rng(int(room.get("seed", 7)))
    t = np.arange(int(decay * rate)) / rate
    impulse = rng.standard_normal(len(t)) * np.exp(-6.9 * t / decay)
    impulse[:int(float(room.get("predelay_s", 0.012)) * rate)] = 0.0
    energy = np.sqrt(np.sum(impulse ** 2))
    return impulse / energy if energy > 0 else impulse


def with_room(signal, rate, room):
    wet = fftconvolve(signal, room_impulse(rate, room))[:len(signal)]
    return signal + wet * db(float(room["wet_db"]))


def seamless(signal, rate, crossfade_s):
    fade = int(crossfade_s * rate)
    if fade <= 0 or fade * 2 >= len(signal):
        return signal
    head, body, tail = signal[:fade], signal[fade:-fade], signal[-fade:]
    ramp = np.linspace(0.0, 1.0, fade)
    joined = tail * (1.0 - ramp) + head * ramp
    return np.concatenate([body, joined])


def render_channel(sound_id, spec, sources, rate, channel):
    suffix = f":{channel}" if channel else ""
    seed = int(hashlib.sha256(f"{sound_id}:{spec.get('seed', 0)}{suffix}".encode()).hexdigest()[:8], 16)
    rng = np.random.default_rng(seed)
    loop = bool(spec.get("loop", False))
    crossfade = float(spec.get("crossfade_s", 0.5)) if loop else 0.0
    length = int((float(spec["seconds"]) + crossfade) * rate)
    shift = float(spec.get("stereo_offset_s", 0.0)) * channel
    cache = {}
    mix = np.zeros(length)
    for layer in spec.get("layers", []):
        if shift and layer["type"] == "sample":
            layer = dict(layer, start=float(layer.get("start", 0.0)) + shift)
        mix += render_layer(rng, length, rate, layer, sources, cache)
    if "room" in spec:
        mix = with_room(mix, rate, spec["room"])
    mix = filtered(mix, rate, spec.get("mix_filter", DEFAULT_MIX_FILTER))
    if loop:
        mix = seamless(mix, rate, crossfade)
    return mix


def render(sound_id, spec, sources, rate):
    if spec.get("stereo", False):
        mix = np.stack([render_channel(sound_id, spec, sources, rate, channel) for channel in (0, 1)], axis=1)
    else:
        mix = render_channel(sound_id, spec, sources, rate, 0)
    peak = np.max(np.abs(mix))
    if peak > 0 and not spec.get("silent", False):
        mix = mix / peak * db(float(spec.get("peak_dbfs", PEAK_DBFS)))
    else:
        mix = np.zeros_like(mix)
    return mix


def write(path, signal, rate, channels):
    path.parent.mkdir(parents=True, exist_ok=True)
    data = signal if signal.ndim == 2 else (np.repeat(signal[:, None], channels, axis=1) if channels > 1 else signal)
    soundfile.write(str(path), data, rate, subtype="PCM_16")


def main(argv=None):
    parser = argparse.ArgumentParser(description="Render game sounds from a JSON spec: seeded noise, tones, click trains and CC0 sample layers, peak-normalised, loops crossfaded seamless.")
    parser.add_argument("--spec", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--only", nargs="*", default=None)
    args = parser.parse_args(argv)
    spec = json.loads(args.spec.read_text(encoding="utf-8"))
    rate = int(spec.get("sample_rate", 48000))
    sources = spec.get("sources", {})
    verify_sources(sources)
    for sound_id, sound in sorted(spec["sounds"].items()):
        if args.only and sound_id not in args.only:
            continue
        signal = render(sound_id, sound, sources, rate)
        path = args.out / f"{sound_id}.wav"
        write(path, signal, rate, int(sound.get("channels", 1)))
        print(f"{sound_id}: {len(signal) / rate:.2f} s -> {path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
