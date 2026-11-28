#!/bin/sh
# Runs check-ffi-r5f-link.sh; accepts PASS or SKIP and checks the output format.
here=$(cd "$(dirname "$0")" && pwd)
out=$(sh "$here/check-ffi-r5f-link.sh" 2>&1)
rc=$?
last=$(printf '%s\n' "$out" | tail -n 1)
case $last in
PASS)
	[ "$rc" -eq 0 ] || { echo "FAIL: PASS with exit $rc"; exit 1; } ;;
"SKIP: "?*)
	[ "$rc" -eq 0 ] || { echo "FAIL: SKIP with exit $rc"; exit 1; } ;;
*)
	printf '%s\n' "$out"
	echo "FAIL: last line is not PASS or SKIP: <reason>"
	exit 1 ;;
esac
echo "ok: $last"
