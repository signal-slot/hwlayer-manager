#!/usr/bin/env bash
# Self-test for trace-to-metrics.py. Fixtures are synthetic.
set -u
cd "$(dirname "$0")"
D=testdata/trace-to-metrics
T="python3 -I trace-to-metrics.py"
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
tmp=$(mktemp -d "${TMPDIR:-.}/t2m.XXXXXX") || exit 1
trap 'rm -rf "$tmp"' EXIT

$T --stage A $D/linux-ok.log >"$tmp/linux.txt" || bad "linux exit"
$T $D/zephyr-wrap.log >"$tmp/zephyr.txt" || bad "zephyr exit"
$T $D/zephyr-no-latch.log >"$tmp/nolatch.txt" || bad "no-latch exit"
$T $D/violation.log >"$tmp/viol.txt" || bad "violation exit"

has() { grep -qF -- "$2" "$1" && ok "$3" || bad "$3"; }
lacks() { grep -qF -- "$2" "$1" && bad "$3" || ok "$3"; }
has "$tmp/linux.txt" "RUN date=2026-01-02 side=linux stage=A tool=trace-to-metrics" "RUN line from log date"
has "$tmp/linux.txt" 'id=M-1.median name="frame interval median" unit=ms value=16.667000' "frame interval (tev)"
has "$tmp/linux.txt" 'id=M-2a.max name="commit-to-latch max" unit=ms value=16.367000' "commit-to-latch max"
has "$tmp/linux.txt" 'id=M-3.dropped name="dropped frames" unit=frames value=1' "dropped"
has "$tmp/linux.txt" 'id=M-3.timeouts name="latch timeouts" unit=events value=1' "latch timeouts"
has "$tmp/linux.txt" 'value=layer 0 plane 31 zpos 0' "plane assignment"
has "$tmp/linux.txt" 'id=M-6.violations name="event order violations" unit=events value=0' "no violations"
has "$tmp/zephyr.txt" 'id=M-1.median name="frame interval median" unit=ms value=16.846000' "cyc32 wrap interval"
has "$tmp/zephyr.txt" 'id=M-2a.median name="commit-to-latch median" unit=ms value=7.875000' "cyc32 wrap latency"
lacks "$tmp/zephyr.txt" "id=M-4" "CPU load never derived"
lacks "$tmp/zephyr.txt" "id=M-2b" "latch-to-light never derived"
has "$tmp/nolatch.txt" "id=M-3.submitted" "no-latch keeps counts"
lacks "$tmp/nolatch.txt" "id=M-1" "no-latch omits M-1"
lacks "$tmp/nolatch.txt" "id=M-2a" "no-latch omits M-2a"
has "$tmp/viol.txt" 'id=M-6.violations name="event order violations" unit=events value=2' "violations counted"

for f in bad-nodate bad-version; do
	$T $D/$f.log >/dev/null 2>&1
	[ $? -eq 2 ] && ok "$f exit 2" || bad "$f exit 2"
done
$T /nonexistent >/dev/null 2>&1
[ $? -eq 2 ] && ok "missing file exit 2" || bad "missing file exit 2"

out=$(python3 -I m2-compare-report.py "$tmp/linux.txt" "$tmp/zephyr.txt") || bad "report exit"
rhas() { grep -qF -- "$1" <<<"$out" && ok "$2" || bad "$2"; }
rhas "(measured on Linux 2026-01-02, Zephyr 2026-01-03)" "report dates"
rhas "| M-1.median frame interval median | 16.667000 ms | 16.846000 ms | +0.179000 ms |" "report M-1 diff"
rhas "| M-3.dropped dropped frames | 1 frames | 0 frames | -1 frames |" "report counts"
rhas "| M-5.plane plane assignment (first validate ok) | layer 0 plane 31 zpos 0 | not measured | not measured |" "report not measured"
exit $fail
