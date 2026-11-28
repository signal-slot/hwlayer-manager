#!/bin/sh
# Self-test for twoplane-expect.py, with a camera-check.py round trip.
# Run: sh scripts/twoplane-expect.test.sh
# Builds fixtures under a mktemp directory inside $TMPDIR.

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/twoplane-expect.py
cam=$here/camera-check.py
root=$(mktemp -d "${TMPDIR:-.}/tpe-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
py() { python3 -I "$@"; }

cat > "$root/normal.txt" <<'T'
GEOMETRY output 200x100
EXPECT phase=normal "green block" x=40 y=50 fx=0.2000 fy=0.5000 rgb=#00ff00
EXPECT phase=normal "overlay opaque over green" x=100 y=50 fx=0.5000 fy=0.5000 rgb=#ffff00
EXPECT phase=normal "background" x=180 y=20 fx=0.9000 fy=0.2000 rgb=#102030
T
grep '^EXPECT' "$root/normal.txt" | sed 's/phase=normal/phase=swapped/;s/#ffff00/#00ff00/' > "$root/swapped.txt"
sed 's/$/ skipped/' "$root/swapped.txt" > "$root/swapped-skipped.txt"

cp "$root/normal.txt" "$root/n.txt"
cat "$root/normal.txt" "$root/swapped.txt" > "$root/ns.txt"
cat "$root/normal.txt" "$root/swapped-skipped.txt" > "$root/skip.txt"
{
    cat "$root/normal.txt"
    echo 'SWAPZ skipped: a plane of the plan has a fixed zpos'
    cat "$root/swapped.txt"
} > "$root/swapz.txt"

py "$tool" "$root/n.txt" --out "$root/a" > "$root/a.log" || bad "normal only exit"
if [ -f "$root/a/expect-normal.json" ] && [ ! -e "$root/a/expect-swapped.json" ]; then ok "normal only"; else bad "normal only files"; fi

py "$tool" "$root/ns.txt" --out "$root/b" > "$root/b.log" || bad "both exit"
if [ -f "$root/b/expect-normal.json" ] && [ -f "$root/b/expect-swapped.json" ]; then ok "normal+swapped"; else bad "both files"; fi
cat > "$root/chk.py" <<'P'
import json, sys
d = json.load(open(sys.argv[1]))
assert d["rotation"] == 0 and d["panel"]["unit"] == "frac" and d["tolerance"] == 40
r = d["regions"][1]
assert r["name"] == "overlay opaque over green" and r["expect"] == [0, 255, 0]
assert r["rect"] == [0.49, 0.49, 0.51, 0.51], r["rect"]
P
if py "$root/chk.py" "$root/b/expect-swapped.json"; then ok "swapped json content"; else bad "swapped json content"; fi

for t in skip swapz; do
    py "$tool" "$root/$t.txt" --out "$root/$t" > "$root/$t.log" || bad "$t exit"
    if [ -f "$root/$t/expect-normal.json" ] && [ ! -e "$root/$t/expect-swapped.json" ] \
        && grep -q 'skipped phase swapped' "$root/$t.log"; then
        ok "$t: swapped skipped"
    else
        bad "$t: swapped skipped"
    fi
done

py "$tool" "$root/n.txt" > /dev/null 2>&1
[ $? -eq 2 ] && ok "no --out is exit 2" || bad "no --out"
py "$tool" "$root/nonexistent" --out "$root/x" > /dev/null 2>&1
[ $? -eq 2 ] && ok "missing input is exit 2" || bad "missing input"
echo 'GEOMETRY output 1x1' | py "$tool" --out "$root/y" > /dev/null 2>&1
[ $? -eq 2 ] && ok "no EXPECT is exit 2" || bad "no EXPECT"
echo 'EXPECT phase=normal "a" x=1 y=1 fx=0.5 fy=0.5 rgb=#zzzzzz' | py "$tool" --out "$root/z" > /dev/null 2>&1
[ $? -eq 2 ] && ok "bad rgb is exit 2" || bad "bad rgb"
if py "$tool" - --out "$root/s" < "$root/n.txt" > /dev/null && [ -f "$root/s/expect-normal.json" ]; then ok "stdin"; else bad "stdin"; fi

# phase=commit (plane_commit): any phase name, skipped of any phase, mixed phases.
cat > "$root/commit.txt" <<'T'
EXPECT phase=commit "left" x=40 y=50 fx=0.2500 fy=0.5000 rgb=#ff0000
EXPECT phase=commit "right" x=160 y=50 fx=0.7500 fy=0.5000 rgb=#0000ff
T
py "$tool" "$root/commit.txt" --out "$root/c" > "$root/c.log" || bad "commit exit"
if [ -f "$root/c/expect-commit.json" ] && [ "$(ls "$root/c" | wc -l)" -eq 1 ] \
    && py -c 'import json,sys; d=json.load(open(sys.argv[1])); assert [r["expect"] for r in d["regions"]]==[[255,0,0],[0,0,255]]' "$root/c/expect-commit.json"; then
    ok "commit phase"
else bad "commit phase"; fi
echo 'EXPECT phase=commit skipped reason=no plane with alpha' > "$root/cs.txt"
cat "$root/normal.txt" "$root/cs.txt" > "$root/ncs.txt"
py "$tool" "$root/ncs.txt" --out "$root/d" > "$root/d.log" || bad "commit skipped exit"
if [ -f "$root/d/expect-normal.json" ] && [ ! -e "$root/d/expect-commit.json" ] \
    && grep -q 'skipped phase commit' "$root/d.log"; then ok "commit skipped"; else bad "commit skipped"; fi
py "$tool" "$root/cs.txt" --out "$root/d2" > /dev/null 2>&1
[ $? -eq 2 ] && ok "only skipped is exit 2" || bad "only skipped"
cat "$root/ns.txt" "$root/commit.txt" > "$root/mix.txt"
py "$tool" "$root/mix.txt" --out "$root/e" > /dev/null || bad "mixed exit"
if [ -f "$root/e/expect-normal.json" ] && [ -f "$root/e/expect-swapped.json" ] && [ -f "$root/e/expect-commit.json" ]; then ok "mixed phases"; else bad "mixed phases"; fi
echo 'EXPECT phase=../x "a" x=1 y=1 fx=0.5 fy=0.5 rgb=#000000' | py "$tool" --out "$root/f" > /dev/null 2>&1
[ $? -eq 2 ] && ok "bad phase name is exit 2" || bad "bad phase name"

# Marker line from twoplane --swap-z when the swap cannot run.
echo 'EXPECT phase=swapped skipped reason=fixed-zpos' > "$root/mark.txt"
cat "$root/normal.txt" "$root/mark.txt" > "$root/nm.txt"
py "$tool" "$root/nm.txt" --out "$root/m1" > "$root/m1.log" 2> "$root/m1.err" || bad "marker exit"
if [ -f "$root/m1/expect-normal.json" ] && [ ! -e "$root/m1/expect-swapped.json" ] \
    && grep -q 'skipped phase swapped' "$root/m1.log" && [ ! -s "$root/m1.err" ]; then
    ok "marker: normal + skipped"
else bad "marker: normal + skipped"; fi
py "$tool" "$root/nm.txt" --out "$root/m2" --phase normal > /dev/null 2>&1
if [ $? -eq 0 ] && [ -f "$root/m2/expect-normal.json" ] && [ ! -e "$root/m2/expect-swapped.json" ]; then
    ok "marker: --phase normal"
else bad "marker: --phase normal"; fi
py "$tool" "$root/nm.txt" --out "$root/m3" --phase swapped > "$root/m3.log" 2> "$root/m3.err"
rc=$?
if [ $rc -eq 3 ] && [ -z "$(ls "$root/m3" 2>/dev/null)" ] \
    && grep -q 'swapped' "$root/m3.err" && grep -q 'fixed-zpos' "$root/m3.err"; then
    ok "marker: --phase swapped is exit 3 with reason"
else bad "marker: --phase swapped (rc=$rc)"; cat "$root/m3.err"; fi
py "$tool" "$root/nm.txt" --out "$root/m4" --phase normal --phase swapped > /dev/null 2>&1
rc=$?
if [ $rc -eq 3 ] && [ -f "$root/m4/expect-normal.json" ]; then ok "marker: both phases, exit 3, normal written"; else bad "marker: both phases (rc=$rc)"; fi
py "$tool" "$root/n.txt" --out "$root/m5" --phase swapped > /dev/null 2>&1
[ $? -eq 2 ] && ok "absent phase is exit 2" || bad "absent phase"
py "$tool" "$root/swapz.txt" --out "$root/m6" --phase swapped > /dev/null 2> "$root/m6.err"
rc=$?
if [ $rc -eq 3 ] && grep -q 'fixed zpos' "$root/m6.err"; then ok "SWAPZ line: --phase swapped is exit 3"; else bad "SWAPZ line --phase swapped"; fi

# Round trip: a picture painted from the canned text, and one with a wrong colour.
cat > "$root/mk.py" <<'P'
import re, sys
txt, out, wrong = sys.argv[1], sys.argv[2], sys.argv[3] == "wrong"
w, h = 200, 100
px = [[(0, 0, 0)] * w for _ in range(h)]
for line in open(txt):
    m = re.search(r'phase=normal "([^"]*)".* x=(\d+) y=(\d+) .*rgb=#(\w{6})', line)
    if not m:
        continue
    x, y = int(m.group(2)), int(m.group(3))
    c = tuple(int(m.group(4)[i:i + 2], 16) for i in (0, 2, 4))
    if wrong and m.group(1) == "background":
        c = (255, 0, 255)
    for j in range(y - 8, y + 9):
        for i in range(x - 8, x + 9):
            px[j][i] = c
with open(out, "wb") as f:
    f.write(b"P6\n%d %d\n255\n" % (w, h))
    for row in px:
        for p in row:
            f.write(bytes(p))
P
py "$root/mk.py" "$root/n.txt" "$root/good.ppm" good
py "$root/mk.py" "$root/n.txt" "$root/wrong.ppm" wrong
py "$cam" "$root/good.ppm" "$root/a/expect-normal.json" > "$root/g.log" 2>&1
if grep -q 'VERDICT: PASS' "$root/g.log"; then ok "camera-check PASS on matching picture"; else bad "camera-check PASS"; cat "$root/g.log"; fi
py "$cam" "$root/wrong.ppm" "$root/a/expect-normal.json" > "$root/w.log" 2>&1
if grep -q 'VERDICT: FAIL' "$root/w.log"; then ok "camera-check FAIL on wrong colour"; else bad "camera-check FAIL"; cat "$root/w.log"; fi

[ $fail -eq 0 ] && echo "all ok"
exit $fail
