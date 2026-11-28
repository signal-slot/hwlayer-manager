#!/bin/sh
# Self-test for kms-sweep-expect.py.
# Run: sh scripts/kms-sweep-expect.test.sh

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/kms-sweep-expect.py
root=$(mktemp -d "${TMPDIR:-.}/kse-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
py() { python3 -I "$@"; }
c() { # k set planes result
    echo "COMBO k=$1 set=$2 planes=$3 scale=1.0000x1.0000 src=1920.0000x1080.0000 dst=1920x1080 result=$4"
}

{
    echo "DATE 2026-11-28T00:00:00Z"
    echo "KERNEL 6.1.0"
    c 1 0 31 ok
    c 2 0 31,37 ok
    c 2 1 31,43 EINVAL
    c 3 0 31,37,43 ok
    c 4 0 31,37,43,49 EINVAL
    c 4 1 31,37,43,55 ERANGE
    echo "COMBO_MAX accepted_k=3"
    echo "VERDICT kms_sweep: DONE combos"
} > "$root/pass.log"

out=$(py "$tool" "$root/pass.log"); rc=$?
[ $rc -eq 0 ] || bad "pass exit $rc"
if echo "$out" | grep -qx 'SWEEP windows=2 pass=1 fail=1' \
    && echo "$out" | grep -qx 'SWEEP windows=4 pass=0 fail=2' \
    && echo "$out" | grep -qx 'SWEEP max_simultaneous=3'; then ok "per-count lines"; else bad "per-count lines: $out"; fi
if echo "$out" | grep -q '^EXPECT'; then bad "EXPECT without flag"; else ok "no EXPECT without flag"; fi

out=$(py "$tool" "$root/pass.log" --expect-min 3); rc=$?
if [ $rc -eq 0 ] && echo "$out" | grep -qx 'EXPECT ok'; then ok "expect-min met"; else bad "expect-min met"; fi

out=$(py "$tool" "$root/pass.log" --expect-min 8); rc=$?
if [ $rc -eq 1 ] && echo "$out" | grep -qx 'EXPECT fail'; then ok "expect-min missed"; else bad "expect-min missed"; fi

{ c 1 0 31 EBUSY; echo "COMBO_MAX accepted_k=0"; } > "$root/none.log"
out=$(py "$tool" "$root/none.log" --expect-min 1); rc=$?
if [ $rc -eq 1 ] && echo "$out" | grep -qx 'SWEEP max_simultaneous=0'; then ok "all fail"; else bad "all fail"; fi

{ c 1 0 31 ok; echo "COMBO_MAX accepted_k=5"; } > "$root/mis.log"
py "$tool" "$root/mis.log" >/dev/null; rc=$?
if [ $rc -eq 1 ]; then ok "COMBO_MAX mismatch"; else bad "COMBO_MAX mismatch"; fi

echo "COMBO k=2 planes=31" > "$root/m1.log"
echo "just text" > "$root/m2.log"
c 3 0 31,37 ok > "$root/m3.log"
echo "COMBO_MAX accepted_k=x" > "$root/m4.log"
for f in m1 m2 m3 m4 nosuch; do
    py "$tool" "$root/$f.log" >/dev/null 2>&1; rc=$?
    if [ $rc -eq 2 ]; then ok "malformed $f"; else bad "malformed $f"; fi
done
py "$tool" "$root/pass.log" --expect-min x >/dev/null 2>&1; rc=$?
if [ $rc -eq 2 ]; then ok "bad option"; else bad "bad option"; fi

[ $fail -eq 0 ] && echo "all ok"
exit $fail
