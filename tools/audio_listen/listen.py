import argparse
import json
import math
from pathlib import Path

import numpy as np
import soundfile as sf
from scipy import signal

BANDS = [("sub", 20, 80), ("low", 80, 300), ("mid", 300, 2000), ("presence", 2000, 6000), ("air", 6000, 20000)]
MAINS = [50, 60, 100, 120, 150, 180]


def parse_args():
    parser = argparse.ArgumentParser(description="Describe what an audio file sounds like in text a reader can act on without hearing it: loudness over time, tone colour, steadiness, transients and what band they hit, hums and whistles, possible voices, stereo width, clipping, and loop seams. Optional PNG sheet with spectrogram and curves.")
    parser.add_argument("files", nargs="+", type=Path)
    parser.add_argument("--segments", type=int, default=12, help="how many time segments to describe")
    parser.add_argument("--event-db", type=float, default=10.0, help="rise over the recent background that counts as a transient")
    parser.add_argument("--max-events", type=int, default=15)
    parser.add_argument("--loop-search", nargs=2, type=float, metavar=("MIN_S", "MAX_S"), help="find the best loop regions of this length range")
    parser.add_argument("--png", type=Path, help="folder to write one PNG sheet per file")
    parser.add_argument("--json", type=Path, help="write the full report as JSON")
    parser.add_argument("--marks", type=Path, help="JSON list of {time_s, name} drawn on the sheet as one coloured line per name")
    return parser.parse_args()


def biquad(kind, fc, rate, q, gain_db=0.0):
    a = 10 ** (gain_db / 40)
    w0 = 2 * math.pi * fc / rate
    alpha = math.sin(w0) / (2 * q)
    cos = math.cos(w0)
    if kind == "shelf":
        root = 2 * math.sqrt(a) * alpha
        b = [a * ((a + 1) + (a - 1) * cos + root), -2 * a * ((a - 1) + (a + 1) * cos), a * ((a + 1) + (a - 1) * cos - root)]
        den = [(a + 1) - (a - 1) * cos + root, 2 * ((a - 1) - (a + 1) * cos), (a + 1) - (a - 1) * cos - root]
    else:
        b = [(1 + cos) / 2, -(1 + cos), (1 + cos) / 2]
        den = [1 + alpha, -2 * cos, 1 - alpha]
    return np.array(b) / den[0], np.array(den) / den[0]


def k_weight(audio, rate):
    for kind, fc, q, gain in (("shelf", 1681.97, 0.7072, 4.0), ("pass", 38.14, 0.5003, 0.0)):
        b, a = biquad(kind, fc, rate, q, gain)
        audio = signal.lfilter(b, a, audio, axis=0)
    return audio


def block_loudness(audio, rate, window_s, hop_s):
    weighted = k_weight(audio, rate) ** 2
    window, hop = int(window_s * rate), max(1, int(hop_s * rate))
    if len(weighted) < window:
        return np.array([]), np.array([])
    starts = np.arange(0, len(weighted) - window + 1, hop)
    cumulative = np.vstack([np.zeros((1, weighted.shape[1])), np.cumsum(weighted, axis=0)])
    power = ((cumulative[starts + window] - cumulative[starts]) / window).sum(axis=1)
    return starts / rate, -0.691 + 10 * np.log10(np.maximum(power, 1e-12))


def integrated(audio, rate):
    _, blocks = block_loudness(audio, rate, 0.4, 0.1)
    blocks = blocks[blocks > -70]
    if len(blocks) == 0:
        return float("-inf")
    relative = 10 * np.log10(np.mean(10 ** (blocks / 10))) - 10
    gated = blocks[blocks > relative]
    return float(10 * np.log10(np.mean(10 ** (gated / 10))))


def true_peak(audio):
    up = signal.resample_poly(audio, 4, 1, axis=0)
    return float(20 * np.log10(max(np.max(np.abs(up)), 1e-12)))


def spectrum_frames(mono, rate):
    freqs, times, z = signal.stft(mono, rate, nperseg=2048, noverlap=1536, boundary=None)
    return freqs, times, np.abs(z) ** 2


def band_shares(freqs, power):
    total = power.sum(axis=0) + 1e-20
    return {name: power[(freqs >= low) & (freqs < high)].sum(axis=0) / total for name, low, high in BANDS}


