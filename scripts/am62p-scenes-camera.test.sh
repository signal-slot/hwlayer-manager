#!/bin/sh
# Host self-test of scripts/am62p-scenes.py (expect) with scripts/camera-check.py.
# Run: sh scripts/am62p-scenes-camera.test.sh
#
# For each of the eight AM62P scenes it draws a synthetic PPM of what the scene
# should show (scene table and rectangle geometry of am62p-scenes-README.md),
# expects VERDICT: PASS, then draws a wrong picture (safety rectangle on the
# other side for scenes 2..8, colours swapped for scene 1) and expects
# VERDICT: FAIL.
#
# NOTE: this only tests the tools against pictures made by this script. It shows
# nothing about a real panel or camera or about the board.
# Scratch files are under $TMPDIR.

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/am62p-scenes.py
cam=$here/camera-check.py
root=$(mktemp -d "${TMPDIR:-.}/am62p-scenes-camera.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
W=1280
H=800

cat > "$root/draw.py" <<'P'
import sys
scene, w, h, mode, out = int(sys.argv[1]), int(sys.argv[2]), int(sys.argv[3]), sys.argv[4], sys.argv[5]
# application plane colour (None = off, background black), safety side
app = {2: (0, 0, 255), 3: (0, 255, 0), 4: (0, 255, 0)}.get(scene)
side = {1: "full", 2: "left", 3: "left"}.get(scene, "right")
red, bg = (255, 0, 0), app or (0, 0, 0)
if mode == "bad":
    if side == "full":
        red = (0, 255, 0)  # colours swapped
    else:
        side = "right" if side == "left" else "left"
rw, rh, ry = w // 3, h // 2, h // 4
rx = w // 12 if side == "left" else w - w // 12 - rw
rows = []
for y in range(h):
    if side == "full":
        rows.append(bytes(red) * w)
    elif ry <= y < ry + rh:
        rows.append(bytes(bg) * rx + bytes(red) * rw + bytes(bg) * (w - rx - rw))
    else:
        rows.append(bytes(bg) * w)
with open(out, "wb") as f:
    f.write(b"P6\n%d %d\n255\n" % (w, h))
    f.write(b"".join(rows))
P

check() { # name scene mode want
    python3 -I "$root/draw.py" "$2" "$W" "$H" "$3" "$root/p.ppm" || { echo "FAIL: $1 (draw)"; fail=1; return; }
    python3 -I "$cam" "$root/p.ppm" "$root/e$2.json" > "$root/out.log" 2>&1
    if grep -q "VERDICT: $4" "$root/out.log"; then
        echo "ok: $1 -> $4"
    else
        echo "FAIL: $1 expected $4"; cat "$root/out.log"; fail=1
    fi
}

for n in 1 2 3 4 5 6 7 8; do
    if ! python3 -I "$tool" expect --scene "$n" --width "$W" --height "$H" --panel full --rotation 0 > "$root/e$n.json"; then
        echo "FAIL: scene $n expect"; fail=1; continue
    fi
    check "scene $n right picture" "$n" good PASS
    check "scene $n wrong picture" "$n" bad FAIL
done

if [ "$fail" -eq 0 ]; then echo PASS; else echo FAIL; exit 1; fi
