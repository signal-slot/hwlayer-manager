#!/usr/bin/env bash
# Offline test of build-liftoff.sh's commit-pin refusal (fake trees only).
# Run: bash tools/liftoff-crosscheck/build-liftoff.test.sh
# Needs $TMPDIR (not /tmp). No network, no clone; all files live under $TMPDIR.
set -u

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
script="$here/build-liftoff.sh"
other=0123456789abcdef0123456789abcdef01234567

[ -n "${TMPDIR:-}" ] || {
	echo "TMPDIR must be set" >&2
	exit 2
}
root="$(mktemp -d "$TMPDIR/build-liftoff-test.XXXXXX")" || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0

# The paths exist, so the script never fetches; unset the env that could make
# it install meson (pip) or pick another source.
unset CONNECTED_DEPS_DIR LIBLIFTOFF_SRC

fake_tree() { # DIR
	mkdir -p "$1/include"
	: >"$1/meson.build"
	: >"$1/include/libliftoff.h"
}

run() { # NAME args... ; sets rc and err (stderr only)
	local b="$root/build-$1"
	shift
	err="$(timeout 20 bash "$script" --build-dir "$b" "$@" 2>&1 >/dev/null </dev/null)"
	rc=$?
}

pass() { echo "ok   $1"; }
bad() {
	echo "FAIL $1: $2"
	printf '%s\n' "$err"
	fail=1
}

# 1: marker file with a different SHA
t1="$root/src1"
fake_tree "$t1"
echo "$other" >"$t1/.liftoff-crosscheck-commit"
run 1 "$t1"
if [ "$rc" -ne 0 ] && [[ $err == *"not the pinned"* ]] && [[ $err == *"$other"* ]]; then
	pass "case 1 marker with other SHA refused"
else bad "case 1" "rc=$rc"; fi

# 2: no .git, no marker, basename without the pinned SHA
t2="$root/src2"
fake_tree "$t2"
run 2 "$t2"
if [ "$rc" -ne 0 ] && [[ $err == *"not the pinned"* ]]; then
	pass "case 2 unidentifiable tree refused"
else bad "case 2" "rc=$rc"; fi

# 3: local git repo with one commit
t3="$root/src3"
fake_tree "$t3"
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null
if ! { git -C "$t3" init -q . &&
	git -C "$t3" add -A &&
	git -C "$t3" -c user.name=test -c user.email=test@example.invalid commit -q -m init; }; then
	echo "FAIL case 3 setup"
	exit 2
fi
run 3 "$t3"
if [ "$rc" -ne 0 ] && [[ $err == *"not the pinned"* ]]; then
	pass "case 3 git repo at another commit refused"
else bad "case 3" "rc=$rc"; fi

# 4: case 1 tree with --allow-other-commit: warns, no refusal (exit code free)
run 4 --allow-other-commit "$t1"
if [[ $err == *WARNING* ]] && [[ $err != *"not the pinned"* ]]; then
	pass "case 4 --allow-other-commit warns"
else bad "case 4" "rc=$rc"; fi

exit "$fail"
