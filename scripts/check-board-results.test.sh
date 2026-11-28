#!/bin/sh
# Self-test for check-board-results.sh.
# Run: sh scripts/check-board-results.test.sh

here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-board-results.sh
data=$here/testdata/board-results
fail=0

if sh "$script" "$data/good.md" >/dev/null; then echo "ok   good.md"
else echo "FAIL good.md should pass"; fail=1; fi

if sh "$script" "$data/good-primary.md" >/dev/null; then echo "ok   good-primary.md"
else echo "FAIL good-primary.md should pass"; fail=1; fi

# expect FILE PATTERN: exactly one problem line, matching PATTERN, exit 1.
expect() {
    out=$(sh "$script" "$data/$1"); rc=$?
    n=$(printf '%s\n' "$out" | grep -c .)
    if [ "$rc" -eq 1 ] && [ "$n" -eq 1 ] && printf '%s\n' "$out" | grep -q "$2"; then
        echo "ok   $1"
    else
        echo "FAIL $1 (rc=$rc): $out"; fail=1
    fi
}
expect missing-row.md 'board "nanopc-t6": no row A3'
expect missing-time.md 'row A2: host time'
expect missing-tag.md 'row A1: tag'
expect missing-disclaimer.md 'missing disclaimer'
expect half-disclaimer.md 'missing disclaimer'
expect unknown-verdict.md 'row A1: verdict "MAYBE"'

sh "$script" >/dev/null 2>&1
if [ $? -eq 2 ]; then echo "ok   usage"; else echo "FAIL usage"; fail=1; fi
sh "$script" "$data/nope.md" >/dev/null 2>&1
if [ $? -eq 2 ]; then echo "ok   not a file"; else echo "FAIL not a file"; fail=1; fi
exit "$fail"
