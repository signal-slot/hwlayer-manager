#!/bin/bash
# Tests for check-m2-status.sh with fixtures in scripts/testdata/check-m2-status.
set -u
here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-m2-status.sh
d=$here/testdata/check-m2-status
fails=0
expect() { # name want_rc pattern feas status
    local out rc
    out=$(sh "$script" "$4" "$5" 2>&1); rc=$?
    if [ "$rc" -ne "$2" ] || ! printf '%s\n' "$out" | grep -qE -- "$3"; then
        echo "FAIL $1 (rc=$rc): $out"
        fails=$((fails + 1))
    else
        echo "pass $1"
    fi
}
expect m2-ok 0 '^ok$' "$d/feas-m2.md" "$d/status-ok.md"
expect m2-missing 1 'no row for M2-02' "$d/feas-m2.md" "$d/status-missing.md"
expect m2-undated 1 "without a YYYY-MM-DD date" "$d/feas-m2.md" "$d/status-undated.md"
expect num-ok 0 '^ok$' "$d/feas-num.md" "$d/status-ok.md"
expect num-missing 1 'no row for feasibility 1' "$d/feas-num.md" "$d/status-missing.md"
expect unreadable 1 'cannot read' "$d/nope.md" "$d/status-ok.md"
out=$(sh "$script" "$d/feas-m2.md" "$d/status-missing.md" 2>&1)
case $out in
    *M2-99*|*M2-77*) echo "FAIL ids outside section 7 used"; fails=$((fails + 1)) ;;
esac
[ "$fails" -eq 0 ] || exit 1
