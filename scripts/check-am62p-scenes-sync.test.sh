#!/bin/sh
# Self-test for check-am62p-scenes-sync.sh. Run: sh scripts/check-am62p-scenes-sync.test.sh
# Checks the real tree (expects PASS), then copies in $TMPDIR with one colour
# changed (expects FAIL), and usage errors (expect 2). The real tree is not changed.

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/.." && pwd)
check=$here/check-am62p-scenes-sync.sh
realsrc=$repo/tests/board/am62p_r5f/src
tmp=$(mktemp -d "${TMPDIR:-/tmp}/scenes-sync.XXXXXX") || exit 2
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
fail=0

expect() { # name want-code got-code
	if [ "$2" = "$3" ]; then
		echo "ok   $1"
	else
		echo "FAIL $1: exit $3, wanted $2"
		fail=1
	fi
}

out=$(sh "$check" "$repo")
rc=$?
expect "real tree" 0 $rc
case $out in
*differs*)
	echo "FAIL real tree reports a difference:"
	echo "$out"
	fail=1
	;;
esac
[ "$(printf '%s\n' "$out" | tail -n 1)" = PASS ] || { echo "FAIL real tree: last line not PASS"; fail=1; }
[ "$(printf '%s\n' "$out" | grep -c '^SYNC [1-8] ok$')" = 8 ] || { echo "FAIL real tree: not 8 ok lines"; fail=1; }

copy() { # dir
	mkdir -p "$1/tests/board/am62p_r5f" "$1/scripts" || exit 2
	cp -R "$realsrc" "$1/tests/board/am62p_r5f/src"
	cp "$repo/scripts/am62p-scenes.py" "$1/scripts/"
}

# One colour changed: the blue application frame becomes green.
copy "$tmp/colour"
sed 's/SCENES_APP_BLUE)/SCENES_APP_GREEN)/' "$realsrc/scenes.c" >"$tmp/colour/tests/board/am62p_r5f/src/scenes.c"
if cmp -s "$realsrc/scenes.c" "$tmp/colour/tests/board/am62p_r5f/src/scenes.c"; then
	echo "FAIL colour mutation did not change the copy"
	fail=1
fi
out=$(sh "$check" "$tmp/colour")
rc=$?
expect "changed colour" 1 $rc
case $out in
*"SYNC 2 differs"*) echo "ok   scene 2 reported" ;;
*)
	echo "FAIL scene 2 not reported"
	fail=1
	;;
esac
case $(printf '%s\n' "$out" | tail -n 1) in
FAIL:*) echo "ok   FAIL line" ;;
*)
	echo "FAIL no FAIL line"
	fail=1
	;;
esac

# A colour value changed in scenes.h.
copy "$tmp/hdr"
sed 's/0x07E0u/0x0700u/' "$realsrc/scenes.h" >"$tmp/hdr/tests/board/am62p_r5f/src/scenes.h"
sh "$check" "$tmp/hdr" >/dev/null
expect "changed scenes.h colour" 1 $?

# Usage errors.
sh "$check" a b >/dev/null 2>&1
expect "two arguments" 2 $?
sh "$check" "$tmp/none" >/dev/null 2>&1
expect "missing directory" 2 $?
sh "$check" --bogus >/dev/null 2>&1
expect "unknown option" 2 $?

[ $fail = 0 ] && echo "check-am62p-scenes-sync.test: all ok"
exit $fail
