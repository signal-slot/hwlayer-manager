#!/bin/bash
# Host test for plane-count-expect.py using inline fixtures.
here=$(cd "$(dirname "$0")" && pwd) || exit 2
script="$here/plane-count-expect.py"
tool() { python3 -I "$script" "$@"; }
tmp=$(mktemp -d) || exit 2
trap 'rm -rf "$tmp"' EXIT
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }

# HVS6-like: 3 crtcs, each with 1 primary, 2 overlays and 1 cursor.
cat > "$tmp/hvs.txt" <<'TXT'
device x, driver y
raw
crtcs
crtc 10 inactive
  properties:
crtc 11 inactive
crtc 12 inactive
planes
plane 101 type primary possible crtcs 0b1 (10)
plane 102 type overlay possible crtcs 0b1 (10)
plane 103 type overlay possible crtcs 0b1 (10)
plane 104 type cursor possible crtcs 0b1 (10)
plane 105 type primary possible crtcs 0b10 (11)
plane 106 type overlay possible crtcs 0b10 (11)
plane 107 type overlay possible crtcs 0b10 (11)
plane 108 type cursor possible crtcs 0b10 (11)
plane 109 type primary possible crtcs 0b100 (12)
plane 110 type overlay possible crtcs 0b100 (12)
plane 111 type overlay possible crtcs 0b100 (12)
plane 112 type cursor possible crtcs 0b100 (12)
TXT

out=$(tool "$tmp/hvs.txt"); rc=$?
[ $rc -eq 0 ] && ok "hvs exit" || bad "hvs exit $rc"
{ grep -qx 'crtcs: 3' <<<"$out" && grep -qx 'planes: 12' <<<"$out"; } && ok hvs-totals || bad hvs-totals
grep -qx '  cursor: 3' <<<"$out" && ok hvs-cursor-total || bad hvs-cursor-total
grep -q '^crtc 11: 4 planes (primary 1, overlay 2, cursor 1)' <<<"$out" && ok hvs-per-crtc || bad hvs-per-crtc
grep -qx 'shared planes: 0' <<<"$out" && ok hvs-no-shared || bad hvs-no-shared
grep -q 'extra planes are cursors: yes' <<<"$out" && ok hvs-extras-cursor || bad hvs-extras-cursor
grep -q 'caps would not hide planes' <<<"$out" && ok hvs-caps || bad hvs-caps

# RK-like: 2 crtcs, 2 primaries, 16 overlays shared by both: 18 planes.
{
echo raw; echo crtcs; echo "crtc 1 inactive"; echo "crtc 2 inactive"; echo planes
echo "plane 51 type primary possible crtcs 0b1 (1)"
echo "plane 52 type primary possible crtcs 0b10 (2)"
for i in $(seq 3 18); do echo "plane $((50+i)) type overlay possible crtcs 0b11 (1 2)"; done
} > "$tmp/rk.txt"
out=$(tool "$tmp/rk.txt"); rc=$?
[ $rc -eq 0 ] && ok "rk exit" || bad "rk exit $rc"
grep -qx 'shared planes: 16' <<<"$out" && ok rk-shared || bad rk-shared
grep -q 'plane 53 overlay: crtcs 1 2' <<<"$out" && ok rk-shared-line || bad rk-shared-line
grep -q 'caps would hide planes' <<<"$out" && ok rk-caps-hide || bad rk-caps-hide
grep -q '^crtc 1: 17 planes' <<<"$out" && ok rk-per-crtc || bad rk-per-crtc

# Non-cursor extra detected.
printf 'raw\ncrtcs\ncrtc 1 inactive\nplanes\nplane 5 type primary possible crtcs 0b1 (1)\nplane 6 type other possible crtcs 0b1 (1)\n' > "$tmp/odd.txt"
out=$(tool "$tmp/odd.txt")
grep -q 'extra planes are cursors: no' <<<"$out" && ok non-cursor || bad non-cursor

# --expect mismatch and match.
tool --expect-planes 13 "$tmp/hvs.txt" >"$tmp/o" 2>&1; rc=$?
{ [ $rc -eq 1 ] && grep -q 'MISMATCH: expected 13 planes, found 12' "$tmp/o"; } && ok mismatch || bad "mismatch rc=$rc"
tool --expect-planes 12 --expect-crtcs 3 "$tmp/hvs.txt" >/dev/null && ok match || bad match
tool --expect-crtcs 2 "$tmp/hvs.txt" >/dev/null; [ $? -eq 1 ] && ok crtc-mismatch || bad crtc-mismatch

# Parse errors and usage.
printf 'device x, driver y\noutput 0: crtc 1\n' > "$tmp/bad.txt"
tool "$tmp/bad.txt" >/dev/null 2>"$tmp/e"; rc=$?
{ [ $rc -eq 2 ] && grep -q "no 'crtcs' section" "$tmp/e"; } && ok parse-error || bad "parse-error rc=$rc"
tool --expect-planes x "$tmp/hvs.txt" >/dev/null 2>&1; [ $? -eq 2 ] && ok bad-number || bad bad-number
tool "$tmp/missing" >/dev/null 2>&1; [ $? -eq 2 ] && ok missing-file || bad missing-file

[ $fail -eq 0 ] && echo "all passed"
exit $fail
