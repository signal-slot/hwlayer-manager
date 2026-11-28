#!/bin/bash
# Tests for check-am62p-r5f-elf.sh (a host link check, not a firmware build).
# The PASS/SKIP/FAIL logic is tested with stub clang, rustc, cargo, nm and
# linker, so no Arm toolchain is needed. A last case runs the real script on
# this machine and accepts PASS or SKIP.
set -u
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/r5felf.XXXXXX") || exit 1
trap 'rm -rf "$tmp"' EXIT
fails=0

check() { # name rc (0 = pass)
    if [ "$2" -eq 0 ]; then echo "pass $1"; else echo "FAIL $1"; fails=$((fails + 1)); fi
}

# The script finds its root from its own location: run a copy in a fake repo.
root=$tmp/repo
mkdir -p "$root/scripts" "$tmp/target-libdir"
cp "$here/check-am62p-r5f-elf.sh" "$root/scripts/"
script=$root/scripts/check-am62p-r5f-elf.sh

# A PATH of only the basic tools the script uses, so no real clang is seen.
sys=$tmp/sys
mkdir -p "$sys"
for t in sh bash basename dirname mkdir mktemp rm grep awk sed sort head tail cat tr ls touch; do
    p=$(command -v "$t") && ln -s "$p" "$sys/$t"
done

# Stubs. Behaviour is chosen by environment variables:
#   NO_TARGET     rustc reports a target-libdir that does not exist
#   NM_UNDEF      nm lists an undefined symbol in the image
#   LD_UNDEF      the linker prints an undefined symbol and fails
#   LD_FAIL       the linker fails without naming a symbol
#   CC_FAIL       clang fails on scenes.c
#   NO_ARM        clang cannot compile for the target
mkbin() {
    local b=$1
    mkdir -p "$b"
    cat > "$b/clang" <<'EOS'
#!/bin/sh
case "$*" in *-print-resource-dir*) echo /fake/res; exit 0 ;; esac
[ -n "${NO_ARM:-}" ] && { echo "error: unknown target triple" >&2; exit 1; }
[ -n "${CC_FAIL:-}" ] && case "$*" in *scenes.c*) echo "scenes.c: error" >&2; exit 1 ;; esac
out=
while [ $# -gt 0 ]; do
    [ "$1" = -o ] && { out=$2; shift; }
    shift
done
[ -n "$out" ] && : > "$out"
exit 0
EOS
    cat > "$b/rustc" <<'EOS'
#!/bin/sh
case "$*" in
*target-libdir*) [ -n "${NO_TARGET:-}" ] && echo /nonexistent/libdir || echo "$FAKE_LIBDIR" ;;
*sysroot*) echo /nonexistent/sysroot ;;
esac
EOS
    cat > "$b/cargo" <<'EOS'
#!/bin/sh
mkdir -p target/armv7r-none-eabihf/release
: > target/armv7r-none-eabihf/release/libcomposition_engine.a
EOS
    cat > "$b/nm" <<'EOS'
#!/bin/sh
echo "70000000 T scenes_run"
echo "70000100 T ce_twoplane_init"
echo "70000200 T ce_engine_init"
[ -n "${NM_UNDEF:-}" ] && echo "         U __stub_missing"
exit 0
EOS
    cat > "$b/ld.lld" <<'EOS'
