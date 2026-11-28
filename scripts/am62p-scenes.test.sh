#!/bin/sh
# Self-test for am62p-scenes.py, with a camera-check.py round trip.
# Run: sh scripts/am62p-scenes.test.sh
# Builds its pictures under a mktemp directory inside $TMPDIR.

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/am62p-scenes.py
cam=$here/camera-check.py
data=$here/testdata/am62p-scenes
root=$(mktemp -d "${TMPDIR:-.}/am62p-scenes-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
py() { python3 -I "$@"; }

# A synthetic picture of each scene, drawn from the firmware's integer
# geometry (scenes.c scenes_rect), independent of the tool, turned upside
# down like the board camera's picture, with a little colour error.
cat > "$root/render.py" <<'P'
import sys
scene, w, h, out = int(sys.argv[1]), int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
app = {2: (0, 0, 248), 3: (0, 252, 0), 4: (0, 252, 0)}.get(scene)
side = {1: "full", 2: "left", 3: "left"}.get(scene, "right")
rw, rh, ry = w // 3, h // 2, h // 4
rx = w // 12 if side == "left" else w - w // 12 - rw
red, bg = (250, 5, 5), app or (0, 0, 0)
pix = []
for y in range(h):
    for x in range(w):
        inside = side == "full" or (rx <= x < rx + rw and ry <= y < ry + rh)
        pix.append(red if inside else bg)
pix.reverse()  # 180 degrees
with open(out, "wb") as f:
    f.write(b"P6\n%d %d\n255\n" % (w, h))
    f.write(bytes(c for p in pix for c in p))
P

W=240
H=150
for n in 1 2 3 4 5 6 7 8; do
    py "$tool" expect --scene $n --width $W --height $H > "$root/e$n.json" || bad "expect $n exit"
    py "$root/render.py" $n $W $H "$root/p$n.ppm"
    if py "$cam" "$root/p$n.ppm" "$root/e$n.json" > "$root/c$n.log"; then
        ok "scene $n picture passes its EXPECT"
    else
        bad "scene $n picture against its EXPECT"; cat "$root/c$n.log"
    fi
done
# By name, the same JSON as by number.
py "$tool" expect --scene app-under-safety --width $W --height $H > "$root/byname.json"
if cmp -s "$root/byname.json" "$root/e2.json"; then ok "scene by name"; else bad "scene by name"; fi
# The panel of the doc.
py "$tool" expect --scene 2 --width 1280 --height 800 > "$root/e2big.json" || bad "expect 1280x800"
py "$root/render.py" 2 1280 800 "$root/p2big.ppm"
if py "$cam" "$root/p2big.ppm" "$root/e2big.json" > /dev/null; then ok "scene 2 at 1280x800"; else bad "scene 2 at 1280x800"; fi

# A wrong picture fails: scene 4's picture (rectangle right) against scene 3.
if py "$cam" "$root/p4.ppm" "$root/e3.json" > /dev/null; then bad "wrong picture passes"; else ok "wrong picture fails"; fi
# A refused scene shows what scene 5 showed.
if py "$cam" "$root/p5.ppm" "$root/e8.json" > /dev/null; then ok "scene 8 shows scene 5"; else bad "scene 8 shows scene 5"; fi

# JSON fields and the corners mode.
py "$tool" expect --scene 1 --width $W --height $H --tolerance 30 --rotation 0 \
    --panel corners --corners 0.1,0.1,0.9,0.1,0.9,0.9,0.1,0.9 > "$root/corn.json" || bad "corners exit"
cat > "$root/chk.py" <<'P'
import json, sys
d = json.load(open(sys.argv[1]))
assert d["rotation"] == 0 and d["tolerance"] == 30, d
assert d["panel"] == {"corners": [[0.1, 0.1], [0.9, 0.1], [0.9, 0.9], [0.1, 0.9]], "unit": "frac"}, d
assert all(r["expect"] == [255, 0, 0] for r in d["regions"]), d
P
if py "$root/chk.py" "$root/corn.json"; then ok "tolerance, rotation, corners"; else bad "tolerance, rotation, corners"; fi
for args in "--scene 9 --width 10 --height 10" "--scene x --width 240 --height 150" \
    "--scene 1 --width 0 --height 10" "--scene 1 --width 240" \
    "--scene 1 --width 240 --height 150 --panel corners" \
    "--scene 1 --width 240 --height 150 --bogus 1"; do
    # shellcheck disable=SC2086
    py "$tool" expect $args > /dev/null 2>&1
    rc=$?
    if [ $rc -eq 2 ]; then ok "expect rejects: $args"; else bad "expect accepted: $args"; fi
done

# summary
py "$tool" summary "$data/host-run.log" > "$root/sum.md"; rc=$?
if [ $rc -eq 0 ]; then ok "host run summary exit 0"; else bad "host run summary exit $rc"; fi
if [ "$(grep -c '^| [1-8] ' "$root/sum.md")" -eq 8 ]; then ok "eight rows"; else bad "eight rows"; fi
if grep -q '^| 2 app-under-safety | 0 (0) | 4 | 4 | +3 (+3) | yes | 1 (1) | ok |$' "$root/sum.md"; then
    ok "scene 2 row"; else bad "scene 2 row"; fi
if grep -q '^| 7 refused-blank | -16 (-16) | 7 | 7 | +0 (+0) | - | 0 (0) | ok |$' "$root/sum.md"; then
    ok "scene 7 row"; else bad "scene 7 row"; fi
if grep -q '^RESULT PASS (8 scenes, vsyncs 61)$' "$root/sum.md"; then ok "RESULT line"; else bad "RESULT line"; fi
if py "$tool" summary - < "$data/host-run.log" > /dev/null; then ok "summary from stdin"; else bad "summary from stdin"; fi

# Trace-buffer style: a prefix on every line and CR LF line ends.
sed 's/^/[trace] /; s/$/\r/' "$data/host-run.log" > "$root/prefixed.log"
if py "$tool" summary "$root/prefixed.log" > /dev/null; then ok "prefixed lines"; else bad "prefixed lines"; fi

# A failed scene: the table is printed, exit 1.
sed 's/latched 5 (+1, expected +1), presented yes, released 1 (expected 1): ok/latched 4 (+1, expected +1), presented NO, released 0 (expected 1): FAIL/; s/^RESULT PASS (8 scenes/RESULT FAIL (1 checks failed/' \
    "$data/host-run.log" > "$root/fail.log"
py "$tool" summary "$root/fail.log" > "$root/fail.md"; rc=$?
if [ $rc -eq 1 ] && grep -q '^| 3 app-replaced .*| FAIL |$' "$root/fail.md"; then ok "failed scene: exit 1, FAIL row"; else bad "failed scene ($rc)"; fi
# A line that says ok while its numbers disagree is a FAIL.
sed 's/commits 5 latched 5/commits 5 latched 4/' "$data/host-run.log" > "$root/lie.log"
py "$tool" summary "$root/lie.log" > "$root/lie.md"; rc=$?
if [ $rc -eq 1 ] && grep -q '^| 3 app-replaced .*| FAIL |$' "$root/lie.md"; then ok "latched below commits: FAIL"; else bad "latched below commits ($rc)"; fi
# A hold failure line.
{ cat "$data/host-run.log"; echo 'SCENE 4 safety-flipped: commits changed while held: 6 -> 7: FAIL'; } > "$root/held.log"
py "$tool" summary "$root/held.log" > "$root/held.md"; rc=$?
if [ $rc -eq 1 ] && grep -q 'commits changed while held: 6 -> 7' "$root/held.md"; then ok "commits changed while held"; else bad "held ($rc)"; fi

# Input errors: exit 2 and a message.
grep -v '^SCENE 6 ' "$data/host-run.log" > "$root/missing.log"
sed 's/commits 4 latched 4/commits four latched 4/' "$data/host-run.log" > "$root/malformed.log"
grep -v '^RESULT' "$data/host-run.log" > "$root/noresult.log"
sed 's/^SCENE 3 app-replaced/SCENE 3 app-green/' "$data/host-run.log" > "$root/name.log"
{ cat "$data/host-run.log"; grep '^SCENE 1 ' "$data/host-run.log"; } > "$root/twice.log"
: > "$root/empty.log"
for f in missing malformed noresult name twice empty; do
    py "$tool" summary "$root/$f.log" > /dev/null 2> "$root/$f.err"
    rc=$?
    if [ $rc -eq 2 ] && [ -s "$root/$f.err" ]; then ok "summary rejects $f"; else bad "summary $f exit $rc"; fi
done
if grep -q 'missing scene 6' "$root/missing.err"; then ok "names the missing scene"; else bad "names the missing scene"; fi
py "$tool" summary "$root/nonexistent" > /dev/null 2>&1
if [ $? -eq 2 ]; then ok "unreadable input"; else bad "unreadable input"; fi
py "$tool" > /dev/null 2>&1
if [ $? -eq 2 ]; then ok "no subcommand"; else bad "no subcommand"; fi

if [ $fail -eq 0 ]; then echo "all passed"; else echo "FAILED"; fi
exit $fail
