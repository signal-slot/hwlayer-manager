#!/bin/sh
# Link the AM62P R5F board test's C with the engine into a Cortex-R5 ELF.
#
# This is a HOST LINK CHECK, not a firmware build: nothing is run, there is
# no Zephyr, no board support, no real start-up code and no board memory
# map. It only shows that the board test's scene sequence, the two-plane
# backend and libcomposition_engine.a (armv7r-none-eabihf,
# `--no-default-features --features panic-handler --release`) resolve every
# symbol when linked with -nostartfiles into a freestanding ELF.
# M3-05 (the link in a real firmware build) stays open.
#
# Usage: scripts/check-am62p-r5f-elf.sh
#
# It sits between scripts/check-am62p-r5f-c.sh (compiles the C, links
# nothing) and scripts/check-ffi-r5f-link.sh (links the library into a tiny
# C program). Compiled here, with the flags of check-am62p-r5f-c.sh:
#   tests/board/am62p_r5f/src/scenes.c, backends/twoplane/ce_twoplane.c and a
#   stub entry point with a stub driver, written in this script's scratch
#   directory. host/fake_panel.c is not used: it needs <stdio.h>, so it does
#   not build freestanding.
#
# Output: a final line PASS, "FAIL: <reason>" or "SKIP: <reason>".
# Exit 0 for PASS and SKIP, 1 for FAIL. Nothing is ever installed.
# Linker: ld.lld, else rust-lld, else arm-none-eabi-gcc (as the driver).
# Scratch files go under $TMPDIR.
set -u

root=$(cd "$(dirname "$0")/.." && pwd)
rt=armv7r-none-eabihf
app=$root/tests/board/am62p_r5f/src
tp=$root/backends/twoplane

pass() { echo PASS; exit 0; }
skip() { echo "SKIP: $1"; exit 0; }
fail() { echo "FAIL: $1"; exit 1; }

command -v clang >/dev/null 2>&1 || skip "clang not found"
command -v cargo >/dev/null 2>&1 || skip "cargo not found"
command -v rustc >/dev/null 2>&1 || skip "rustc not found"
libdir=$(rustc --print target-libdir --target "$rt" 2>/dev/null)
[ -d "$libdir" ] || skip "Rust target $rt is not installed"

nm=
for n in llvm-nm nm; do command -v "$n" >/dev/null 2>&1 && { nm=$n; break; }; done
[ -n "$nm" ] || skip "no llvm-nm or nm"

# The linker, the same choice as check-ffi-r5f-link.sh, plus gcc as a driver.
ldkind=
lld=
if command -v ld.lld >/dev/null 2>&1; then
	ldkind=lld
	lld=ld.lld
