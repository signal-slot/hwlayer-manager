#!/bin/bash
# Self-test for format-cut-report.py. Run: bash scripts/format-cut-report.test.sh
# Uses the synthetic files in scripts/testdata/format-cut-report/.

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/format-cut-report.py
data=$here/testdata/format-cut-report
root=$(mktemp -d "${TMPDIR:-.}/fcr-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
py() { python3 -I "$tool" "$@"; }

py "$data/big.txt" > "$root/big.out" 2> "$root/big.err"; rc=$?
[ "$rc" -eq 1 ] && ok "big: exit 1 when dropped" || bad "big: exit $rc"
grep -qx 'FORMAT-CUT plane=100 total=45 kept=32 dropped=13 linear_first=no' "$root/big.out" \
    && ok "plane 100 summary" || bad "plane 100 summary"
grep -qx 'FORMAT-CUT plane=101 total=6 kept=6 dropped=0 linear_first=yes' "$root/big.out" \
    && ok "plane 101 summary" || bad "plane 101 summary"
grep -qx 'FORMAT-CUT plane=102 total=3 kept=3 dropped=0 linear_first=yes' "$root/big.out" \
    && ok "plane 102 summary" || bad "plane 102 summary"
# plane 100: the 15 linear pairs are sorted first, then 17 of the 30 AFBC pairs
grep -A1 '^plane 100' "$root/big.out" | grep -q 'kept (32): XR24 AR24' \
    && ok "linear sorted first" || bad "linear sorted first"
grep -A2 '^plane 100' "$root/big.out" | grep -q 'dropped (13):.*:0x800000000000011' \
    && ok "last modifier dropped" || bad "last modifier dropped"

py --max 64 "$data/big.txt" > /dev/null; rc=$?
[ "$rc" -eq 0 ] && ok "--max 64: exit 0" || bad "--max 64: exit $rc"

py "$data/props.txt" > "$root/props.out"; rc=$?
[ "$rc" -eq 0 ] && ok "props: exit 0" || bad "props: exit $rc"
grep -qx 'FORMAT-CUT plane=7 total=6 kept=6 dropped=0 linear_first=no' "$root/props.out" \
    && ok "props summary" || bad "props summary"
py --max 4 "$data/props.txt" > "$root/props4.out"; rc=$?
if grep -qx 'FORMAT-CUT plane=7 total=6 kept=4 dropped=2 linear_first=no' "$root/props4.out" && [ "$rc" -eq 1 ]; then
    ok "props --max 4"
else
    bad "props --max 4"
fi

py "$data/malformed.txt" > /dev/null 2>&1; rc=$?
[ "$rc" -eq 2 ] && ok "malformed: exit 2" || bad "malformed: exit $rc"
py "$data/garbage.txt" > /dev/null 2>&1; rc=$?
[ "$rc" -eq 2 ] && ok "no listing: exit 2" || bad "no listing: exit $rc"
py "$root/missing" > /dev/null 2>&1; rc=$?
[ "$rc" -eq 2 ] && ok "missing file: exit 2" || bad "missing file: exit $rc"
py < "$data/props.txt" > /dev/null; rc=$?
[ "$rc" -eq 0 ] && ok "stdin" || bad "stdin: exit $rc"

grep -q '^    IN_FORMATS: immutable atomic blob, 232 bytes value 9' "$data/big.txt" \
    && ok "big.txt has probe property lines" || bad "big.txt lacks property lines"
grep -q 'IN_FORMATS (id 9): immutable blob' "$data/props.txt" \
    && ok "props.txt has property line" || bad "props.txt lacks property line"
py "$data/big.txt" "$data/props.txt" > /dev/null 2>&1; rc=$?
[ "$rc" -le 1 ] && ok "property lines are skipped (exit $rc)" || bad "property lines: exit $rc"

exit $fail