def centroid(freqs, power):
    return (freqs[:, None] * power).sum(axis=0) / (power.sum(axis=0) + 1e-20)


def flatness(power, freqs):
    floor = power[(freqs >= 100) & (freqs <= 8000)] + 1e-20
    return np.exp(np.mean(np.log(floor), axis=0)) / np.mean(floor, axis=0)


def colour(hz):
    if hz < 300:
        return "dark rumble"
    if hz < 1000:
        return "warm, low-mid"
    if hz < 3000:
        return "mid, present"
    if hz < 6000:
        return "bright"
    return "hissy, very bright"


def texture(flat, tonal):
    if flat > 0.3:
        return "noise-like"
    if flat > 0.1:
        return "noisy with some pitch" if tonal else "coloured noise"
    return "pitched or tonal" if tonal else "narrow-band noise (whoosh or roar)"


def steadiness(spread_db):
    if spread_db < 1.5:
        return "steady"
    if spread_db < 4:
        return "gently moving"
    if spread_db < 8:
        return "gusting"
    return "very uneven"


def frame_levels(mono, rate, hop_s=0.01):
    hop = int(hop_s * rate)
    count = len(mono) // hop
    rms = np.sqrt(np.mean(mono[: count * hop].reshape(count, hop) ** 2, axis=1) + 1e-20)
    return 20 * np.log10(rms)


def band_of(mono, rate, start_s):
    piece = mono[int(start_s * rate): int((start_s + 0.05) * rate)]
    if len(piece) < 64:
        return "unknown"
    freqs, power = signal.periodogram(piece, rate)
    energies = {name: power[(freqs >= low) & (freqs < high)].sum() for name, low, high in BANDS}
    return max(energies, key=energies.get)


def events(mono, rate, rise_db, limit):
    levels = frame_levels(mono, rate)
    background_frames = 30
    found = []
    index = background_frames
    floor = np.percentile(levels, 10)
    while index < len(levels):
        background = np.median(levels[index - background_frames:index])
        if levels[index] - background >= rise_db and levels[index] > floor + rise_db:
            peak = index + int(np.argmax(levels[index:index + 10]))
            end = peak
            while end < len(levels) - 1 and levels[end] > background + 3:
                end += 1
            start_s = index * 0.01
            band = band_of(mono, rate, start_s)
            found.append({"time_s": round(start_s, 2), "rise_db": round(float(levels[peak] - background), 1), "length_ms": int((end - index) * 10), "band": band, "reads_as": describe_event(band, (end - index) * 10)})
            index = end + 1
        else:
            index += 1
    found.sort(key=lambda event: -event["rise_db"])
    return sorted(found[:limit], key=lambda event: event["time_s"]), len(found)


def describe_event(band, length_ms):
    if band in ("sub", "low"):
        return "thump or buffet (mic handling or wind on the capsule if it is in a bed)" if length_ms < 400 else "low swell (wave, gust or vehicle)"
    if band == "mid":
        return "knock, step or voice-band burst" if length_ms < 400 else "mid-band swell"
    return "click, tick or crack" if length_ms < 150 else "hiss burst (spray, gust in grass, rain shower)"


def tones(freqs, power):
    average = np.median(power, axis=1)
    log = 10 * np.log10(average + 1e-20)
    smooth = signal.medfilt(log, 31)
    peaks, props = signal.find_peaks(log - smooth, height=10, distance=5)
    found = []
    for peak, height in sorted(zip(peaks, props["peak_heights"]), key=lambda pair: -pair[1])[:6]:
        hz = float(freqs[peak])
        if hz < 20:
            continue
        mains = any(abs(hz - m) < 3 for m in MAINS)
        found.append({"hz": round(hz, 1), "above_noise_db": round(float(height), 1), "reads_as": "mains hum" if mains else ("low drone" if hz < 200 else ("whistle or tone" if hz > 1000 else "tonal component"))})
    return found