else
	lld=$(ls "$(rustc --print sysroot)"/lib/rustlib/*/bin/rust-lld 2>/dev/null | head -n1)
	if [ -n "$lld" ]; then
		ldkind=lld
		lld="$lld -flavor gnu"
	elif command -v arm-none-eabi-gcc >/dev/null 2>&1; then
		ldkind=gcc
	fi
fi
[ -n "$ldkind" ] || skip "no ld.lld, rust-lld or arm-none-eabi-gcc"

tmp=$(mktemp -d "${TMPDIR:-/tmp}/am62p-r5f-elf.XXXXXX") || fail "no temporary directory"
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

res=$(clang -print-resource-dir 2>/dev/null)/include
set -- --target=$rt -mcpu=cortex-r5 -mfpu=vfpv3-d16 -mfloat-abi=hard -std=c11 \
	-Wall -Wextra -Werror -pedantic -ffreestanding -nostdinc -isystem "$res" -O2 \
	-I"$root/crates/composition-ffi/include" -I"$tp" -I"$app"

# A clang without the ARM backend cannot do this: skip, not fail.
echo 'int ce_probe;' >"$tmp/probe.c"
clang "$@" -c "$tmp/probe.c" -o "$tmp/probe.o" 2>"$tmp/probe.err" ||
	skip "clang cannot compile for $rt ($(head -n1 "$tmp/probe.err"))"

# The stub driver and entry point. Every driver call succeeds and does
# nothing; this only has to give the linker every symbol, not to run.
cat >"$tmp/stub_main.c" <<'CEOF'
#include "scenes.h"

static int dev_token;
static ce_twoplane_vsync_cb_t cb;
static void *cb_user;
static uint32_t commits;
static uint32_t pix[4];
static uint16_t app_mem[2][4];

static bool s_format_ok(uint32_t f) { return f != 0; }
static int s_set_app(const void *d, const struct ce_twoplane_buf *b) { (void)d; (void)b; commits++; return 0; }
static int s_enable(const void *d, bool on) { (void)d; (void)on; commits++; return 0; }
static int s_flip(const void *d) { (void)d; commits++; return 0; }
static void *s_buffer(const void *d, uint32_t *w, uint32_t *h, uint32_t *s)
{
	(void)d; *w = 2; *h = 2; *s = 8;
	return pix;
}
static void *s_front(const void *d) { (void)d; return pix; }
static int s_premul(const void *d, bool on) { (void)d; (void)on; return 0; }
static uint32_t s_count(const void *d) { (void)d; return commits; }
static void s_set_cb(const void *d, ce_twoplane_vsync_cb_t c, void *u)
{
	(void)d; cb = c; cb_user = u;
}

static const struct ce_twoplane_hw hw = {
	.app_format_ok = s_format_ok,
	.set_app_buffer = s_set_app,
	.enable_app_plane = s_enable,
	.safety_flip = s_flip,
	.safety_buffer = s_buffer,
	.safety_front = s_front,
	.safety_premultiplied = s_premul,
	.commit_count = s_count,
	.latched_count = s_count,
	.set_vsync_callback = s_set_cb,
};

static unsigned int e_lock(void) { return 0; }
static void e_unlock(unsigned int k) { (void)k; }
static void e_wake(void *a) { (void)a; }
static bool e_ready(const void *d) { return d == &dev_token; }
static void e_wait(uint32_t ms) { (void)ms; if (cb) cb(&dev_token, cb_user); }
static void e_hold(uint32_t ms, unsigned scene) { (void)ms; (void)scene; }
static void e_flush(void *p, size_t n) { (void)p; (void)n; }
static void e_log(const char *fmt, ...) { (void)fmt; }

int main(void)
{
	const struct scenes_env env = {
		.hw = &hw, .dev = &dev_token,
		.irq_lock = e_lock, .irq_unlock = e_unlock,
		.wake = e_wake, .device_ready = e_ready,
		.wait_wake = e_wait, .hold = e_hold, .flush = e_flush, .log = e_log,
		.app_mem = {app_mem[0], app_mem[1]}, .app_size = sizeof app_mem[0],
		.hold_ms = 1,
	};
	return scenes_run(&env) ? 0 : 1;
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

for pair in "$app/scenes.c:scenes" "$tp/ce_twoplane.c:ce_twoplane" "$tmp/stub_main.c:stub_main"; do
	src=${pair%:*}
	o=${pair##*:}
	clang "$@" -c "$src" -o "$tmp/$o.o" >"$tmp/cc.log" 2>&1 || {
		tail -n 8 "$tmp/cc.log"
		fail "clang could not compile $(basename "$src") for $rt"
	}
done

# The library, as the firmware would get it.
(cd "$root" && cargo build --locked -q -p composition-ffi-staticlib \
	--no-default-features --features panic-handler --release --target "$rt") \
	>"$tmp/cargo.log" 2>&1 || {
	tail -n 5 "$tmp/cargo.log"
	fail "cargo build of composition-ffi-staticlib for $rt failed"
}
tdir=${CARGO_TARGET_DIR:-target}
case $tdir in /*) ;; *) tdir=$root/$tdir ;; esac
lib=$tdir/$rt/release/libcomposition_engine.a
[ -f "$lib" ] || fail "$lib was not produced"

objs="$tmp/scenes.o $tmp/ce_twoplane.o $tmp/stub_main.o"
if [ "$ldkind" = lld ]; then
	# shellcheck disable=SC2086
	$lld -nostdlib --gc-sections -T "$tmp/link.ld" -o "$tmp/image.elf" $objs "$lib" \
		>"$tmp/ld.log" 2>&1
else
	# shellcheck disable=SC2086
	arm-none-eabi-gcc -mcpu=cortex-r5 -mfloat-abi=hard -mfpu=vfpv3-d16 -nostdlib \
		-nostartfiles -T "$tmp/link.ld" -Wl,--gc-sections -o "$tmp/image.elf" \
		$objs "$lib" -lgcc >"$tmp/ld.log" 2>&1
fi
lrc=$?

echo "linker: $ldkind"
if [ "$lrc" -ne 0 ]; then
	undef=$(grep -oE 'undefined (reference to|symbol:) [^A-Za-z0-9_]?[A-Za-z0-9_.$]+' "$tmp/ld.log" |
		sed -E 's/.* //; s/^[^A-Za-z0-9_]//' | sort -u | tr '\n' ' ')
	if [ -n "$undef" ]; then
		echo "undefined symbols: $undef"
		fail "link left undefined symbols: $undef"
	fi
	tail -n 5 "$tmp/ld.log"
	fail "link step failed"
fi
"$nm" "$tmp/image.elf" >"$tmp/image.nm" 2>/dev/null || fail "$nm could not read the linked image"
if grep -E '^ +U ' "$tmp/image.nm" >/dev/null; then
	fail "undefined symbols in the image: $(grep -E '^ +U ' "$tmp/image.nm" | tr -s ' ' | tr '\n' ' ')"
fi
for sym in scenes_run ce_twoplane_init ce_engine_init; do
	grep -E " $sym\$" "$tmp/image.nm" >/dev/null || fail "$sym is not in the linked image"
done
echo "undefined symbols: none"
pass
