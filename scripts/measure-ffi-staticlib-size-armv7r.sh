#!/bin/sh
# Host measurement: composition-ffi-staticlib size for the AM62P MCU R5F.
#
# Usage: scripts/measure-ffi-staticlib-size-armv7r.sh
#
# Builds, into the workspace target directory (CARGO_TARGET_DIR if set):
#   cargo build -p composition-ffi-staticlib --no-default-features \
#     --features panic-handler --target armv7r-none-eabihf --release
# and sums text, data and bss over all archive members with size, llvm-size
# or rust-size (first one found). The result is an input for M3-05. It is a
# host build, not a hardware measurement; the archive is not a linked image.
#
# If the Rust target is not installed the script prints
# "SKIP: target not installed" and exits 0. It never installs anything.
#
# Exit: 0 measured or skipped, 1 build or measurement failed, 2 bad usage.
set -eu

target=armv7r-none-eabihf
lib=libcomposition_engine.a

if [ $# -ne 0 ]; then
	echo "usage: $0" >&2
	exit 2
fi

root=$(cd "$(dirname "$0")/.." && pwd)

# Whole-line match: "armv7r-none-eabihf" must not match a longer name.
if ! rustup target list --installed | grep -Fx -- "$target" >/dev/null; then
	echo "SKIP: target not installed"
	exit 0
fi

sizetool=
for t in size llvm-size rust-size; do
	if command -v "$t" >/dev/null 2>&1; then
		sizetool=$t
		break
	fi
done
if [ -z "$sizetool" ]; then
	echo "FAIL: no size tool (size, llvm-size, rust-size)" >&2
	exit 1
fi

cd "$root"
echo "== cargo build -p composition-ffi-staticlib --target $target --release"
if ! cargo build -p composition-ffi-staticlib --no-default-features \
	--features panic-handler --target "$target" --release; then
	echo "FAIL: composition-ffi-staticlib does not build for $target" >&2
	exit 1
fi

tdir=${CARGO_TARGET_DIR:-$root/target}
archive=$tdir/$target/release/$lib
if [ ! -f "$archive" ]; then
	echo "FAIL: archive not found: $archive" >&2
	exit 1
fi

# Berkeley format: one line per member, "text data bss dec hex filename".
# Header lines and non-numeric lines are skipped.
if ! sums=$("$sizetool" "$archive" | awk '
	$1 ~ /^[0-9]+$/ && $2 ~ /^[0-9]+$/ && $3 ~ /^[0-9]+$/ {
		t += $1; d += $2; b += $3; n++
	}
	END { if (n == 0) exit 1; printf "%d %d %d\n", t, d, b }'); then
	echo "FAIL: no sizes read from $archive with $sizetool" >&2
	exit 1
fi
set -- $sums
echo "composition-ffi-staticlib $target release: text=$1 data=$2 bss=$3 bytes ($sizetool, summed over archive members)"
echo "$(date +%Y-%m-%d): host build, not measured on hardware"
