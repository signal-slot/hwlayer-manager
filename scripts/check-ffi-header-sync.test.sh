#!/bin/sh
# Test check-ffi-header-sync.sh with fake lib.rs / header pairs.
set -u
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
chk=$here/check-ffi-header-sync.sh
tmp=$(mktemp -d "${TMPDIR:-/tmp}/ffi-sync.XXXXXX") || exit 1
trap 'rm -rf "$tmp"' EXIT
fail=0

rs_base='// pub extern "C" fn ce_commented(x: u32) {}
/* #[no_mangle] pub extern "C" fn ce_block() {} */
#[no_mangle]
pub extern "C" fn ce_a(x: u32) -> u32 { x }
#[no_mangle]
pub unsafe extern "C" fn ce_b(
    p: *mut u8,
) -> i32 { 0 }
struct S { cb: Option<unsafe extern "C" fn(*mut u8) -> i32> }
'
h_base='/* ce_in_comment(void); */
#ifndef X_H_
#define X_H_
#define CE_MACRO(a, b) ((a) + (b))
#ifdef __cplusplus
extern "C" {
#endif
typedef struct { int (*cb)(void *); } ce_ops;
typedef int (*ce_cb)(void *);
unsigned ce_a(unsigned x);
int ce_b(unsigned char *p,
         int q);
#ifdef __cplusplus
}
#endif
#endif
'

# run NAME EXPECTED_EXIT PATTERN
run() {
	name=$1
	want=$2
	pat=$3
	out=$(sh "$chk" --rust "$tmp/l.rs" --header "$tmp/h.h" 2>&1)
	rc=$?
	last=$(printf '%s\n' "$out" | tail -n 1)
	if [ "$want" -eq 0 ]; then verdict="FFI-HEADER PASS"; else verdict="FFI-HEADER FAIL"; fi
	if [ "$rc" -ne "$want" ] || [ "$last" != "$verdict" ] ||
		! printf '%s\n' "$out" | grep -q -- "$pat"; then
		echo "not ok - $name (rc=$rc)"
		printf '%s\n' "$out"
		fail=1
	else
		echo "ok - $name"
	fi
}

printf '%s' "$rs_base" >"$tmp/l.rs"
printf '%s' "$h_base" >"$tmp/h.h"
run match 0 '^FFI-HEADER PASS$'

printf '%s\n#[no_mangle]\npub extern "C" fn ce_new() {}\n' "$rs_base" >"$tmp/l.rs"
run missing-in-header 1 '^FFI-HEADER MISSING-IN-HEADER: ce_new$'

printf '%s' "$rs_base" >"$tmp/l.rs"
printf '%s\nint ce_extra(void);\n' "$h_base" >"$tmp/h.h"
run missing-in-rust 1 '^FFI-HEADER MISSING-IN-RUST: ce_extra$'

printf '%s\n#[unsafe(no_mangle)]\npub unsafe extern "C" fn ce_c() {}\n' "$rs_base" >"$tmp/l.rs"
printf '%s' "$h_base" >"$tmp/h.h"
run unsafe-no-mangle 1 'MISSING-IN-HEADER: ce_c$'

sh "$chk" --rust "$tmp/none" --header "$tmp/h.h" >/dev/null 2>&1
rc=$?
if [ "$rc" -eq 2 ]; then echo "ok - unreadable file"; else echo "not ok - unreadable file (rc=$rc)"; fail=1; fi

[ "$fail" -eq 0 ] && echo "all tests passed"
exit "$fail"
