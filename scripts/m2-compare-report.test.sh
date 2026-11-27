#!/usr/bin/env bash
# Self-test for m2-compare-report.py. Fixtures are synthetic.
set -u
cd "$(dirname "$0")"
D=testdata/m2-compare-report
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
out=$(python3 -I m2-compare-report.py $D/linux.txt $D/zephyr.txt) || bad "exit code"
has() { grep -qF -- "$1" <<<"$out" && ok "$2" || bad "$2"; }
has "(measured on Linux 2026-01-02, Zephyr 2026-01-03)" "date header"
has "| M-1.median frame interval median | 16.67 ms | 16.70 ms | +0.03 ms |" "numeric diff"
has "| M-1.max frame interval max | 17.5 ms | 17500 us | units differ |" "unit mismatch"
has "| M-2a.p95 submit-to-latch p95 | 8.2 ms | not measured | not measured |" "missing on Zephyr"
has "| M-2b.light latch-to-light | not measured | not measurable (no photodiode) | not measured |" "missing on Linux"
has "| M-3.dropped dropped frames | 0 frames | 2 frames | +2 frames |" "counts"
has "| M-5.plane plane assignment | plane 31 primary, plane 42 overlay | layer 0 | n/a |" "text values"
has "| M-7.mem memory | not measured | not measured | not measured |" "empty value"
python3 -I m2-compare-report.py --json $D/linux.txt $D/zephyr.txt | python3 -I -c 'import json,sys; d=json.load(sys.stdin); assert len(d["rows"])==7 and d["zephyr_dates"]==["2026-01-03"]' && ok json || bad json
python3 -I m2-compare-report.py $D/linux-nodate.txt $D/zephyr.txt >/dev/null 2>&1
[ $? -eq 2 ] && ok "missing date exit 2" || bad "missing date exit 2"
python3 -I m2-compare-report.py $D/zephyr.txt $D/zephyr.txt >/dev/null 2>&1
[ $? -eq 2 ] && ok "wrong side exit 2" || bad "wrong side exit 2"
python3 -I m2-compare-report.py /nonexistent $D/zephyr.txt >/dev/null 2>&1
[ $? -eq 2 ] && ok "exit 2" || bad "exit 2"
exit $fail
