#!/bin/sh
# Link composition-ffi-staticlib into a Cortex-R5 C program (M3-05 prep).
#
# This is a HOST LINK CHECK, not a firmware build: nothing is run, there is
# no Zephyr, no board support and no real start-up code. It only shows that
# libcomposition_engine.a, built for armv7r-none-eabihf with
# `--no-default-features --features panic-handler --release`, resolves
# every symbol a small C program needs when linked with -nostartfiles.
# M3-05 (the link in a real firmware build) stays open.
#
# Usage: scripts/check-ffi-r5f-link.sh
#
# Output: one final line, PASS, "FAIL: <reason>" or "SKIP: <reason>".
# Exit 0 for PASS and SKIP, 1 for FAIL.
#
# Compiler: arm-none-eabi-gcc (-mcpu=cortex-r5 -mfloat-abi=hard
# -mfpu=vfpv3-d16) when present, else clang --target=armv7r-none-eabihf
# with ld.lld or rust-lld. Scratch files go under $TMPDIR.
set -u

root=$(cd "$(dirname "$0")/.." && pwd)
rt=armv7r-none-eabihf
hdr_dir=$root/crates/composition-ffi/include

pass() { echo PASS; exit 0; }
skip() { echo "SKIP: $1"; exit 0; }
fail() { echo "FAIL: $1"; exit 1; }

command -v cargo >/dev/null 2>&1 || skip "cargo not found"
libdir=$(rustc --print target-libdir --target "$rt" 2>/dev/null)
if [ ! -d "$libdir" ]; then
	if command -v rustup >/dev/null 2>&1; then
		rustup target add "$rt" >/dev/null 2>&1 || true
	fi
	libdir=$(rustc --print target-libdir --target "$rt" 2>/dev/null)
	[ -d "$libdir" ] || skip "Rust target $rt is not installed"
fi

tmp=$(mktemp -d "${TMPDIR:-/tmp}/ffi-r5f-link.XXXXXX") || fail "no temporary directory"
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

# (b) a tiny C main that calls a few entry points
cat >"$tmp/main.c" <<'CEOF'
#include "composition_engine.h"

static unsigned long long storage[512];

int main(void)
{
	ce_engine *e = ce_engine_init(storage, sizeof storage, 0, 0);
	int s = 0;
	if (e) {
		s += (int)ce_set_latch_timeout(e, 4);
		s += (int)ce_advance_time(e, 1);
		s += (int)ce_begin_write(e, 0);
		s += (int)ce_buffer_state(e, 0);
	}
	return s;
}

void _start(void)
{
	for (;;)
		(void)main();
}
CEOF

cat >"$tmp/link.ld" <<'LEOF'
ENTRY(_start)
MEMORY { RAM (rwx) : ORIGIN = 0x70000000, LENGTH = 1M }
SECTIONS
{
	.text   : { *(.text .text.*) *(.rodata .rodata.*) } > RAM
	.data   : { *(.data .data.*) } > RAM
	.bss    : { *(.bss .bss.*) *(COMMON) } > RAM
	/DISCARD/ : { *(.ARM.exidx*) *(.ARM.extab*) *(.comment) }
}
LEOF

# (a) the library
(cd "$root" && cargo build --locked -q -p composition-ffi-staticlib \
	--no-default-features --features panic-handler --release --target "$rt") \
	>"$tmp/cargo.log" 2>&1 || {
	tail -n 5 "$tmp/cargo.log"
	fail "cargo build of composition-ffi-staticlib for $rt failed"
}
lib=$root/target/$rt/release/libcomposition_engine.a
[ -f "$lib" ] || fail "$lib was not produced"

# compile and link
if command -v arm-none-eabi-gcc >/dev/null 2>&1; then
	kind=arm-none-eabi-gcc
	nm=arm-none-eabi-nm
	command -v "$nm" >/dev/null 2>&1 || nm=nm
	flags="-mcpu=cortex-r5 -mfloat-abi=hard -mfpu=vfpv3-d16"
	# shellcheck disable=SC2086
	arm-none-eabi-gcc $flags -ffreestanding -O2 -I"$hdr_dir" -c "$tmp/main.c" -o "$tmp/main.o" \
		>"$tmp/cc.log" 2>&1 || { cat "$tmp/cc.log"; fail "$kind could not compile the C main"; }
	# shellcheck disable=SC2086
	arm-none-eabi-gcc $flags -nostdlib -nostartfiles -T "$tmp/link.ld" -Wl,--gc-sections \
		-o "$tmp/image.elf" "$tmp/main.o" "$lib" -lgcc >"$tmp/ld.log" 2>&1
	lrc=$?
elif command -v clang >/dev/null 2>&1; then
	kind=clang
	nm=
	for n in llvm-nm nm; do command -v "$n" >/dev/null 2>&1 && { nm=$n; break; }; done
	lld=$(command -v ld.lld || true)
	if [ -z "$lld" ]; then
		lld=$(ls "$(rustc --print sysroot)"/lib/rustlib/*/bin/rust-lld 2>/dev/null | head -n1)
		[ -n "$lld" ] && lld="$lld -flavor gnu"
	fi
	[ -n "$lld" ] || skip "no ld.lld or rust-lld for clang"
	res=$(clang -print-resource-dir)/include
	clang --target="$rt" -ffreestanding -nostdinc -isystem "$res" -O2 -I"$hdr_dir" \
		-c "$tmp/main.c" -o "$tmp/main.o" >"$tmp/cc.log" 2>&1 ||
		{ cat "$tmp/cc.log"; fail "$kind could not compile the C main"; }
	# shellcheck disable=SC2086
	$lld -nostdlib --gc-sections -T "$tmp/link.ld" -o "$tmp/image.elf" "$tmp/main.o" "$lib" \
		>"$tmp/ld.log" 2>&1
	lrc=$?
else
	skip "neither arm-none-eabi-gcc nor clang found"
fi

echo "compiler: $kind"
if [ "$lrc" -ne 0 ]; then
	undef=$(grep -oE 'undefined (reference to|symbol:) [^A-Za-z0-9_]?[A-Za-z0-9_.$]+' "$tmp/ld.log" |
		sed -E 's/.* //; s/^[^A-Za-z0-9_]//' | sort -u | tr '\n' ' ')
	if [ -n "$undef" ]; then
		echo "undefined symbols: $undef"
		fail "link left undefined symbols: $undef"
	fi
	tail -n 5 "$tmp/ld.log"
	fail "$kind link step failed"
fi
if [ -n "$nm" ]; then
	"$nm" "$tmp/image.elf" >"$tmp/image.nm" 2>/dev/null
	if grep -E '^ +U ' "$tmp/image.nm" >/dev/null; then
		fail "undefined symbols in the image: $(grep -E '^ +U ' "$tmp/image.nm" | tr -s ' ' | tr '\n' ' ')"
	fi
	grep -E ' ce_engine_init$' "$tmp/image.nm" >/dev/null ||
		fail "ce_engine_init is not in the linked image"
fi
echo "undefined symbols: none"
pass
