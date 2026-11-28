#!/bin/sh
# Self-test for am62p-console-compare.py.
# Run: sh scripts/am62p-console-compare.test.sh
# Fixtures are written under $TMPDIR. The host side is also taken from
# `cargo test -p composition-twoplane board_scenes -- --nocapture` when cargo
# works here; set CONSOLE_COMPARE_NO_CARGO=1 to skip that.

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/am62p-console-compare.py
root=$(mktemp -d "${TMPDIR:-.}/console-compare.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
cmp_() { python3 -I "$tool" "$@"; }
has() { grep -qF -- "$2" "$1" && ok "$3" || bad "$3"; }

names="safety-only app-under-safety app-replaced safety-flipped app-off refused-no-safety refused-blank refused-foreign-safety"
status="0 0 0 0 0 -16 -16 -16"
commits="1 4 5 6 7 7 7 7"
deltas="1 3 1 1 1 0 0 0"
rel="0 1 1 1 1 0 0 0"
gen() { # $1 vsyncs
	i=1
	echo "BEGIN hwlayer-manager board test am62p_r5f"
	for nm in $names; do
		s=$(echo $status | cut -d' ' -f$i); c=$(echo $commits | cut -d' ' -f$i)
		d=$(echo $deltas | cut -d' ' -f$i); r=$(echo $rel | cut -d' ' -f$i)
		if [ "$s" = 0 ]; then p=yes; er=$r; else p=-; er=0; fi
		echo "SCENE $i $nm: status $s (expected $s), commits $c latched $c (+$d, expected +$d), presented $p, released $r (expected $er): ok"
		echo "PANEL $i tl=ff0000"
		i=$((i+1))
	done
	echo "RESULT PASS (8 scenes, vsyncs $1)"
}
gen 61 > "$root/host.txt"

# Board console: prefixes and timestamps, another vsync count and another
# commit base (latched still equal to commits).
gen 480 | sed -e 's/^/[trace] [   12.345] /' -e 's/commits \([0-9]*\) latched \([0-9]*\)/commits 1\1 latched 1\2/' > "$root/board.txt"
cmp_ "$root/board.txt" "$root/host.txt" > "$root/o1"; rc=$?
[ $rc = 0 ] && ok "soft differences pass (exit 0)" || bad "soft differences exit $rc"
has "$root/o1" "COMPARE PASS" "PASS line"
has "$root/o1" "SCENE 2: DIFF soft (not a failure): commits board=14 host=4" "soft diff shown"
has "$root/o1" "RESULT: DIFF soft (not a failure): vsyncs board=480 host=61" "vsyncs soft"

# Identical.
cmp_ "$root/host.txt" "$root/host.txt" > "$root/o2"; rc=$?
[ $rc = 0 ] && ok "identical exits 0" || bad "identical exit $rc"
has "$root/o2" "SCENE 8: MATCH" "MATCH line"
[ "$(grep -c MATCH "$root/o2")" = 9 ] && ok "nine MATCH lines" || bad "nine MATCH lines"

# Hard difference: released count and verdict.
sed 's/released 1 (expected 1): ok/released 0 (expected 1): FAIL/' "$root/host.txt" > "$root/b3"
cmp_ "$root/b3" "$root/host.txt" > "$root/o3"; rc=$?
[ $rc = 1 ] && ok "hard diff exits 1" || bad "hard diff exit $rc"
has "$root/o3" "released board=0 host=1" "both values shown"
has "$root/o3" "COMPARE FAIL" "FAIL line"

# Missing scene and missing result.
grep -v '^SCENE 5 \|^RESULT' "$root/host.txt" > "$root/b4"
cmp_ "$root/b4" "$root/host.txt" > "$root/o4"; rc=$?
[ $rc = 1 ] && ok "missing exits 1" || bad "missing exit $rc"
has "$root/o4" "SCENE 5: MISSING missing on board" "MISSING scene"
has "$root/o4" "RESULT: MISSING missing on board" "MISSING result"

# Held-commits failure only on the board.
cp "$root/host.txt" "$root/b5"
echo "SCENE 3 app-replaced: commits changed while held: 5 -> 6: FAIL" >> "$root/b5"
cmp_ "$root/b5" "$root/host.txt" > "$root/o5"; rc=$?
[ $rc = 1 ] && ok "held failure exits 1" || bad "held exit $rc"
has "$root/o5" "SCENE 3 held: MISSING missing on host" "held line reported"

# Empty and unreadable input.
: > "$root/empty"
cmp_ "$root/empty" "$root/empty" > "$root/o6"; rc=$?
[ $rc = 1 ] && has "$root/o6" "COMPARE FAIL" "empty input fails" || bad "empty exit $rc"
cmp_ "$root/nonexistent" "$root/host.txt" >/dev/null 2>&1; rc=$?
[ $rc = 2 ] && ok "unreadable input exits 2" || bad "unreadable exit $rc"

# The real host console, when it can be produced.
repo=$(cd "$here/.." && pwd)
if command -v cargo >/dev/null 2>&1 && [ -z "$CONSOLE_COMPARE_NO_CARGO" ]; then
	if (cd "$repo" && cargo test -p composition-twoplane board_scenes -- --nocapture) > "$root/cargo.txt" 2>&1; then
		cmp_ "$root/cargo.txt" "$root/cargo.txt" > "$root/o7"; rc=$?
		[ $rc = 0 ] && ok "real host console parses" || bad "real console exit $rc"
		has "$root/o7" "SCENE 8: MATCH" "real console has 8 scenes"
		cmp_ "$root/board.txt" "$root/cargo.txt" > "$root/o8"; rc=$?
		if [ $rc = 0 ]; then ok "fixture board vs real host console"; else bad "fixture vs real host exit $rc"; cat "$root/o8"; fi
	else
		echo "skip: cargo test did not run here"
	fi
else
	echo "skip: cargo not used"
fi

if [ $fail = 0 ]; then echo "all ok"; else echo "FAILED"; fi
exit $fail
