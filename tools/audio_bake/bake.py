import argparse
import hashlib
import json
import os
import sys
from math import gcd
from pathlib import Path

import numpy as np
import soundfile
from scipy.signal import resample_poly

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "sfx_synth"))
import synth

CLIP_DBFS = 0.0


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description="Bake game sounds from a JSON spec: take each source from a fetcher manifest (SHA-256 checked) or from synth.py, cut its range with short fades, fold to mono, resample, level its loudest momentary window to the reference loudness with Keel's BS.1770-4 meter, and write 16-bit WAVs plus a manifest.")
    parser.add_argument("--spec", type=Path, default=ROOT / "specs" / "audio" / "bake.json")
    parser.add_argument("--out", type=Path, default=ROOT / "assets" / "gen" / "audio")
    parser.add_argument("--keel", type=Path, default=Path(os.environ.get("KEEL", r"C:\Projects\Keel")))
    parser.add_argument("--only", nargs="*", default=None)
    return parser.parse_args(argv)


def load_meters(keel):
    sys.path.insert(0, str(keel))
    import meters
    return meters


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def manifest_source(source, root):
    rows = json.loads((root / source["manifest"]).read_text(encoding="utf-8"))
    row = next((row for row in rows if str(row["id"]) == str(source["id"])), None)
    if row is None:
        raise SystemExit(f"{source['manifest']} has no record {source['id']}")
    path = root / source["folder"] / row["file"]
    if sha256(path.read_bytes()) != row["sha256"]:
        raise SystemExit(f"{path} does not match the SHA-256 in {source['manifest']}")
    audio, rate = soundfile.read(str(path), always_2d=True)
    return audio, rate, {"manifest": source["manifest"], "id": str(source["id"]), "sha256": row["sha256"]}


def synth_source(source, root, rate):
    spec = json.loads((root / source["synth"]).read_text(encoding="utf-8"))
    sound = spec["sounds"][source["id"]]
    sources = {key: dict(value, path=str(root / value["path"])) for key, value in spec.get("sources", {}).items()}
    synth.verify_sources(sources)
    signal = synth.render(source["id"], sound, sources, rate)
    return signal.reshape(len(signal), -1), rate, {"synth": source["synth"], "id": source["id"]}


def read_source(source, root, rate):
    if "synth" in source:
        return synth_source(source, root, rate)
    return manifest_source(source, root)


def cut(audio, rate, start, end):
    first = int(round(start * rate))
    last = len(audio) if end is None else int(round(end * rate))
    if not 0 <= first < last <= len(audio):
        raise SystemExit(f"range {start} to {end} s is outside a {len(audio) / rate:.3f} s source")
    return audio[first:last]


def fade(mono, rate, fade_in_s, fade_out_s):
    out = mono.copy()
    count_in = min(len(out), int(fade_in_s * rate))
    count_out = min(len(out), int(fade_out_s * rate))
    if count_in:
        out[:count_in] *= np.linspace(0.0, 1.0, count_in)
    if count_out:
        out[-count_out:] *= np.linspace(1.0, 0.0, count_out)
    return out


def resample(mono, rate, target):
    if rate == target:
        return mono
    divisor = gcd(rate, target)
    return resample_poly(mono, target // divisor, rate // divisor)


def momentary_max(meters, mono, rate, window_s, hop_s):
    window = int(window_s * rate)
    if len(mono) < window:
        mono = np.concatenate([mono, np.zeros(window - len(mono))])
    return float(meters.short_term_lufs_max(mono, rate, window_s=window_s, hop_s=hop_s))


def peak_dbfs(mono):
    peak = float(np.max(np.abs(mono))) if mono.size else 0.0
    return float(20.0 * np.log10(peak)) if peak > 0 else float("-inf")


def level(meters, mono, rate, spec, trim_db):
    before = momentary_max(meters, mono, rate, spec["momentary_window_s"], spec["momentary_hop_s"])
    if not np.isfinite(before):
        raise SystemExit("the source is silent")
    gain_db = spec["reference_lufs"] + trim_db - before
    levelled = mono * 10.0 ** (gain_db / 20.0)
    peak = peak_dbfs(levelled)
    if peak >= CLIP_DBFS:
        raise SystemExit(f"levelling by {gain_db:+.2f} dB would clip, sample peak {peak:+.2f} dBFS; lower its trim_db")
    after = momentary_max(meters, levelled, rate, spec["momentary_window_s"], spec["momentary_hop_s"])
    return levelled, {"lufs_before": round(before, 2), "gain_db": round(gain_db, 2), "lufs_after": round(after, 2), "peak_dbfs": round(peak, 2)}


def bake(name, sound, spec, meters, root, out):
    rate = spec["sample_rate"]
    audio, source_rate, record = read_source(sound["source"], root, rate)
    piece = cut(audio, source_rate, sound.get("start", 0.0), sound.get("end"))
    mono = resample(piece.mean(axis=1), source_rate, rate)
    mono = fade(mono, rate, sound.get("fade_in_s", spec["fade_in_s"]), sound.get("fade_out_s", spec["fade_out_s"]))
    levelled, measured = level(meters, mono, rate, spec, sound.get("trim_db", 0.0))
    path = out / f"{name}.wav"
    path.parent.mkdir(parents=True, exist_ok=True)
    soundfile.write(str(path), levelled, rate, subtype="PCM_16")
    return {"file": path.name, "source": record, "start": sound.get("start", 0.0), "end": sound.get("end"), "trim_db": sound.get("trim_db", 0.0), "seconds": round(len(levelled) / rate, 4), **measured, "sha256": sha256(path.read_bytes())}


def main(argv=None):
    args = parse_args(argv)
    spec = json.loads(args.spec.read_text(encoding="utf-8"))
    meters = load_meters(args.keel)
    manifest_path = args.out / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8")) if manifest_path.exists() else {}
    for name, sound in sorted(spec["sounds"].items()):
        if args.only and name not in args.only:
            continue
        manifest[name] = bake(name, sound, spec, meters, ROOT, args.out)
        row = manifest[name]
        print(f"{name}: {row['seconds']:.3f} s, {row['lufs_before']:+.2f} -> {row['lufs_after']:+.2f} LUFS ({row['gain_db']:+.2f} dB), peak {row['peak_dbfs']:+.2f} dBFS")
    manifest = {name: manifest[name] for name in sorted(manifest) if name in spec["sounds"]}
    manifest_path.write_text(json.dumps(manifest, indent=1, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
