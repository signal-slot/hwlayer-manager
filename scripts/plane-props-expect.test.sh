#!/bin/sh
# Host test for plane-props-expect.py on fixture text shaped like the props
# example's output. Run: bash scripts/plane-props-expect.test.sh

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/plane-props-expect.py
root=$(mktemp -d "${TMPDIR:-.}/ppe-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
py() { python3 -I "$@"; }

cat > "$root/good.txt" <<'F'
device /dev/dri/card0, driver fake 1.0.0 (x), date 2026
crtc 10 (index 0): no mode
    ACTIVE (id 1): range 0..=1 = 0
plane 31: possible_crtcs 0b1 (crtc indices [0]), current crtc None, fb None
    COLOR_ENCODING (id 5): enum {BT.601=0, BT.709=1} = 1 (BT.709)
    COLOR_RANGE (id 6): enum {limited=0, full=1} = 0 (limited)
    SCALING_FILTER (id 7): enum {Default=0, Nearest Neighbor=1} = 0 (Default)
    alpha (id 8): range 0..=65535 = 65535
    pixel blend mode (id 9): enum {None=0, Pre-multiplied=1} = 1 (Pre-multiplied)
    rotation (id 4): bitmask = 63 (rotate-0|rotate-90|rotate-180|rotate-270|reflect-x|reflect-y)
    type (id 2): enum {Overlay=0, Primary=1, Cursor=2} = 0 (Overlay)
    zpos (id 3): range 0..=7 = 2
plane 32: possible_crtcs 0b1 (crtc indices [0]), current crtc None, fb None
    rotation (id 4): bitmask = 1 (rotate-0)
    type (id 2): enum {Overlay=0, Primary=1, Cursor=2} = 1 (Primary)
    zpos (id 3): range 0..=0 [immutable,atomic] = 0
plane 33: possible_crtcs 0b1 (crtc indices [0]), current crtc None, fb None
    type (id 2): enum {Overlay=0, Primary=1, Cursor=2} = 2 (Cursor)
F

py "$tool" "$root/good.txt" > "$root/out.txt" 2> "$root/err.txt"
[ $? -eq 0 ] && ok "exit 0" || bad "exit 0"
[ "$(wc -l < "$root/out.txt")" -eq 3 ] && ok "one line per plane" || bad "one line per plane"
l1=$(sed -n 1p "$root/out.txt"); l2=$(sed -n 2p "$root/out.txt"); l3=$(sed -n 3p "$root/out.txt")
case $l1 in
*"plane 31 overlay"*"rotate-0(Normal), rotate-90(Rotate90), rotate-180(Rotate180), rotate-270(Rotate270), reflect-x(FlipH), reflect-y(FlipV)"*) ok "all rotation bits" ;;
*) bad "all rotation bits: $l1" ;;
esac
case $l1 in
*"zpos 0..=7 mutable"*"alpha yes"*"blend-mode yes"*"color-encoding yes"*"color-range yes"*"scaling-filter yes"*) ok "props present" ;;
*) bad "props present: $l1" ;;
esac
case $l2 in
*"plane 32 primary"*"rotation=1 [rotate-0(Normal)]"*) ok "rotate-0 only" ;;
*) bad "rotate-0 only: $l2" ;;
esac
case $l2 in
*"zpos 0..=0 immutable"*"alpha no"*"scaling-filter no"*) ok "zpos immutable" ;;
*) bad "zpos immutable: $l2" ;;
esac
case $l3 in
*"plane 33 cursor"*"no rotation property"*"no zpos"*) ok "no rotation property" ;;
*) bad "no rotation property: $l3" ;;
esac

printf 'plane 5: possible_crtcs 0b1 (x)\n    zpos garbage line\n' > "$root/bad.txt"
py "$tool" "$root/bad.txt" > /dev/null 2> "$root/err.txt"
if [ $? -eq 2 ] && grep -q "cannot parse" "$root/err.txt"; then ok "parse error is exit 2"; else bad "parse error"; fi
printf 'device x\n' > "$root/none.txt"
py "$tool" "$root/none.txt" > /dev/null 2>&1
[ $? -eq 2 ] && ok "no plane section is exit 2" || bad "no plane section"

exit $fail
