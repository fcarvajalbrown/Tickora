import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SCENARIOS = ["arena.json", "lighting.json"]


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
    args = parser.parse_args(argv)
    if args.command == "record":
        return record(args.out, args.adapter, args.binary, args.scenarios)
    report = compare(args.before, args.after)
    for key in ["changed", "only_before", "only_after"]:
        for name in report[key]:
            print(f"{key}: {name}")
    print(
        f"same {len(report['same'])}, changed {len(report['changed'])}, "
        f"only before {len(report['only_before'])}, only after {len(report['only_after'])}"
    )
    failed = report["changed"] or report["only_before"] or (report["only_after"] and not args.allow_new)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
