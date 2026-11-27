#!/usr/bin/env bash
# Self-test of the C side of the libliftoff cross-check: builds libliftoff,
# the fake libdrm and the driver (build-liftoff.sh), then runs the driver on
# testdata/minimal.xcheck with the built-in responders and over the pipe, and
# compares the result lines. Arguments are passed to build-liftoff.sh.
# Needs a C compiler, meson and ninja (or CONNECTED_DEPS_DIR for a venv), and
# network access the first time libliftoff is fetched. Not run in CI.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
build="$here/../build-liftoff.sh"
input="$here/testdata/minimal.xcheck"
origin="synthetic 2026-11-28 self-test topology of the C driver, imitates no device"

fails=0
check() {
	local name="$1" expected="$2" actual="$3"
	if [ "$expected" == "$actual" ]; then
		echo "PASS $name"
	else
		echo "FAIL $name"
		echo "  expected:"
		printf '    %s\n' "$expected"
		echo "  actual:"
		printf '    %s\n' "$actual"
		fails=$((fails + 1))
	fi
}

# Result lines without the tests= count, which depends on libliftoff's search.
results() {
	grep '^result ' | sed 's/ tests=[0-9]* / /'
}

# 1. The build script refuses a checkout at another commit.
scratch="$(mktemp -d "${TMPDIR:?TMPDIR must be set}/liftoff-selftest.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/other/include"
touch "$scratch/other/meson.build" "$scratch/other/include/libliftoff.h"
echo 0000000000000000000000000000000000000000 >"$scratch/other/.liftoff-crosscheck-commit"
if out="$(bash "$build" --build-dir "$scratch/b" "$scratch/other" 2>&1)"; then
	check "build-liftoff.sh refuses another commit" "refused" "accepted"
else
	case "$out" in
	*"not the pinned c4226a79b7a52f59bb51789b1eda112658609125"*)
		check "build-liftoff.sh refuses another commit" "refused" "refused" ;;
	*) check "build-liftoff.sh refuses another commit" "refused" "$out" ;;
	esac
fi

# 2. Build.
driver="$(bash "$build" "$@" | tail -n1)"
[ -x "$driver" ] || {
	echo "FAIL build: no driver binary ('$driver')"
	exit 1
}
echo "driver: $driver"

accept="result frame=one-layer output=40 layers=1 apply=ok placed=1 planes=31 deadline=0 origin=$origin
result frame=two-layers output=40 layers=2 apply=ok placed=2 planes=31,32 deadline=0 origin=$origin"
reject="result frame=one-layer output=40 layers=1 apply=ok placed=0 planes=composition deadline=0 origin=$origin
result frame=two-layers output=40 layers=2 apply=ok placed=0 planes=composition,composition deadline=0 origin=$origin"

# 3. Built-in responders.
out="$("$driver" --accept-all "$input")"
check "--accept-all" "$accept" "$(results <<<"$out")"
grep -q '^test frame=one-layer seq=1 ' <<<"$out" || check "--accept-all prints test requests" "a test line" "none"

out="$("$driver" --reject-all "$input")"
check "--reject-all" "$reject" "$(results <<<"$out")"

# 4. Pipe protocol: answer every request with "ok" from this shell; the
# result must equal --accept-all, and every request must be well formed.
coproc DRIVER { "$driver" "$input"; }
driver_pid=$DRIVER_PID
# Bash drops the DRIVER array when the coprocess exits: keep copies.
exec {from_driver}<&"${DRIVER[0]}" {to_driver}>&"${DRIVER[1]}"
pipe_out=""
bad=""
while IFS= read -r line <&"$from_driver"; do
	case "$line" in
	"test "*)
		[[ "$line" =~ ^test\ frame=[^\ ]+\ seq=[0-9]+\ flags=0x[0-9a-f]+\ planes=[0-9]+( plane=[0-9]+ fb=[0-9]+ crtc=[0-9]+ src16=[0-9,]+ dst=[-0-9,]+ alpha=[0-9]+ blend=[a-z-]+ rotation=0x[0-9a-f]+ zpos=-?[0-9]+)*$ ]] ||
			bad="$line"
		echo ok >&"$to_driver"
		;;
	*) pipe_out+="$line"$'\n' ;;
	esac
done
exec {from_driver}<&- {to_driver}>&-
wait "$driver_pid"
check "pipe answering ok" "$accept" "$(results <<<"$pipe_out")"
check "pipe request grammar" "" "$bad"

if [ "$fails" -ne 0 ]; then
	echo "selftest: $fails check(s) failed"
	exit 1
fi
echo "selftest: all checks passed"
