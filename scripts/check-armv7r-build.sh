#!/bin/sh
# Host check: the no_std crates build for the AM62P MCU R5F (ARMv7-R).
#
# Usage: scripts/check-armv7r-build.sh [--strict]
#
# CI builds only thumbv7em. The AM62P MCU R5F is a Cortex-R5, so this builds
# for armv7r-none-eabihf:
#   cargo build -p composition-core --target armv7r-none-eabihf
#   cargo build -p composition-twoplane --target armv7r-none-eabihf
# composition-twoplane is `#![no_std]` (its lib.rs is empty; the backend is C),
# so it is built too. Nothing is linked and no C is compiled; the link of the
# static library into C is scripts/check-ffi-r5f-link.sh.
#
# If the Rust target is not installed the script prints
# "ARMV7R SKIP: target not installed" and exits 0; with --strict it exits 1.
# It never installs anything (no `rustup target add`).
#
# Exit: 0 built or skipped, 1 build failed or target missing under --strict,
# 2 bad usage.
set -eu

target=armv7r-none-eabihf
strict=0
if [ $# -gt 1 ]; then
	echo "usage: $0 [--strict]" >&2
	exit 2
fi
if [ $# -eq 1 ]; then
	if [ "$1" = --strict ]; then
		strict=1
	else
		echo "usage: $0 [--strict]" >&2
		exit 2
	fi
fi

root=$(cd "$(dirname "$0")/.." && pwd)

# Whole-line match: "armv7r-none-eabihf" must not match a longer name.
if ! rustup target list --installed | grep -Fx -- "$target" >/dev/null; then
	if [ "$strict" -eq 1 ]; then
		echo "ARMV7R FAIL: target not installed ($target; --strict)" >&2
		exit 1
	fi
	echo "ARMV7R SKIP: target not installed"
	exit 0
fi

cd "$root"
for pkg in composition-core composition-twoplane; do
	echo "== cargo build -p $pkg --target $target"
	if ! cargo build -p "$pkg" --target "$target"; then
		echo "ARMV7R FAIL: $pkg does not build for $target" >&2
		exit 1
	fi
done
echo "ARMV7R PASS: composition-core composition-twoplane build for $target"