def voice_likeness(mono, rate):
    b, a = signal.butter(4, [300, 3400], btype="band", fs=rate)
    band = signal.filtfilt(b, a, mono)
    hop = rate // 100
    count = len(band) // hop
    if count < 400:
        return None
    envelope = np.sqrt(np.mean(band[: count * hop].reshape(count, hop) ** 2, axis=1))
    envelope = envelope - envelope.mean()
    freqs, power = signal.welch(envelope, 100, nperseg=min(512, count))
    syllable = power[(freqs >= 3) & (freqs <= 7)].sum()
    total = power[(freqs >= 0.5) & (freqs <= 20)].sum() + 1e-20
    return round(float(syllable / total), 2)


def stereo(audio, rate):
    if audio.shape[1] < 2:
        return None
    folded = audio.mean(axis=1, keepdims=True)
    fold_loss = integrated(audio, rate) - (integrated(folded, rate) + 3.01)
    left, right = audio[:, 0], audio[:, 1]
    correlation = float(np.corrcoef(left, right)[0, 1]) if np.std(left) > 0 and np.std(right) > 0 else 1.0
    side = np.sqrt(np.mean((left - right) ** 2)) / (np.sqrt(np.mean((left + right) ** 2)) + 1e-20)
    words = "near mono" if correlation > 0.9 else ("moderate width" if correlation > 0.5 else ("wide" if correlation > 0.0 else "very wide or phase-cancelling in mono"))
    return {"correlation": round(correlation, 2), "side_to_mid": round(float(side), 2), "mono_fold_loss_db": round(float(fold_loss), 1), "reads_as": words}


def segments(mono, audio, rate, freqs, times, power, count, tonal):
    duration = len(mono) / rate
    edges = np.linspace(0, duration, count + 1)
    block_times, blocks = block_loudness(audio, rate, 0.4, 0.1)
    shares = band_shares(freqs, power)
    centre = centroid(freqs, power)
    flat = flatness(power, freqs)
    rows = []
    for start, end in zip(edges[:-1], edges[1:]):
        in_blocks = (block_times >= start) & (block_times < end)
        in_frames = (times >= start) & (times < end)
        if not in_blocks.any() or not in_frames.any():
            continue
        level = blocks[in_blocks]
        hz = float(np.median(centre[in_frames]))
        noise = float(np.median(flat[in_frames]))
        spread = float(np.percentile(level, 90) - np.percentile(level, 10))
        band = {name: round(float(np.mean(share[in_frames])) * 100) for name, share in shares.items()}
        rows.append({"from_s": round(float(start), 1), "to_s": round(float(end), 1), "loudness_lufs": round(float(10 * np.log10(np.mean(10 ** (level / 10)))), 1), "swing_db": round(spread, 1), "centroid_hz": round(hz), "bands_pct": band, "reads_as": f"{steadiness(spread)}, {colour(hz)}, {texture(noise, tonal)}"})
    return rows


def seam(mono, rate, start_s, end_s):
    window = int(0.2 * rate)
    head = mono[int(start_s * rate): int(start_s * rate) + window]
    tail = mono[int(end_s * rate) - window: int(end_s * rate)]
    if len(head) < window or len(tail) < window:
        return None
    level = abs(20 * np.log10((np.sqrt(np.mean(head ** 2)) + 1e-12) / (np.sqrt(np.mean(tail ** 2)) + 1e-12)))
    shape = float(np.mean(np.abs(third_octaves(head, rate) - third_octaves(tail, rate))))
    return {"level_jump_db": round(float(level), 1), "spectral_jump_db": round(shape, 1)}


def third_octaves(piece, rate):
    power = np.abs(np.fft.rfft(piece * np.hanning(len(piece)))) ** 2
    freqs = np.fft.rfftfreq(len(piece), 1 / rate)
    centres = 1000 * 2 ** (np.arange(-17, 13) / 3)
    centres = centres[centres < rate / 2.3]
    levels = [power[(freqs >= c / 2 ** (1 / 6)) & (freqs < c * 2 ** (1 / 6))].sum() for c in centres]
    total = sum(levels) + 1e-20
    return 10 * np.log10(np.array(levels) / total + 1e-9)


