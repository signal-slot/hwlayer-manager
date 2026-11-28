#!/bin/bash
# Offline test: ltdc-picture-expect.py + camera-check.py on synthetic frames.
# Run: bash scripts/ltdc-picture-synthetic.test.sh
# No hardware is touched; temp files live under $TMPDIR and are removed.
# A synthetic picture checks the tools, it is never a board result.

here=$(cd "$(dirname "$0")" && pwd)
root=$(mktemp -d "${TMPDIR:-.}/lps-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
py() { python3 -I "$@"; }
M=640x480
P=80,60,480x360

# case_ GROUP NAME DRAWN-ARGS WANT(pass|fail) EXPECT-ARGS
case_() {
    local group=$1 name=$2 drawn=$3 want=$4 expect=$5 rc
    # shellcheck disable=SC2086
    py "$here/ltdc-picture-synthetic.py" "$root/$name.ppm" --mode $M $drawn \
        || { echo "FAIL: [$group] $name: draw"; fail=1; return; }
    # shellcheck disable=SC2086
    py "$here/ltdc-picture-expect.py" --mode $M $expect --out "$root" --name "$name" >/dev/null 2>&1 \
        || { echo "FAIL: [$group] $name: expect tool"; fail=1; return; }
    py "$here/camera-check.py" "$root/$name.ppm" "$root/expect-$name.json" >"$root/$name.out" 2>&1
    rc=$?
    if [ "$want" = pass ] && [ $rc -eq 0 ]; then
        echo "ok: [$group] $name: camera-check PASS as wanted"
    elif [ "$want" = fail ] && [ $rc -eq 1 ]; then
        echo "ok: [$group] $name: camera-check FAIL as wanted"
    else
        echo "FAIL: [$group] $name: want $want, camera-check exit $rc"
        cat "$root/$name.out"; fail=1
    fi
}

# Transforms: good frames.
n=0
for t in 0 90 180 270 reflect-x reflect-y 180+reflect-x; do
    n=$((n + 1))
    case_ transform "t$n-good" "--plane $P --transform $t" pass "--plane $P --transform $t"
done
# Transforms: wrong on purpose.
case_ transform wrong-90-as-180 "--plane $P --transform 90" fail "--plane $P --transform 180"
case_ transform wrong-180-as-identity "--plane $P --transform 180" fail "--plane $P --transform 0"
case_ transform wrong-reflect-x-as-y "--plane $P --transform reflect-x" fail "--plane $P --transform reflect-y"

# NV12: good frames, then wrong encoding/range, then swapped colours.
case_ nv12 nv12-601-limited "--plane $P --nv12" pass "--plane $P --nv12"
case_ nv12 nv12-709-full "--plane $P --nv12 --encoding bt709 --range full" pass \
    "--plane $P --nv12 --encoding bt709 --range full"
case_ nv12 nv12-wrong-encoding "--plane $P --nv12 --encoding bt709 --range full" fail "--plane $P --nv12"
case_ nv12 nv12-wrong-swapped "--plane $P --nv12 --transform 180" fail "--plane $P --nv12"

# Scaling: 2x up and 2x down, then a wrong plane position and a wrong turn.
case_ scaling up-good "--plane $P --src 240x180" pass "--plane $P --src 240x180"
case_ scaling down-good "--plane 80,60,240x180 --src 480x360" pass "--plane 80,60,240x180 --src 480x360"
case_ scaling up-wrong-position "--plane 0,0,240x180 --src 120x90" fail "--plane $P --src 240x180"
case_ scaling up-wrong-rotation "--plane $P --src 240x180 --transform 180" fail "--plane $P --src 240x180"

if [ $fail -eq 0 ]; then echo "ltdc-picture-synthetic: all passed"; else echo "ltdc-picture-synthetic: FAILED"; fi
exit $fail