#!/bin/sh
[ -n "${LD_UNDEF:-}" ] && { echo "ld.lld: error: undefined symbol: ce_missing_fn" >&2; exit 1; }
[ -n "${LD_FAIL:-}" ] && { echo "ld.lld: error: cannot open linker script" >&2; exit 1; }
out=
while [ $# -gt 0 ]; do
    [ "$1" = -o ] && { out=$2; shift; }
    shift
done
: > "$out"
exit 0
EOS
    chmod +x "$b"/*
}
stubs=$tmp/stubs
mkbin "$stubs"

run() { # extra env assignments are passed through "env"; PATH is the stubs
    (cd "$root" && env FAKE_LIBDIR="$tmp/target-libdir" PATH="$stubs:$sys" "$@" sh "$script") 2>&1
}
last() { printf '%s\n' "$1" | tail -n 1; }

# 1. PASS: everything resolves.
out=$(run); rc=$?
[ "$(last "$out")" = PASS ] && [ "$rc" -eq 0 ]; check "stub toolchain: PASS, exit 0" $?

# 2. SKIP without clang (PATH has no clang at all), exit 0.
out=$(cd "$root" && env PATH="$sys" sh "$script" 2>&1); rc=$?
case $(last "$out") in "SKIP: "*clang*) [ "$rc" -eq 0 ]; check "no clang: SKIP, exit 0" $? ;; *) echo "$out"; check "no clang: SKIP, exit 0" 1 ;; esac

# 3. SKIP without the Rust target.
out=$(run NO_TARGET=1); rc=$?
case $(last "$out") in "SKIP: "*armv7r*) [ "$rc" -eq 0 ]; check "no rust target: SKIP, exit 0" $? ;; *) echo "$out"; check "no rust target: SKIP, exit 0" 1 ;; esac

# 4. SKIP when clang has no Arm backend.
out=$(run NO_ARM=1); rc=$?
case $(last "$out") in "SKIP: "*) [ "$rc" -eq 0 ]; check "clang without Arm: SKIP, exit 0" $? ;; *) echo "$out"; check "clang without Arm: SKIP, exit 0" 1 ;; esac

# 5. SKIP without any linker (no ld.lld, no rust-lld, no arm-none-eabi-gcc).
nold=$tmp/nold
mkbin "$nold"; rm "$nold/ld.lld"
out=$(cd "$root" && env FAKE_LIBDIR="$tmp/target-libdir" PATH="$nold:$sys" sh "$script" 2>&1); rc=$?
case $(last "$out") in "SKIP: "*linker*|"SKIP: no ld.lld"*) [ "$rc" -eq 0 ]; check "no linker: SKIP, exit 0" $? ;; *) echo "$out"; check "no linker: SKIP, exit 0" 1 ;; esac

# 6. FAIL: the linker reports an undefined symbol, which is named.
out=$(run LD_UNDEF=1); rc=$?
case $(last "$out") in "FAIL: "*ce_missing_fn*) [ "$rc" -eq 1 ]; check "undefined symbol at link: FAIL, exit 1" $? ;; *) echo "$out"; check "undefined symbol at link: FAIL, exit 1" 1 ;; esac

# 7. FAIL: the linker fails for another reason.
out=$(run LD_FAIL=1); rc=$?
case $(last "$out") in "FAIL: "*) [ "$rc" -eq 1 ]; check "link error: FAIL, exit 1" $? ;; *) echo "$out"; check "link error: FAIL, exit 1" 1 ;; esac

# 8. FAIL: the image still has an undefined symbol (found with nm).
out=$(run NM_UNDEF=1); rc=$?
case $(last "$out") in "FAIL: "*__stub_missing*) [ "$rc" -eq 1 ]; check "undefined in image: FAIL, exit 1" $? ;; *) echo "$out"; check "undefined in image: FAIL, exit 1" 1 ;; esac

# 9. FAIL: a C file does not compile.
out=$(run CC_FAIL=1); rc=$?
case $(last "$out") in "FAIL: "*scenes.c*) [ "$rc" -eq 1 ]; check "compile error: FAIL, exit 1" $? ;; *) echo "$out"; check "compile error: FAIL, exit 1" 1 ;; esac

# 10. The real script on this machine: PASS or SKIP: <reason>, exit 0.
out=$(sh "$here/check-am62p-r5f-elf.sh" 2>&1); rc=$?
case $(last "$out") in
PASS | "SKIP: "?*) [ "$rc" -eq 0 ]; check "real run: $(last "$out")" $? ;;
*) printf '%s\n' "$out"; check "real run: last line is PASS or SKIP" 1 ;;
esac

# 11. The header says what this is and that M3-05 stays open.
head -n 15 "$script" | grep -q 'HOST LINK CHECK, not a firmware build' &&
    head -n 15 "$script" | grep -q 'M3-05'
check "header: host link check, M3-05 open" $?

[ "$fails" -eq 0 ] || { echo "FAIL: $fails check(s)"; exit 1; }
echo "ok: all checks passed"