def loop_search(mono, rate, min_s, max_s):
    duration = len(mono) / rate
    grid = np.arange(0.2, duration - 0.2, 0.25)
    candidates = []
    for start in grid:
        for end in grid:
            if min_s <= end - start <= max_s:
                result = seam(mono, rate, start, end)
                if result:
                    candidates.append((result["level_jump_db"] + result["spectral_jump_db"] * 0.5, float(start), float(end), result))
    candidates.sort(key=lambda row: row[0])
    picked = []
    for score, start, end, result in candidates:
        if all(abs(start - other["start_s"]) > 2 or abs(end - other["end_s"]) > 2 for other in picked):
            picked.append({"start_s": round(start, 2), "end_s": round(end, 2), **result})
        if len(picked) == 3:
            break
    return picked


def summary(report):
    lines = []
    lines.append(f"{report['file']}: {report['duration_s']} s, {report['rate']} Hz, {report['channels']} ch, {report['subtype']}")
    lines.append(f"  loudness {report['integrated_lufs']} LUFS integrated, true peak {report['true_peak_dbtp']} dBTP, short-term range {report['short_term_range_db']} dB (10th to 95th percentile)")
    if report["clipped_samples"]:
        lines.append(f"  CLIPPING: {report['clipped_samples']} samples at full scale")
    if abs(report["dc_offset"]) > 0.005:
        lines.append(f"  DC offset {report['dc_offset']}")
    lines.append(f"  overall: {report['overall']['reads_as']}; centroid {report['overall']['centroid_hz']} Hz; bands % {report['overall']['bands_pct']}")
    if report["overall"]["bands_pct"]["sub"] >= 40:
        lines.append("  sub-80 Hz carries most of the energy: likely wind on the microphone or traffic rumble; a high-pass near 80 Hz would clean it")
    if report["stereo"]:
        lines.append(f"  stereo: {report['stereo']['reads_as']} (correlation {report['stereo']['correlation']}); folded to mono it loses {report['stereo']['mono_fold_loss_db']} dB against identical channels" + (", so it thins out on a mono speaker or a mono-folded 3D source" if report['stereo']['mono_fold_loss_db'] > 3.5 else ""))
    if report["voice_likeness"] is not None:
        verdict = "rhythm at syllable rate in the voice band: possible voices, listen to check" if report["voice_likeness"] >= 0.35 else "no syllable-rate rhythm in the voice band"
        lines.append(f"  voices: {verdict} (score {report['voice_likeness']}, heuristic)")
    for tone in report["tones"]:
        lines.append(f"  tone {tone['hz']} Hz, {tone['above_noise_db']} dB above the noise: {tone['reads_as']}")
    lines.append(f"  transients: {report['event_count']} found over {report['event_rise_db']} dB, loudest {len(report['events'])}:")
    for event in report["events"]:
        lines.append(f"    {event['time_s']:7.2f} s  +{event['rise_db']} dB  {event['length_ms']} ms  {event['band']}: {event['reads_as']}")
    lines.append("  timeline:")
    for row in report["segments"]:
        lines.append(f"    {row['from_s']:6.1f}-{row['to_s']:6.1f} s  {row['loudness_lufs']:6.1f} LUFS  swing {row['swing_db']:4.1f} dB  {row['centroid_hz']:5d} Hz  {row['reads_as']}")
    if report.get("whole_file_seam"):
        lines.append(f"  looping the whole file: level jump {report['whole_file_seam']['level_jump_db']} dB, spectral jump {report['whole_file_seam']['spectral_jump_db']} dB at the seam")
    for loop in report.get("loops", []):
        lines.append(f"  loop candidate {loop['start_s']} to {loop['end_s']} s: level jump {loop['level_jump_db']} dB, spectral jump {loop['spectral_jump_db']} dB")
    return "\n".join(lines)


def mark_colours(marks):
    import matplotlib
    names = sorted({mark["name"] for mark in marks})
    palette = matplotlib.colormaps["tab20"]
    return {name: matplotlib.colors.to_hex(palette(index % 20)) for index, name in enumerate(names)}


