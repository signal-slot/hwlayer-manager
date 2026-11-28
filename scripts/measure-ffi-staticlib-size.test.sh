#!/bin/sh
# Test of scripts/measure-ffi-staticlib-size.sh: output format and the
# --max-text failure path. Needs the thumbv7em-none-eabihf target.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
s=$here/measure-ffi-staticlib-size.sh
out=${TMPDIR:-/tmp}/measure-ffi-size.$$
trap 'rm -f "$out"' EXIT
fail() {
	echo "FAIL: $*" >&2
	exit 1
}

sh "$s" --armv7r >"$out" || fail "plain run failed"
grep -q 'host build size; not a board measurement' "$out" || fail "label"
grep -Eq '^- date: [0-9]{4}-[0-9]{2}-[0-9]{2}$' "$out" || fail "date"
grep -q '^- rustc: rustc ' "$out" || fail "rustc"
grep -Eq '^- commit: [0-9a-f]+' "$out" || fail "commit"
grep -q '^| target | .text | .rodata | .data | .bss |$' "$out" || fail "header"
grep -Eq '^\| thumbv7em-none-eabihf \| [0-9]+ \| [0-9]+ \| [0-9]+ \| [0-9]+ \|$' "$out" || fail "row"
t=$(awk -F'|' '/thumbv7em-none-eabihf/ { gsub(/ /, "", $3); print $3 }' "$out")
[ "$t" -gt 0 ] || fail ".text is zero"
grep -Eq '^\| armv7r-none-eabihf \| ([0-9]+ \| [0-9]+ \| [0-9]+ \| [0-9]+|SKIP \| SKIP \| SKIP \| SKIP) \|$' "$out" || fail "armv7r row"

if sh "$s" --max-text 1 >/dev/null 2>&1; then fail "--max-text 1 did not fail"; fi
sh "$s" --max-text 999999999 >/dev/null 2>&1 || fail "huge limit failed"
if sh "$s" --max-text x >/dev/null 2>&1; then fail "bad number accepted"; fi
echo "PASS"
