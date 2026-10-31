import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SCENARIOS = ["arena.json", "lighting.json", "lighting-second-room.pending.json"]


def record(out: Path, adapter: str, binary: Path, scenarios: list[str]) -> int:
    target = out / "scenarios"
    if target.exists():
        shutil.rmtree(target)
    target.mkdir(parents=True)
    for name in scenarios:
        shutil.copy2(ROOT / "scenarios" / name, target / name)
    env = dict(os.environ, TICKORA_ADAPTER=adapter)
    command = [str(binary), "scenario", "--record", *[str(target / name) for name in scenarios]]
    result = subprocess.run(command, env=env, capture_output=True, text=True)
    sys.stdout.write(result.stdout)
    sys.stderr.write(result.stderr)
    return result.returncode


def shots(base: Path) -> dict[str, bytes]:
    found = {}
    for path in sorted(base.rglob("*.png")):
        found[path.relative_to(base).as_posix()] = path.read_bytes()
    return found


def compare(before: Path, after: Path) -> dict[str, list[str]]:
    old = shots(before)
    new = shots(after)
    report = {"same": [], "changed": [], "only_before": [], "only_after": []}
    for name in sorted(set(old) | set(new)):
        if name not in new:
            report["only_before"].append(name)
        elif name not in old:
            report["only_after"].append(name)
        elif old[name] == new[name]:
            report["same"].append(name)
        else:
            report["changed"].append(name)
    return report


def rgb_pairs(before: Path, after: Path):
    from PIL import Image

    old = Image.open(before).convert("RGB")
    new = Image.open(after).convert("RGB")
    if old.size != new.size:
        raise SystemExit(f"{after} is {new.size}, {before} is {old.size}")
    a, b = old.tobytes(), new.tobytes()
    return old, new, [(a[i : i + 3], b[i : i + 3]) for i in range(0, len(a), 3)]


def pixel_stats(before: Path, after: Path) -> dict[str, int]:
    old, _, pairs = rgb_pairs(before, after)
    darker = brighter = changed = largest = 0
    for p0, p1 in pairs:
        delta = sum(p1) - sum(p0)
        if p0 != p1:
            changed += 1
            largest = max(largest, *(abs(x1 - x0) for x0, x1 in zip(p0, p1)))
        if delta < 0:
            darker += 1
        elif delta > 0:
            brighter += 1
    return {"changed": changed, "darker": darker, "brighter": brighter, "max_delta": largest, "pixels": old.width * old.height}


def diff_image(before: Path, after: Path, out: Path, gain: int) -> None:
    from PIL import Image

    old, new, pairs = rgb_pairs(before, after)
    marks = bytearray()
    for p0, p1 in pairs:
        delta = sum(p1) - sum(p0)
        level = min(255, abs(delta) * gain)
        marks += bytes((level, 0, 0) if delta < 0 else (0, level, 0))
    diff = Image.frombytes("RGB", old.size, bytes(marks))
    canvas = Image.new("RGB", (old.width * 3, old.height))
    for index, tile in enumerate([old, new, diff]):
        canvas.paste(tile, (index * old.width, 0))
    out.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(out)


def sheet(base: Path, prefixes: list[str], out: Path, columns: int) -> list[str]:
    from PIL import Image, ImageDraw

    picked = []
    for name in sorted(shots(base)):
        if "/" not in name:
            continue
        folder = name.split("/")[-2]
        if not prefixes or any(folder.startswith(prefix) for prefix in prefixes):
            picked.append((folder, base / name))
    if not picked:
        raise SystemExit(f"no shots under {base} start with {prefixes}")
    tiles = [(folder, Image.open(path).convert("RGB")) for folder, path in picked]
    width = max(tile.width for _, tile in tiles)
    height = max(tile.height for _, tile in tiles)
    label = 18
    rows = (len(tiles) + columns - 1) // columns
    canvas = Image.new("RGB", (columns * width, rows * (height + label)), (40, 40, 40))
    draw = ImageDraw.Draw(canvas)
    for index, (folder, tile) in enumerate(tiles):
        x = (index % columns) * width
        y = (index // columns) * (height + label)
        draw.text((x + 4, y + 3), folder, fill=(230, 230, 230))
        canvas.paste(tile, (x, y + label))
    out.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(out)
    return [folder for folder, _ in tiles]


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description="Record every scenario shot into a scratch copy on one adapter, or compare two recorded sets byte for byte."
    )
    commands = parser.add_subparsers(dest="command", required=True)
    rec = commands.add_parser("record", help="record shots into OUT/scenarios/goldens")
    rec.add_argument("--out", type=Path, required=True)
    rec.add_argument("--adapter", choices=["auto", "hardware", "software"], default="auto")
    rec.add_argument(
        "--binary",
        type=Path,
        default=ROOT / "target" / "verify" / "release" / ("tickora-mcp.exe" if os.name == "nt" else "tickora-mcp"),
    )
    rec.add_argument("scenarios", nargs="*", default=DEFAULT_SCENARIOS)
    cmp = commands.add_parser("compare", help="compare BEFORE and AFTER, exit 1 when any shared shot changed")
    cmp.add_argument("before", type=Path)
    cmp.add_argument("after", type=Path)
    cmp.add_argument("--allow-new", action="store_true", help="shots only in AFTER do not fail")
    cmp.add_argument("--diff-dir", type=Path, help="write BEFORE | AFTER | diff (red darker, green brighter) for each changed shot")
    cmp.add_argument("--gain", type=int, default=8, help="diff brightness per unit of summed RGB change")
    cmp.add_argument("--stats", action="store_true", help="for each changed shot print changed, darker and brighter pixel counts and the largest channel delta")
    sht = commands.add_parser("sheet", help="tile recorded shots whose folder starts with a prefix into one labelled PNG")
    sht.add_argument("base", type=Path)
    sht.add_argument("--out", type=Path, required=True)
    sht.add_argument("--columns", type=int, default=4)
    sht.add_argument("prefixes", nargs="*")
    args = parser.parse_args(argv)
    if args.command == "record":
        return record(args.out, args.adapter, args.binary, args.scenarios)
    if args.command == "sheet":
        for folder in sheet(args.base, args.prefixes, args.out, args.columns):
            print(folder)
        return 0
    report = compare(args.before, args.after)
    for key in ["changed", "only_before", "only_after"]:
        for name in report[key]:
            if key == "changed" and args.diff_dir:
                diff_image(args.before / name, args.after / name, args.diff_dir / (name.replace("/", "__")), args.gain)
            if key == "changed" and args.stats:
                stats = pixel_stats(args.before / name, args.after / name)
                print(
                    f"changed: {name} pixels {stats['changed']} of {stats['pixels']}, "
                    f"darker {stats['darker']}, brighter {stats['brighter']}, max delta {stats['max_delta']}"
                )
            else:
                print(f"{key}: {name}")
    print(
        f"same {len(report['same'])}, changed {len(report['changed'])}, "
        f"only before {len(report['only_before'])}, only after {len(report['only_after'])}"
    )
    failed = report["changed"] or report["only_before"] or (report["only_after"] and not args.allow_new)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
