#!/usr/bin/env python3
"""am62p-photo-manifest: per-scene photograph lines for the M3 AM62P report.

  am62p-photo-manifest.py --photos DIR --verdicts DIR [--link-prefix PREFIX]
                          [--require-pass]

DIR (photos) holds sceneN.jpg, sceneN.jpeg or sceneN.png for N = 1..8.
DIR (verdicts) holds sceneN.txt (the saved stdout of scripts/camera-check.py;
its last "VERDICT: PASS|FAIL" line counts) or sceneN.json (the saved
camera-check.py --json output; its "pass" field counts).
PREFIX is the relative path from the report to the photo directory; default
is the --photos value if that is relative. Absolute prefixes are refused so
that no host path reaches the report.

Prints a markdown section on stdout, one line per scene, in the form that
scripts/check-m3-report.sh accepts:
  - Scene N: camera-check PASS - [sceneN.jpg](PREFIX/sceneN.jpg) sha256 HEX
Never reads picture metadata, copies files, or runs the camera.
Exit: 0 ok, 1 a photo or verdict is missing/ambiguous (nothing on stdout)
or, with --require-pass, a verdict is FAIL, 2 usage error.
"""
import hashlib
import json
import os
import re
import sys

SCENES = range(1, 9)
EXTS = (".jpg", ".jpeg", ".png")
VERDICT_RE = re.compile(r"^VERDICT:\s*(PASS|FAIL)\s*$")


def find_photo(d, n):
    hits = [f for f in (("scene%d" % n) + e for e in EXTS)
            if os.path.isfile(os.path.join(d, f))]
    if not hits:
        raise ValueError("scene %d: no photograph (scene%d.jpg|jpeg|png)" % (n, n))
    if len(hits) > 1:
        raise ValueError("scene %d: more than one photograph: %s" % (n, ", ".join(hits)))
    return hits[0]


def read_verdict(d, n):
    txt = os.path.join(d, "scene%d.txt" % n)
    js = os.path.join(d, "scene%d.json" % n)
    have = [p for p in (txt, js) if os.path.isfile(p)]
    if not have:
        raise ValueError("scene %d: no camera-check output (scene%d.txt|json)" % (n, n))
    if len(have) > 1:
        raise ValueError("scene %d: both scene%d.txt and .json present" % (n, n))
    p = have[0]
    try:
        with open(p, encoding="utf-8", errors="replace") as f:
            text = f.read()
    except OSError as e:
        raise ValueError("scene %d: cannot read %s: %s" % (n, os.path.basename(p), e.strerror))
    if p == js:
        try:
            v = json.loads(text)["pass"]
        except (ValueError, KeyError, TypeError):
            raise ValueError("scene %d: scene%d.json has no \"pass\" field" % (n, n))
        if not isinstance(v, bool):
            raise ValueError("scene %d: \"pass\" is not true/false" % n)
        return "PASS" if v else "FAIL"
    last = None
    for line in text.splitlines():
        m = VERDICT_RE.match(line.strip())
        if m:
            last = m.group(1)
    if last is None:
        raise ValueError("scene %d: no 'VERDICT: PASS|FAIL' line in scene%d.txt" % (n, n))
    return last


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for blk in iter(lambda: f.read(65536), b""):
            h.update(blk)
    return h.hexdigest()


def main(argv):
    photos = verdicts = prefix = None
    req = False
    it = iter(argv[1:])
    try:
        for a in it:
            if a == "--photos":
                photos = next(it)
            elif a == "--verdicts":
                verdicts = next(it)
            elif a == "--link-prefix":
                prefix = next(it)
            elif a == "--require-pass":
                req = True
            elif a in ("-h", "--help"):
                print(__doc__)
                return 0
            else:
                raise StopIteration
    except StopIteration:
        pass
    else:
        if photos and verdicts:
            if prefix is None:
                prefix = photos
            if os.path.isabs(prefix) or re.match(r"^[A-Za-z][A-Za-z0-9+.-]*:", prefix):
                print("error: link prefix must be a relative path", file=sys.stderr)
                return 2
            prefix = prefix.rstrip("/")
            lines, errors = [], []
            for n in SCENES:
                try:
                    f = find_photo(photos, n)
                    v = read_verdict(verdicts, n)
                    digest = sha256(os.path.join(photos, f))
                except (ValueError, OSError) as e:
                    errors.append(str(e))
                    continue
                if req and v != "PASS":
                    errors.append("scene %d: camera-check verdict is %s" % (n, v))
                link = "%s/%s" % (prefix, f) if prefix else f
                lines.append("- Scene %d: camera-check %s - [%s](%s) sha256 %s"
                             % (n, v, f, link, digest))
            if errors:
                for e in errors:
                    print("am62p-photo-manifest: " + e, file=sys.stderr)
                return 1
            print("## Photographs and camera-check\n")
            print("\n".join(lines))
            return 0
    print("usage: am62p-photo-manifest.py --photos DIR --verdicts DIR "
          "[--link-prefix PREFIX] [--require-pass]", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
