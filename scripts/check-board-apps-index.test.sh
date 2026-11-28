#!/bin/bash
# Tests for check-board-apps-index.sh using the fixtures in
# testdata/check-board-apps-index/ (one fake repository per case).
set -u
here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-board-apps-index.sh
data=$here/testdata/check-board-apps-index
fails=0

expect() { # name want_rc pattern root
    local out rc
    out=$(sh "$script" "$4" 2>&1); rc=$?
    if [ "$rc" -ne "$2" ] || ! printf '%s\n' "$out" | grep -qE -- "$3"; then
        echo "FAIL $1 (rc=$rc): $out"
        fails=$((fails + 1))
    else
        echo "pass $1"
    fi
}

expect all-listed 0 '^ok$' "$data/all"
expect missing-app 1 '^missing app_b$' "$data/missing"
expect prefix-name 1 '^missing app$' "$data/prefix"
expect missing-boards 1 '^missing boards/' "$data/noboards"
expect no-cmake-ignored 0 '^ok$' "$data/nocmake"
expect no-readme 2 'README.md not found' "$data/noreadme"
expect no-board-dir 2 'tests/board not found' "$data/nodir"

out=$(sh "$script" "$data/missing")
case $out in
*app_a*) echo "FAIL listed app reported"; fails=$((fails + 1)) ;;
esac

if [ "$fails" -ne 0 ]; then
    echo "$fails failure(s)"
    exit 1
fi
echo "all passed"
