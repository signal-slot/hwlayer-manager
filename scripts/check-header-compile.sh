#!/bin/sh
# Host check: composition_engine.h compiles as C99 and C++17 with warnings
# as errors (and for Cortex-R5 when an ARM cross compiler is on PATH).
# Override the header with HEADER=path (or first argument).
# Last line: PASS, FAIL: <reason> or SKIP: <reason>. Exit 0 for PASS/SKIP.

here=$(cd "$(dirname "$0")" && pwd)
root=$(dirname "$here")
header=${1:-${HEADER:-}}
if [ -z "$header" ]; then
	for h in "$root/include/composition_engine.h" \
		"$root/crates/composition-ffi/include/composition_engine.h"; do
		if [ -f "$h" ]; then header=$h; break; fi
	done
fi
if [ ! -f "$header" ]; then
	echo "FAIL: header not found: ${header:-none}"
	exit 1
fi
header=$(cd "$(dirname "$header")" && pwd)/$(basename "$header")

T=$(mktemp -d "${TMPDIR:-/tmp}/header-compile.XXXXXX") || { echo "FAIL: no temp dir"; exit 1; }
trap 'rm -rf "$T"' EXIT
trap 'exit 1' INT TERM

# Include twice to also check the include guard.
printf '#include "%s"\n#include "%s"\nint main(void) { return 0; }\n' \
	"$header" "$header" > "$T/t.c"
cp "$T/t.c" "$T/t.cpp"

W="-Wall -Wextra -Wpedantic -Werror"
ran=0
try() { # label cmd...
	label=$1; shift
	ran=$((ran + 1))
	if ! "$@" > "$T/out" 2>&1; then
		sed 's/^/  /' "$T/out"
		echo "FAIL: $label"
		exit 1
	fi
	echo "ok: $label"
}

if command -v cc >/dev/null 2>&1; then
	try "cc -std=c99" cc -std=c99 $W -fsyntax-only "$T/t.c"
fi
if command -v c++ >/dev/null 2>&1; then
	try "c++ -std=c++17" c++ -std=c++17 $W -fsyntax-only "$T/t.cpp"
fi
for x in arm-none-eabi-gcc arm-zephyr-eabi-gcc; do
	if command -v "$x" >/dev/null 2>&1; then
		try "$x -mcpu=cortex-r5 -std=c99" "$x" -mcpu=cortex-r5 -std=c99 $W -fsyntax-only "$T/t.c"
		break
	fi
done

if [ "$ran" -eq 0 ]; then
	echo "SKIP: no C compiler (cc, c++) on PATH"
	exit 0
fi
echo PASS
