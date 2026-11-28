#!/usr/bin/env bash
# End-to-end host test: trace logs -> trace-to-metrics.py -> m2-compare-report.py.
# Uses the synthetic fixtures of scripts/testdata/trace-to-metrics (invented
# numbers, not board data; no hardware claim). Neither script is modified.
set -u
cd "$(dirname "$0")"
D=testdata/trace-to-metrics
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
tmp=$(mktemp -d "${TMPDIR:-.}/m2pipe.XXXXXX") || exit 1
trap 'rm -rf "$tmp"' EXIT

python3 -I trace-to-metrics.py $D/linux-ok.log >"$tmp/linux.txt" || bad "convert linux"
python3 -I trace-to-metrics.py $D/zephyr-wrap.log >"$tmp/zephyr.txt" || bad "convert zephyr"
grep -qF "side=linux" "$tmp/linux.txt" && ok "linux log tagged linux" || bad "linux tag"
grep -qF "side=zephyr" "$tmp/zephyr.txt" && ok "zephyr log tagged zephyr" || bad "zephyr tag"

out=$(python3 -I m2-compare-report.py "$tmp/linux.txt" "$tmp/zephyr.txt") || bad "report exit"
rhas() { grep -qF -- "$1" <<<"$out" && ok "$2" || bad "$2"; }
rhas "(measured on Linux 2026-01-02, Zephyr 2026-01-03)" "report dates"
rhas "| M-1.median frame interval median | 16.667000 ms | 16.846000 ms | +0.179 ms |" "M-1 row"
rhas "| M-3.dropped dropped frames | 1 frames | 0 frames | -1 frames |" "M-3 dropped row"
rhas "| M-5.plane plane assignment (first validate ok) | layer 0 plane 31 zpos 0 | not measured | not measured |" "M-5 not measured on one side"

# A log without a RUN date is rejected, at conversion and in the report.
python3 -I trace-to-metrics.py $D/bad-nodate.log >/dev/null 2>&1
[ $? -eq 2 ] && ok "trace-to-metrics rejects log without date" || bad "trace-to-metrics nodate"
grep -v '^RUN' "$tmp/linux.txt" >"$tmp/nodate.txt"
python3 -I m2-compare-report.py "$tmp/nodate.txt" "$tmp/zephyr.txt" >/dev/null 2>&1
[ $? -eq 2 ] && ok "report rejects log without RUN date" || bad "report nodate"
sed 's/ date=[^ ]*//' "$tmp/linux.txt" >"$tmp/nodate2.txt"
python3 -I m2-compare-report.py "$tmp/nodate2.txt" "$tmp/zephyr.txt" >/dev/null 2>&1
[ $? -eq 2 ] && ok "report rejects RUN line without date" || bad "report RUN without date"
exit $fail