def sheet(report, audio, rate, freqs, times, power, folder, marks=()):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    folder.mkdir(parents=True, exist_ok=True)
    figure, axes = plt.subplots(3, 1, figsize=(14, 9), sharex=True, gridspec_kw={"height_ratios": [3, 1, 1]})
    visible = freqs > 20
    axes[0].pcolormesh(times, freqs[visible], 10 * np.log10(power[visible] + 1e-12), shading="auto", cmap="magma")
    axes[0].set_yscale("log")
    axes[0].set_ylabel("Hz")
    axes[0].set_title(f"{Path(report['file']).name}  {report['integrated_lufs']} LUFS  {report['true_peak_dbtp']} dBTP")
    block_times, blocks = block_loudness(audio, rate, 3.0, 0.1)
    axes[1].plot(block_times + 1.5, blocks, color="tab:blue")
    axes[1].set_ylabel("short-term LUFS")
    axes[2].plot(times, centroid(freqs, power), color="tab:green", linewidth=0.6)
    axes[2].set_yscale("log")
    axes[2].set_ylabel("centroid Hz")
    axes[2].set_xlabel("s")
    for event in report["events"]:
        for axis in axes:
            axis.axvline(event["time_s"], color="cyan", linewidth=0.6, alpha=0.7)
    colours = mark_colours(marks)
    for mark in marks:
        for axis in axes:
            axis.axvline(mark["time_s"], color=colours[mark["name"]], linewidth=1.2, linestyle="--", label=mark["name"] if axis is axes[0] else None)
    if colours:
        handles, labels = axes[0].get_legend_handles_labels()
        unique = dict(zip(labels, handles))
        axes[0].legend(unique.values(), unique.keys(), loc="upper left", bbox_to_anchor=(1.0, 1.0), fontsize=7)
    figure.tight_layout()
    out = folder / (Path(report["file"]).stem + ".png")
    figure.savefig(out, dpi=90)
    plt.close(figure)
    return str(out)


def analyse(path, args):
    audio, rate = sf.read(str(path), always_2d=True)
    info = sf.info(str(path))
    mono = audio.mean(axis=1)
    freqs, times, power = spectrum_frames(mono, rate)
    _, short_term = block_loudness(audio, rate, 3.0, 0.1)
    short_term = short_term[short_term > -70]
    shares = band_shares(freqs, power)
    overall_spread = float(np.percentile(short_term, 90) - np.percentile(short_term, 10)) if len(short_term) else 0.0
    overall_centroid = float(np.median(centroid(freqs, power)))
    found, total = events(mono, rate, args.event_db, args.max_events)
    tone_list = tones(freqs, power)
    tonal = any(tone["above_noise_db"] >= 15 for tone in tone_list)
    report = {
        "file": str(path),
        "duration_s": round(len(audio) / rate, 2),
        "rate": rate,
        "channels": audio.shape[1],
        "subtype": info.subtype,
        "integrated_lufs": round(integrated(audio, rate), 1),
        "true_peak_dbtp": round(true_peak(audio), 1),
        "short_term_range_db": round(float(np.percentile(short_term, 95) - np.percentile(short_term, 10)), 1) if len(short_term) else 0.0,
        "clipped_samples": int(np.sum(np.abs(audio) >= 0.9999)),
        "dc_offset": round(float(np.mean(mono)), 4),
        "overall": {"centroid_hz": round(overall_centroid), "bands_pct": {name: round(float(np.mean(share)) * 100) for name, share in shares.items()}, "reads_as": f"{steadiness(overall_spread)}, {colour(overall_centroid)}, {texture(float(np.median(flatness(power, freqs))), tonal)}"},
        "stereo": stereo(audio, rate),
        "voice_likeness": voice_likeness(mono, rate),
        "tones": tone_list,
        "events": found,
        "event_count": total,
        "event_rise_db": args.event_db,
        "segments": segments(mono, audio, rate, freqs, times, power, args.segments, tonal),
        "whole_file_seam": seam(mono, rate, 0.0, len(mono) / rate),
    }
    if args.loop_search:
        report["loops"] = loop_search(mono, rate, *args.loop_search)
    marks = json.loads(args.marks.read_text(encoding="utf-8")) if getattr(args, "marks", None) else []
    if marks:
        report["marks"] = marks
        report["mark_colours"] = mark_colours(marks)
    if args.png:
        report["png"] = sheet(report, audio, rate, freqs, times, power, args.png, marks)
    return report


def main():
    args = parse_args()
    reports = []
    for path in args.files:
        report = analyse(path, args)
        reports.append(report)
        print(summary(report))
        if report.get("png"):
            print(f"  sheet {report['png']}")
        print()
    if args.json:
        args.json.write_text(json.dumps(reports, indent=1), encoding="utf-8")


if __name__ == "__main__":
    main()
