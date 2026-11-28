#!/bin/sh
# Runs check-trace-log.sh and trace-to-metrics.py on the sample logs of
# scripts/testdata/trace-samples/. Run: sh scripts/trace-samples-consumers.test.sh
# The samples carry no '# date:' line (both tools require one), so every
# sample is fed as a copy with "# date: 2026-11-28" prepended; the undated
# original is also run, to pin that the missing date is why it is rejected.
# Each tool is asserted only on what it documents: check-trace-log.sh checks
# the header shape (version, date, host-run mark), trace-to-metrics.py checks
# date, one hdr, src=, version, numeric fields. Neither checks the R1/R2
# relations or event names (that is the strict Rust parser's job).
here=$(cd "$(dirname "$0")" && pwd)
s=$here/testdata/trace-samples
tmp=$(mktemp -d "${TMPDIR:-/var/tmp}/trace-samples-consumers.XXXXXX") || exit 2
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
fail=0
ko() { echo "FAIL $*"; fail=1; }
dated() { printf '# date: 2026-11-28\n' >"$tmp/$1"; cat "$s/$1" >>"$tmp/$1"; }
chk() { # name expected-rc file  (check-trace-log.sh)
    sh "$here/check-trace-log.sh" "$3" >"$tmp/out" 2>/dev/null; rc=$?
    [ "$rc" = "$2" ] || ko "check-trace-log $1: rc=$rc want $2: $(cat "$tmp/out")"
}
met() { # name expected-rc file  (trace-to-metrics.py)
    python3 -I "$here/trace-to-metrics.py" "$3" >"$tmp/out" 2>"$tmp/err"; rc=$?
    [ "$rc" = "$2" ] || ko "trace-to-metrics $1: rc=$rc want $2: $(cat "$tmp/err")"
}
for f in hold.log modeset.log revoke.log stop.log; do
    dated "$f"
    chk "$f undated" 1 "$s/$f"
    met "$f undated" 2 "$s/$f"
    chk "$f" 0 "$tmp/$f"
    met "$f" 0 "$tmp/$f"
    grep -q '^RUN date=2026-11-28 side=linux ' "$tmp/out" || ko "trace-to-metrics $f: no RUN line"
done
# recovery.log holds two runs, each starting with its own ev=hdr (seq=0).
# check-trace-log.sh accepts it; trace-to-metrics.py rejects a second hdr
# ("second hdr line", exit 2): READER DISAGREEMENT, it converts one run per log.
dated recovery.log
chk recovery 0 "$tmp/recovery.log"
met "recovery (two hdr lines)" 2 "$tmp/recovery.log"
grep -q 'second hdr' "$tmp/err" || ko "trace-to-metrics recovery: expected 'second hdr' message"
# Bad samples. Only unknown-version and malformed-line break a rule that a
# tool documents; the others (R1, R2, R3 event names) are relations of the
# strict Rust parser and neither shell nor Python tool claims to check them.
dated bad-unknown-version.log
chk bad-unknown-version 1 "$tmp/bad-unknown-version.log"
met bad-unknown-version 2 "$tmp/bad-unknown-version.log"
dated bad-malformed-line.log
met bad-malformed-line 2 "$tmp/bad-malformed-line.log"   # t=abc is not unsigned
chk bad-malformed-line 0 "$tmp/bad-malformed-line.log"   # header shape only
# Not asserted (no claim): only that the tools do not crash on them.
for f in bad-released-exceeds-commits.log bad-missing-latch.log bad-unknown-event.log; do
    dated "$f"
    sh "$here/check-trace-log.sh" "$tmp/$f" >/dev/null 2>&1; [ $? -le 1 ] || ko "check-trace-log $f crashed"
    python3 -I "$here/trace-to-metrics.py" "$tmp/$f" >/dev/null 2>&1; [ $? -le 2 ] || ko "trace-to-metrics $f crashed"
done
[ $fail = 0 ] && echo "trace-samples-consumers.test.sh: all passed"
exit $fail
