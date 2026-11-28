#!/bin/bash
# Tests for check-am62p-r5f-c.sh with stub clang, nm, rustup and cargo; no
# Arm toolchain is needed and nothing is really built.
set -u
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/r5fc.XXXXXX") || exit 1
trap 'rm -rf "$tmp"' EXIT
fails=0

check() { # name rc (0 = pass)
    if [ "$2" -eq 0 ]; then echo "pass $1"; else echo "FAIL $1"; fails=$((fails + 1)); fi
}

# The script finds its root from its own location: run a copy in a fake repo.
root=$tmp/repo
mkdir -p "$root/scripts"
cp "$here/check-am62p-r5f-c.sh" "$root/scripts/"
script=$root/scripts/check-am62p-r5f-c.sh
chmod +x "$script"

# A PATH of only the basic tools the script uses, so a real clang is never seen.
sys=$tmp/sys
mkdir -p "$sys"
for t in bash sh basename dirname mkdir grep awk sed sort head cat touch; do
    p=$(command -v "$t") && ln -s "$p" "$sys/$t"
done

# clang stub: -print-resource-dir, a probe/compile that writes the -o file.
# FAKE_CLANG_NOARM makes every compile fail, as a clang without the ARM target.
mkbin() {
    local b=$1
    mkdir -p "$b"
    cat > "$b/clang" <<'EOS'
#!/bin/sh
case "$*" in *-print-resource-dir*) echo /fake/res; exit 0 ;; esac
echo "clang $*" >> "$FAKE_LOG"
[ -n "${FAKE_CLANG_NOARM:-}" ] && { echo "error: unknown target triple" >&2; exit 1; }
[ -n "${FAKE_CC_FAIL:-}" ] && case "$*" in *scenes.c*) exit 1 ;; esac
out=
while [ $# -gt 0 ]; do
    [ "$1" = -o ] && out=$2
    shift
done
[ -n "$out" ] && : > "$out"
exit 0
EOS
    # nm stub: prints $FAKE_NM_DIR/<basename of the file>
    cat > "$b/llvm-nm" <<'EOS'
#!/bin/sh
f=$FAKE_NM_DIR/$(basename "$1")
[ -f "$f" ] && cat "$f"
exit 0
EOS
    cat > "$b/cargo" <<'EOS'
#!/bin/sh
echo "cargo $*" >> "$FAKE_LOG"
exit 0
EOS
    # rustup: "target list --installed" prints $FAKE_RUST_TARGETS
    cat > "$b/rustup" <<'EOS'
#!/bin/sh
printf '%s\n' $FAKE_RUST_TARGETS
exit 0
EOS
    chmod +x "$b"/*
}

bin=$tmp/bin
mkbin "$bin"

nmd=$tmp/nm
export FAKE_NM_DIR=$nmd FAKE_LOG=$tmp/log
export FAKE_RUST_TARGETS="x86_64-unknown-linux-gnu armv7r-none-eabihf"

setnm() { # scenes-nm backend-nm lib-nm
    rm -rf "$nmd"; mkdir -p "$nmd"
    printf '%s' "$1" > "$nmd/scenes.o"
    printf '%s' "$2" > "$nmd/ce_twoplane.o"
    printf '%s' "$3" > "$nmd/libcomposition_engine.a"
}

run() { # path-dir args... ; sets rc, out, err
    local b=$1
    shift
    : > "$FAKE_LOG"
    out=$(PATH="$b:$sys" "$script" "$@" 2>"$tmp/err")
    rc=$?
    err=$(cat "$tmp/err")
}

scenes='00000000 T board_scene
         U ce_commit
         U ce_twoplane_commit
         U memcpy
'
backend='00000000 T ce_twoplane_commit
         U __aeabi_memclr4
         U ce_plane_set
'
lib='00000010 T ce_commit
00000020 T ce_plane_set
00000030 W __aeabi_memclr4
00000040 W memcpy
'

# everything in order: pass, with the right flags
setnm "$scenes" "$backend" "$lib"
run "$bin" "$tmp/o1"
check pass-rc "$rc"
echo "$out" | grep -q '^toolchain: clang (llvm-nm)$'; check toolchain-line $?
echo "$out" | grep -q 'ok: scenes.c and ce_twoplane.c compile'; check compile-ok $?
echo "$out" | grep -q 'ok: undefined symbols are only'; check undef-ok $?
echo "$out" | grep -q 'ok: libcomposition_engine.a defines all 4 symbols'; check lib-ok $?
grep -q -- '--target=armv7r-none-eabihf -mcpu=cortex-r5 -mfpu=vfpv3-d16 -mfloat-abi=hard -std=c11' "$FAKE_LOG"; check flags-target $?
grep -q -- '-Wall -Wextra -Werror -pedantic -ffreestanding -nostdinc -isystem /fake/res/include -O2' "$FAKE_LOG"; check flags-freestanding $?
grep -q -- 'scenes.c' "$FAKE_LOG"; check compiles-scenes $?
grep -q -- 'ce_twoplane.c' "$FAKE_LOG"; check compiles-backend $?
grep -q -- 'cargo build --locked -p composition-ffi-staticlib .*--target armv7r-none-eabihf' "$FAKE_LOG"; check builds-lib $?
[ -d "$tmp/o1" ]; check out-dir-arg $?

# a stray undefined symbol: fail
setnm "$scenes" "$backend
         U printf
" "$lib"
run "$bin" "$tmp/o2"
[ "$rc" -eq 1 ]; check stray-rc $?
echo "$err" | grep -q 'fail: unexpected undefined symbols'; check stray-msg $?
echo "$err" | grep -q 'printf'; check stray-lists-symbol $?

# an engine symbol the library lacks: fail
setnm "$scenes" "$backend" '00000010 T ce_commit
00000030 W __aeabi_memclr4
00000040 W memcpy
'
run "$bin" "$tmp/o3"
[ "$rc" -eq 1 ]; check lib-missing-rc $?
echo "$err" | grep -q 'libcomposition_engine.a does not define'; check lib-missing-msg $?
echo "$err" | grep -q 'ce_plane_set'; check lib-missing-lists-symbol $?

# a runtime symbol the library lacks: fail
setnm "$scenes" "$backend" '00000010 T ce_commit
00000020 T ce_plane_set
00000040 W memcpy
'
run "$bin" "$tmp/o4"
[ "$rc" -eq 1 ]; check memclr-missing-rc $?
echo "$err" | grep -q '__aeabi_memclr4'; check memclr-missing-msg $?

# a symbol the objects define themselves is not asked of the library
setnm "$scenes" "$backend" '00000010 T ce_commit
00000020 T ce_plane_set
00000030 W __aeabi_memclr4
00000040 W memcpy
'
run "$bin" "$tmp/o5"
echo "$out" | grep -q 'ok: libcomposition_engine.a defines all 4'; check own-defs-not-needed $?

# compile failure aborts with non-zero status
setnm "$scenes" "$backend" "$lib"
FAKE_CC_FAIL=1 run "$bin" "$tmp/o6"
[ "$rc" -ne 0 ]; check compile-fail-rc $?
echo "$out" | grep -q 'ok: undefined'; [ $? -ne 0 ]; check compile-fail-stops $?

# rust target missing: the library check is skipped, the rest passes
FAKE_RUST_TARGETS="x86_64-unknown-linux-gnu" run "$bin" "$tmp/o7"
check no-rust-target-rc "$rc"
echo "$out" | grep -q 'skipped: library check'; check no-rust-target-msg $?
grep -q '^cargo' "$FAKE_LOG"; [ $? -ne 0 ]; check no-rust-target-no-cargo $?

# no clang on PATH: skipped, exit 0
mkdir -p "$tmp/noclang"
cp "$bin"/* "$tmp/noclang/"
rm "$tmp/noclang/clang"
run "$tmp/noclang" "$tmp/o8"
check no-clang-rc "$rc"
echo "$out" | grep -q '^skipped: clang is not on PATH'; check no-clang-msg $?

# clang without the ARM target: skipped, exit 0
FAKE_CLANG_NOARM=1 run "$bin" "$tmp/o9"
check no-arm-rc "$rc"
echo "$out" | grep -q '^skipped: clang cannot compile for armv7r-none-eabihf'; check no-arm-msg $?

# no nm at all: skipped, exit 0
mkdir -p "$tmp/nonm"
cp "$bin"/* "$tmp/nonm/"
rm "$tmp/nonm/llvm-nm"
run "$tmp/nonm" "$tmp/o10"
check no-nm-rc "$rc"
echo "$out" | grep -q '^skipped: no llvm-nm or nm'; check no-nm-msg $?

echo
if [ "$fails" -eq 0 ]; then echo "all passed"; else echo "$fails failed"; exit 1; fi
