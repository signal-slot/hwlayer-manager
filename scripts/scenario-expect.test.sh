#!/bin/sh
# Self-test for scenario-expect.py, with a camera-check.py round trip.
# Run: sh scripts/scenario-expect.test.sh
# Builds fixtures under a mktemp directory inside $TMPDIR.

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/scenario-expect.py
cam=$here/camera-check.py
root=$(mktemp -d "${TMPDIR:-.}/se-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
py() { python3 -I "$@"; }

# Painter: a 400x225 picture (panel = the whole picture, rotation 0) in which
# every region of the EXPECT file is filled with its expected colour, on black.
# mode: good | wrong (first coloured region painted magenta) | blank (all black)
# | grey (all mid grey).
cat > "$root/mk.py" <<'P'
import json, sys
exp = json.load(open(sys.argv[1]))
out, mode = sys.argv[2], sys.argv[3]
w, h = 400, 225
px = [[(96, 96, 96) if mode == "grey" else (0, 0, 0)] * w for _ in range(h)]
if mode in ("good", "wrong"):
    wrong = mode == "wrong"
    for r in exp["regions"]:
        c = tuple(r.get("expect", (30, 30, 30)))
        if wrong and "expect" in r:
            c = (255, 0, 255)
            wrong = False
        u0, v0, u1, v1 = r["rect"]
        for y in range(int(v0 * h), int(v1 * h) + 1):
            for x in range(int(u0 * w), int(u1 * w) + 1):
                px[y][x] = c
with open(out, "wb") as f:
    f.write(b"P6\n%d %d\n255\n" % (w, h))
    for row in px:
        for p in row:
            f.write(bytes(p))
P
cat > "$root/chk.py" <<'P'
import json, sys
d = json.load(open(sys.argv[1]))
assert d["rotation"] == int(sys.argv[2]) and d["panel"]["unit"] == "frac"
assert d["regions"]
for r in d["regions"]:
    u0, v0, u1, v1 = r["rect"]
    assert 0 <= u0 < u1 <= 1 and 0 <= v0 < v1 <= 1, r
    if sys.argv[3] == "colours":
        assert len(r["expect"]) == 3 and all(0 <= c <= 255 for c in r["expect"]), r
if sys.argv[3] == "compare":
    assert d["compare"] and all(c["expect"] == sys.argv[4] for c in d["compare"])
    names = {r["name"] for r in d["regions"]}
    assert all(c["region"] in names for c in d["compare"])
else:
    assert "compare" not in d
P

steps="a-over-b b-over-a plane-alpha-50 pixel-alpha-premultiplied"
for s in two-planes zorder-alpha move rotate revoke; do
    py "$tool" --scenario "$s" > "$root/$s.json" || bad "$s exit"
    if python3 -c 'import json,sys; json.load(open(sys.argv[1]))' "$root/$s.json" \
        && py "$root/chk.py" "$root/$s.json" 0 colours; then ok "$s json"; else bad "$s json"; fi
done
for st in $steps; do
    py "$tool" --scenario zorder-alpha --step "$st" > "$root/z-$st.json" || bad "step $st exit"
    if py "$root/chk.py" "$root/z-$st.json" 0 colours; then ok "zorder-alpha $st json"; else bad "zorder-alpha $st json"; fi
done
py "$tool" --scenario combos > "$root/combos.json" || bad "combos exit"
if py "$root/chk.py" "$root/combos.json" 0 compare same; then ok "combos json (compare same)"; else bad "combos json"; fi
for s in stop modeset recovery; do
    py "$tool" --scenario "$s" > "$root/$s.json" || bad "$s exit"
    if py "$root/chk.py" "$root/$s.json" 0 compare changed; then ok "$s json (compare changed)"; else bad "$s json"; fi
done
py "$tool" --scenario stop --after rotate > "$root/stop-rotate.json" && py "$root/chk.py" "$root/stop-rotate.json" 0 compare changed \
    && grep -q 'red top plane' "$root/stop-rotate.json" && ok "stop --after rotate" || bad "stop --after rotate"

# Options.
py "$tool" --scenario two-planes --rotation 180 --width 1280 --height 720 > "$root/r180.json" \
    && py "$root/chk.py" "$root/r180.json" 180 colours && ok "--rotation 180, --width/--height" || bad "--rotation"
py "$tool" --scenario rotate --width 1280 > "$root/r1280.json"
py "$tool" --scenario rotate --width 3840 > "$root/r3840.json"
if python3 -c '
import json, sys
a = json.load(open(sys.argv[1]))["tolerance"]
b = json.load(open(sys.argv[2]))["tolerance"]
assert a > b >= 40, (a, b)' "$root/r1280.json" "$root/r3840.json"; then ok "bar tolerance depends on width"; else bad "bar tolerance"; fi
py "$tool" --list | grep -qx revoke && ok "--list" || bad "--list"
py "$tool" --scenario nope > /dev/null 2>&1
[ $? -eq 2 ] && ok "unknown scenario is exit 2" || bad "unknown scenario"
py "$tool" > /dev/null 2>&1
[ $? -eq 2 ] && ok "no scenario is exit 2" || bad "no scenario"
py "$tool" --scenario two-planes --rotation 45 > /dev/null 2>&1
[ $? -eq 2 ] && ok "bad rotation is exit 2" || bad "bad rotation"
py "$tool" --scenario zorder-alpha --step c-over-d > /dev/null 2>&1
[ $? -eq 2 ] && ok "bad step is exit 2" || bad "bad step"
py "$tool" --scenario two-planes --panel corners > /dev/null 2>&1
[ $? -eq 2 ] && ok "corners without --corners is exit 2" || bad "corners"

# Round trip: camera-check accepts every coloured scenario's file, passes a
# matching picture and fails one with a wrong colour.
for s in two-planes zorder-alpha move rotate revoke $(for st in $steps; do echo "z-$st"; done); do
    py "$root/mk.py" "$root/$s.json" "$root/$s-good.ppm" good
    py "$root/mk.py" "$root/$s.json" "$root/$s-wrong.ppm" wrong
    py "$cam" "$root/$s-good.ppm" "$root/$s.json" > "$root/g.log" 2>&1
    if grep -q 'VERDICT: PASS' "$root/g.log"; then ok "camera-check PASS: $s"; else bad "camera-check PASS: $s"; cat "$root/g.log"; fi
    py "$cam" "$root/$s-wrong.ppm" "$root/$s.json" > "$root/w.log" 2>&1
    if grep -q 'VERDICT: FAIL' "$root/w.log"; then ok "camera-check FAIL on wrong colour: $s"; else bad "camera-check FAIL: $s"; fi
done

# Compare entries: hold picture against a blank (stopped) picture is "changed",
# against itself is not; combos wants "same".
py "$root/mk.py" "$root/stop.json" "$root/hold.ppm" good
py "$root/mk.py" "$root/two-planes.json" "$root/hold.ppm" good
py "$root/mk.py" "$root/stop.json" "$root/blank.ppm" blank
for s in stop modeset recovery; do
    py "$cam" "$root/hold.ppm" "$root/$s.json" --compare "$root/blank.ppm" > "$root/g.log" 2>&1
    if grep -q 'VERDICT: PASS' "$root/g.log"; then ok "$s: changed picture passes"; else bad "$s: changed picture"; cat "$root/g.log"; fi
    py "$cam" "$root/hold.ppm" "$root/$s.json" --compare "$root/hold.ppm" > "$root/w.log" 2>&1
    if grep -q 'VERDICT: FAIL' "$root/w.log"; then ok "$s: unchanged picture fails"; else bad "$s: unchanged picture"; fi
done
py "$cam" "$root/hold.ppm" "$root/combos.json" --compare "$root/hold.ppm" > "$root/g.log" 2>&1
if grep -q 'VERDICT: PASS' "$root/g.log"; then ok "combos: same picture passes"; else bad "combos: same picture"; fi
py "$cam" "$root/hold.ppm" "$root/combos.json" --compare "$root/blank.ppm" > "$root/w.log" 2>&1
if grep -q 'VERDICT: FAIL' "$root/w.log"; then ok "combos: changed picture fails"; else bad "combos: changed picture"; fi

[ $fail -eq 0 ] && echo "all ok"
exit $fail
