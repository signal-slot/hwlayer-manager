#!/usr/bin/env python3
"""Re-run every camera check of the 2026-11-27 master-fd session.

    python3 docs/m1-imx95/run_checks.py

Runs scripts/camera-check.py on the committed rectified pictures with the
expectation files in checks/. Needs Pillow (JPEG input). Exit status 0 only
if every check passed.
"""
import glob
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
CHECK = os.path.join(HERE, "..", "..", "scripts", "camera-check.py")
PICS = os.path.join(HERE, "pictures")


def pic(state, n):
    (path,) = glob.glob(os.path.join(PICS, f"*-{state}-{n}-rect.jpg"))
    return path


def main():
    runs = [("two-planes", "two-planes.json", pic("two-planes", i), None) for i in (1, 2, 3)]
    for state in ("a-over-b", "b-over-a", "plane-alpha-50", "pixel-alpha"):
        runs += [(state, state + ".json", pic(state, i), None) for i in (1, 2, 3)]
    runs.append(("move", "move-1-2.json", pic("move", 1), pic("move", 2)))
    runs.append(("move", "move-2-3.json", pic("move", 2), pic("move", 3)))
    ok = True
    for name, exp, a, b in runs:
        title = f"== {name}: {os.path.basename(a)}" + (f" vs {os.path.basename(b)}" if b else "")
        print(title, flush=True)
        cmd = [sys.executable, "-I", CHECK, a, os.path.join(HERE, "checks", exp)]
        if b:
            cmd += ["--compare", b]
        ok &= subprocess.run(cmd).returncode == 0
    print("ALL:", "PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
