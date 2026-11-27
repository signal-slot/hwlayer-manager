#!/bin/sh
# Self-test for decode-modifiers.py.
# Run: bash scripts/decode-modifiers.test.sh
# Compares the output on the synthetic inputs in scripts/testdata/decode-modifiers/
# with the .expected files (pass --regen to rewrite them after a reviewed change).

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/decode-modifiers.py
data=$here/testdata/decode-modifiers
root=$(mktemp -d "${TMPDIR:-.}/decmod-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
py() { python3 -I "$tool" "$@"; }

# arguments: every family, hex and decimal
py $(cat "$data/values.txt") > "$root/values.out"
# stdin: tokens picked out of a dump
py < "$data/dump.txt" > "$root/dump.out"

if [ "$1" = "--regen" ]; then
    cp "$root/values.out" "$data/values.expected"
    cp "$root/dump.out" "$data/dump.expected"
fi

for n in values dump; do
    if diff -u "$data/$n.expected" "$root/$n.out"; then ok "$n output"; else bad "$n output"; fi
done

# line format
if grep -qvE '^MODIFIER 0x[0-9a-f]{16} .+$' "$root/values.out" "$root/dump.out"; then
    bad "line format"
else
    ok "line format"
fi

# the Pi 5 conflict check: raw Broadcom codes 1, 3, 4, 5
got=$(py 0x0700000000000001 0x0700000000000003 0x0700000000000004 0x0700000000000005 | cut -d' ' -f3 | tr '\n' ' ')
want="BROADCOM_VC4_T_TILED BROADCOM_SAND64 BROADCOM_SAND128 BROADCOM_SAND256 "
if [ "$got" = "$want" ]; then ok "broadcom codes 1,3,4,5"; else bad "broadcom codes: $got"; fi

# every family and the unknown vendor are present in the expected data
for pat in LINEAR INVALID ARM_AFBC ARM_AFRC ARM_16X16_BLOCK BROADCOM_VC4_T_TILED \
    BROADCOM_SAND32 BROADCOM_SAND64 BROADCOM_SAND128 BROADCOM_SAND256 BROADCOM_UIF \
    'param=' 'vendor-0x42' 'INTEL 0x'; do
    if grep -q "$pat" "$data/values.expected"; then ok "covers $pat"; else bad "covers $pat"; fi
done

# bad input is refused
if py zzz 2>/dev/null; then bad "bad value accepted"; else ok "bad value refused"; fi

exit $fail
