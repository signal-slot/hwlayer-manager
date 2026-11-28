#!/bin/sh
# Host test for check-header-compile.sh: bad headers must FAIL, the real one PASS.

here=$(cd "$(dirname "$0")" && pwd)
chk=$here/check-header-compile.sh
T=$(mktemp -d "${TMPDIR:-/tmp}/header-compile-test.XXXXXX") || exit 1
trap 'rm -rf "$T"' EXIT
trap 'exit 1' INT TERM
rc=0

if ! command -v cc >/dev/null 2>&1 && ! command -v c++ >/dev/null 2>&1; then
	echo "SKIP: no compiler"
	exit 0
fi

expect_fail() { # name, out, status
	if [ "$3" -ne 1 ] || [ "$(printf '%s\n' "$2" | tail -n 1 | cut -c1-5)" != "FAIL:" ]; then
		echo "$1: expected FAIL (exit 1), got exit $3: $2"
		rc=1
	fi
}

# Hard error, via the HEADER variable.
printf '#include <stdint.h>\nint broken(void) { return undeclared_symbol; }\n' > "$T/bad.h"
out=$(HEADER="$T/bad.h" sh "$chk" 2>&1); r=$?
expect_fail "error header" "$out" $r

# Warning-only defect (stray semicolon, pedantic), via the argument.
printf '#include <stdint.h>\nint f(void);;\n' > "$T/warn.h"
out=$(sh "$chk" "$T/warn.h" 2>&1); r=$?
expect_fail "warning header" "$out" $r

# Real header.
out=$(sh "$chk" 2>&1); r=$?
printf '%s\n' "$out" | tail -n 3
last=$(printf '%s\n' "$out" | tail -n 1)
case $last in
PASS | SKIP:*) [ $r -eq 0 ] || { echo "real header: exit $r"; rc=1; } ;;
*) echo "real header: expected PASS"; rc=1 ;;
esac

[ $rc -eq 0 ] && echo "check-header-compile.test: OK"
exit $rc
