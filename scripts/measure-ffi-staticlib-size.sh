#!/bin/sh
# Host measurement: code size of composition-ffi-staticlib for Cortex-M/R.
#
# Usage: scripts/measure-ffi-staticlib-size.sh [--armv7r] [--max-text N]
#
# Builds the static library with the project's command
#   cargo build -p composition-ffi-staticlib --no-default-features \
#     --features panic-handler --target thumbv7em-none-eabihf --release
# and prints the total .text, .rodata, .data and .bss of the archive as a
# markdown table (section sizes summed over all members, `size -A`).
# With --armv7r the same is done for armv7r-none-eabihf when that target is
# installed; otherwise the row says SKIP. No target is ever installed.
# --max-text N: exit 1 when .text of any measured target exceeds N bytes.
#
# This is a host build size, not a board measurement. Section sizes of an
# unlinked archive are an upper bound: the linker drops unused sections.
#
# Size tool: llvm-size, rust-size or size, whichever is found first.
# Exit: 0 ok, 1 build/measure failed or limit exceeded, 2 bad usage.
set -eu

usage() {
	echo "usage: $0 [--armv7r] [--max-text N]" >&2
	exit 2
}

armv7r=0
max_text=
while [ $# -gt 0 ]; do
	case $1 in
	--armv7r) armv7r=1 ;;
	--max-text)
		[ $# -ge 2 ] || usage
		max_text=$2
		shift
		case $max_text in
		'' | *[!0-9]*) usage ;;
		esac
		;;
	*) usage ;;
	esac
	shift
done

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

sizetool=
for t in llvm-size rust-size size; do
	if command -v "$t" >/dev/null 2>&1; then
		sizetool=$t
		break
	fi
done
if [ -z "$sizetool" ]; then
	# distro-versioned llvm, e.g. /usr/lib/llvm/23/bin/llvm-size
	for p in /usr/lib/llvm/*/bin/llvm-size /usr/lib/llvm-*/bin/llvm-size; do
		if [ -x "$p" ]; then sizetool=$p; fi
	done
fi
if [ -z "$sizetool" ]; then
	echo "no size tool found (size, llvm-size, rust-size)" >&2
	exit 1
fi

tdir=${CARGO_TARGET_DIR:-$root/target}
over=0

echo "## FFI static library size (host build size; not a board measurement)"
echo
echo "- date: $(date -u +%Y-%m-%d)"
echo "- rustc: $(rustc --version)"
echo "- commit: $(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
echo "- size tool: $(basename "$sizetool")"
echo
echo "| target | .text | .rodata | .data | .bss |"
echo "| --- | ---: | ---: | ---: | ---: |"

measure() {
	target=$1
	if ! cargo build -q -p composition-ffi-staticlib --no-default-features \
		--features panic-handler --target "$target" --release >&2; then
		echo "build for $target failed" >&2
		return 1
	fi
	lib=$tdir/$target/release/libcomposition_engine.a
	if [ ! -f "$lib" ]; then
		echo "missing $lib" >&2
		return 1
	fi
	sums=$("$sizetool" -A "$lib" | awk '
		/^\.text/ { t += $2; next }
		/^\.rodata/ { r += $2; next }
		/^\.data/ { d += $2; next }
		/^\.bss/ { b += $2; next }
		END { printf "%d %d %d %d", t, r, d, b }') || return 1
	set -- $sums
	echo "| $target | $1 | $2 | $3 | $4 |"
	if [ -n "$max_text" ] && [ "$1" -gt "$max_text" ]; then
		echo "FAIL: $target .text $1 exceeds --max-text $max_text" >&2
		over=1
	fi
}

measure thumbv7em-none-eabihf || exit 1

if [ "$armv7r" = 1 ]; then
	libdir=$(rustc --print target-libdir --target armv7r-none-eabihf 2>/dev/null || true)
	if [ -n "$libdir" ] && ls "$libdir"/libcore-*.rlib >/dev/null 2>&1; then
		measure armv7r-none-eabihf || exit 1
	else
		echo "| armv7r-none-eabihf | SKIP | SKIP | SKIP | SKIP |"
		echo "armv7r-none-eabihf: SKIP (target not installed)" >&2
	fi
fi

echo
echo "Host build size; not a board measurement."
[ "$over" = 0 ] || exit 1
