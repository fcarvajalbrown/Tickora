import argparse
import json
import sys

REQUIRED = ("winding_consistent",)
ZERO = ("open_edges", "degenerate_triangles")


def failures(report):
    found = [key for key in REQUIRED if not report.get(key, False)]
    found += [key for key in ZERO if report.get(key, 0)]
    return found


def main(argv=None):
    parser = argparse.ArgumentParser(description="Gate check_mesh.py JSON lines read from stdin: every mesh needs consistent winding, no open edges and no degenerate triangles. Non-manifold edges where parts touch and vertices off the grid on round parts are reported, not failed.")
    parser.parse_args(argv)
    failed = 0
    for line in sys.stdin:
        if not line.strip():
            continue
        report = json.loads(line)
        bad = failures(report)
        failed += bool(bad)
        status = "FAIL " + ",".join(bad) if bad else "ok"
        print(f"{report['file']:28s} {report['triangles']:6d} tris  nm {report['non_manifold_edges']:4d}  off-grid {report['vertices_off_grid']:4d}  {status}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
