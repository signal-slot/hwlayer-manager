#!/bin/sh
# Self-test for ltdc-picture-expect.py, with a camera-check.py round trip.
# Run: sh scripts/ltdc-picture-expect.test.sh
# Builds fixtures under a mktemp directory inside $TMPDIR.

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/ltdc-picture-expect.py
root=$(mktemp -d "${TMPDIR:-.}/lpe-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
py() { python3 -I "$@"; }

cat > "$root/lib.py" <<'P'
import json, sys
cmd, path = sys.argv[1], sys.argv[2]
d = json.load(open(path))
regs = d["regions"]
if cmd == "colours":
    print(" ".join("%s:%s" % (r["name"].split()[1], ",".join(map(str, r["expect"]))) for r in regs))
elif cmd == "rect":
    print(*[r for r in regs if r["name"].split()[1] == sys.argv[3]][0]["rect"])
elif cmd == "first":
    print(*regs[0]["expect"])
elif cmd == "same":
    print(regs == json.load(open(sys.argv[3]))["regions"])
elif cmd == "header":
    assert d["rotation"] == 0 and d["panel"]["unit"] == "frac" and d["tolerance"] == 40
    assert d["prediction"].startswith("PREDICTION (inferred / read from source)")
P
colours() { py "$root/lib.py" colours "$1"; }
rect() { py "$root/lib.py" rect "$1" "$2"; }

M=1000x800
P=100,200,400x200

# Identity: tl red centre (200,250) of a 1000x800 mode.
py "$tool" --mode $M --plane $P --out "$root/id" --name id > /dev/null || bad "identity exit"
[ "$(colours "$root/id/expect-id.json")" = "tl:255,0,0 tr:0,255,0 bl:0,0,255 br:255,255,255" ] \
    && ok "identity colours" || bad "identity colours"
[ "$(rect "$root/id/expect-id.json" tl)" = "0.19 0.3025 0.21 0.3225" ] && ok "identity rect" || bad "identity rect: $(rect "$root/id/expect-id.json" tl)"
py "$root/lib.py" header "$root/id/expect-id.json" && ok "json header and prediction mark" || bad "json header"

# reflect-x: left and right swap, rows stay.
py "$tool" --mode $M --plane $P --transform reflect-x --out "$root/rx" --name rx > /dev/null || bad "reflect-x exit"
[ "$(rect "$root/rx/expect-rx.json" tl)" = "0.39 0.3025 0.41 0.3225" ] && ok "reflect-x tl at right" || bad "reflect-x tl"
[ "$(rect "$root/rx/expect-rx.json" tr)" = "0.19 0.3025 0.21 0.3225" ] && ok "reflect-x tr at left" || bad "reflect-x tr"
[ "$(rect "$root/rx/expect-rx.json" bl)" = "0.39 0.4275 0.41 0.4475" ] && ok "reflect-x bl keeps row" || bad "reflect-x bl"

# reflect-y: top and bottom swap.
py "$tool" --mode $M --plane $P --transform reflect-y --out "$root/ry" --name ry > /dev/null || bad "reflect-y exit"
[ "$(rect "$root/ry/expect-ry.json" tl)" = "0.19 0.4275 0.21 0.4475" ] && ok "reflect-y tl at bottom" || bad "reflect-y tl: $(rect "$root/ry/expect-ry.json" tl)"

# rotate-180: tl red at the bottom right; equals reflect-x+reflect-y.
py "$tool" --mode $M --plane $P --transform 180 --out "$root/r180" --name r > /dev/null || bad "180 exit"
py "$tool" --mode $M --plane $P --transform reflect-x+reflect-y --out "$root/rxy" --name r > /dev/null
[ "$(rect "$root/r180/expect-r.json" tl)" = "0.39 0.4275 0.41 0.4475" ] && ok "rotate-180 tl at bottom right" || bad "rotate-180 tl"
[ "$(rect "$root/r180/expect-r.json" br)" = "0.19 0.3025 0.21 0.3225" ] && ok "rotate-180 br at top left" || bad "rotate-180 br"
[ "$(py "$root/lib.py" same "$root/r180/expect-r.json" "$root/rxy/expect-r.json")" = "True" ] \
    && ok "180 equals reflect-x+reflect-y" || bad "180 vs reflect-x+reflect-y"

# 90 (counter-clockwise): tl lands at the bottom left; stderr carries a note.
py "$tool" --mode $M --plane $P --transform 90 --out "$root/r90" --name r > /dev/null 2> "$root/r90.err"
[ "$(rect "$root/r90/expect-r.json" tl)" = "0.19 0.4275 0.21 0.4475" ] && ok "rotate-90 tl at bottom left" || bad "rotate-90 tl"
grep -q 'null pointer' "$root/r90.err" && ok "rotate-90 warns" || bad "rotate-90 warns"

# YUV conversions of known values (docs/stm32mp257-ltdc-yuv-source.md).
yuv() { py "$tool" --mode 100x100 --plane 0,0,50x50 --yuv "$1" --encoding "$2" --range "$3" | py "$root/lib.py" first /dev/stdin; }
chk() { [ "$(yuv "$1" "$2" "$3")" = "$4" ] && ok "$2 $3 $1 -> $4" || bad "$2 $3 $1 -> $(yuv "$1" "$2" "$3"), want $4"; }
chk 81,90,240 bt601 limited "255 0 0"
chk 145,54,34 bt601 limited "0 255 1"
chk 41,240,110 bt601 limited "0 0 255"
chk 16,128,128 bt601 limited "0 0 0"
chk 235,128,128 bt601 limited "255 255 255"
chk 63,102,240 bt709 limited "255 0 0"
chk 255,128,128 bt709 full "255 255 255"
chk 0,128,128 bt709 full "0 0 0"
chk 63,102,240 bt709 full "239 15 15"
chk 76,85,255 bt601 full "254 0 0"
# Limited-range black on a plane set to full range stays grey.
chk 16,128,128 bt601 full "16 16 16"

# NV12 quadrants: plane_commit's BT.601 limited Y,Cb,Cr through the model.
py "$tool" --mode $M --plane $P --nv12 --out "$root/nv" --name nv > /dev/null || bad "nv12 exit"
[ "$(colours "$root/nv/expect-nv.json")" = "tl:255,1,0 tr:0,254,0 bl:0,0,255 br:255,255,255" ] \
    && ok "nv12 default colours" || bad "nv12 default: $(colours "$root/nv/expect-nv.json")"
py "$tool" --mode $M --plane $P --nv12 --encoding bt709 --range full --out "$root/nv2" --name nv > /dev/null || bad "nv12 709 exit"
[ "$(py "$root/lib.py" first "$root/nv2/expect-nv.json")" != "255 0 0" ] && ok "nv12 bt709 full shifts red" || bad "nv12 709 full"

# Scaling is noted; positions do not change.
py "$tool" --mode $M --plane $P --src 100x50 | grep -q 'scaled 100x50 -> 400x200' && ok "scaling noted" || bad "scaling note"

# Round trip with camera-check on a synthetic picture of the identity layout
# (the 1000x800 layout scaled by 0.2 to 200x160).
cat > "$root/mk.py" <<'P'
import sys
w, h = 200, 160
cols = {(0, 0): (255, 0, 0), (1, 0): (0, 255, 0), (0, 1): (0, 0, 255), (1, 1): (255, 255, 255)}
rows = []
for y in range(h):
    row = bytearray()
    for x in range(w):
        if 20 <= x < 100 and 40 <= y < 80:
            c = cols[(int((x - 20) * 2 >= 80), int((y - 40) * 2 >= 40))]
        else:
            c = (0, 0, 0)
        row += bytes(c)
    rows.append(bytes(row))
open(sys.argv[1], "wb").write(b"P6\n%d %d\n255\n" % (w, h) + b"".join(rows))
P
py "$root/mk.py" "$root/pic.ppm"
if py "$here/camera-check.py" "$root/pic.ppm" "$root/id/expect-id.json" > "$root/cc.log" 2>&1; then ok "camera-check round trip PASS"; else bad "camera-check round trip"; cat "$root/cc.log"; fi
if py "$here/camera-check.py" "$root/pic.ppm" "$root/rx/expect-rx.json" > /dev/null 2>&1; then bad "wrong transform should FAIL"; else ok "wrong transform FAILs"; fi

# Without --out the JSON goes to stdout.
py "$tool" --mode $M --plane $P | py -c 'import json,sys; json.load(sys.stdin)' && ok "stdout json" || bad "stdout json"

# Input errors are exit 2.
err() {
    desc=$1; shift
    py "$tool" "$@" > /dev/null 2>&1
    [ $? -eq 2 ] && ok "error: $desc" || bad "error: $desc"
}
err "no arguments"
err "no plane" --mode $M
err "bad mode" --mode 10by10 --plane $P
err "plane outside mode" --mode 300x300 --plane $P
err "bad transform" --mode $M --plane $P --transform sideways
err "two angles" --mode $M --plane $P --transform 90+180
err "repeated transform" --mode $M --plane $P --transform reflect-x+reflect-x
err "bad yuv" --mode $M --plane $P --yuv 1,2
err "yuv out of range" --mode $M --plane $P --yuv 1,2,300
err "yuv with transform" --mode $M --plane $P --yuv 1,2,3 --transform 180
err "nv12 with yuv" --mode $M --plane $P --nv12 --yuv 1,2,3
err "bad encoding" --mode $M --plane $P --nv12 --encoding bt2020
err "bad range" --mode $M --plane $P --nv12 --range wide
err "encoding without yuv" --mode $M --plane $P --encoding bt709
err "odd nv12 size" --mode $M --plane 0,0,401x200 --nv12
err "bad size" --mode $M --plane $P --size 0
err "bad rotation" --mode $M --plane $P --rotation 45
err "unknown argument" --mode $M --plane $P --bogus
err "missing value" --mode $M --plane
err "bad name" --mode $M --plane $P --name 'A b'

[ $fail -eq 0 ] && echo "ltdc-picture-expect.test.sh: PASS" || echo "ltdc-picture-expect.test.sh: FAIL"
exit $fail
