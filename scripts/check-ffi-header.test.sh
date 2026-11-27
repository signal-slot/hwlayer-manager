#!/usr/bin/env bash
# Tests for scripts/check-ffi-header.sh using a throwaway git repository.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-ffi-header.sh
tmp=$(mktemp -d "${TMPDIR:-/tmp}/ffi-header-test.XXXXXX")
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"
git init -q .
git config user.email t@example.invalid
git config user.name t
git config commit.gpgsign false
mkdir -p crates/composition-ffi/src crates/composition-ffi/include
LIB=crates/composition-ffi/src/lib.rs
HDR=crates/composition-ffi/include/composition_engine.h

cat >"$LIB" <<'R'
//! FFI.
#[repr(C)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
}

/// Adds.
#[no_mangle]
pub extern "C" fn ce_add(a: i32,
    b: i32) -> i32 {
    a + b
}

fn helper() -> i32 { 1 }
R
echo "// header" >"$HDR"
git add -A
git commit -qm base
git branch -q base

fails=0
# check_last <label> <ok|fail>: run the checker on the newest commit only
check_last() {
    local out got
    out=$(bash "$script" HEAD~1 2>&1) && got=ok || got=fail
    if [ "$got" = "$2" ] && printf '%s\n' "$out" | grep -q "^$2 "; then
        echo "pass: $1"
    else
        echo "FAIL: $1 (wanted $2, got: $out)"
        fails=$((fails + 1))
    fi
}

sed -i 's|^/// Adds.|/// Adds two numbers.|' "$LIB"
git commit -qam "comment only"
check_last "comment-only change" ok

sed -i 's|a + b|b + a|; s|{ 1 }|{ 2 }|' "$LIB"
git commit -qam "body only"
check_last "body-only change" ok

sed -i 's|b: i32) -> i32|b: i64) -> i32|' "$LIB"
git commit -qam "sig no header"
check_last "multi-line signature change, no header" fail

sed -i 's|b: i64) -> i32|b: u8) -> i32|' "$LIB"
echo "// sig" >>"$HDR"
git commit -qam "sig with header"
check_last "signature change with header" ok

sed -i 's|    pub y: i32,|    pub y: i32,\n    pub z: i32,|' "$LIB"
git commit -qam "field no header"
check_last "repr(C) field added, no header" fail

printf '#[no_mangle]\npub extern "C" fn ce_new() {}\n' >>"$LIB"
git commit -qam "new fn no header"
check_last "new extern fn, no header" fail

echo "// more" >>"$HDR"
git commit -qam "header only"
check_last "header-only change" ok

if out=$(bash "$script" base 2>&1); then
    echo "FAIL: range should fail"
    fails=$((fails + 1))
else
    n=$(printf '%s\n' "$out" | grep -c '^fail ')
    if [ "$n" = 3 ]; then
        echo "pass: range reports 3 failures"
    else
        echo "FAIL: range: expected 3 failures, got $n"
        fails=$((fails + 1))
    fi
fi

if bash "$script" HEAD >/dev/null 2>&1; then
    echo "pass: empty range"
else
    echo "FAIL: empty range"
    fails=$((fails + 1))
fi

if bash "$script" no-such-ref >/dev/null 2>&1; then
    echo "FAIL: unresolvable base accepted"
    fails=$((fails + 1))
else
    echo "pass: unresolvable base rejected"
fi

if [ "$fails" = 0 ]; then
    echo "all passed"
else
    echo "$fails failed"
    exit 1
fi
