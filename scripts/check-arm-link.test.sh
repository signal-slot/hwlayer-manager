#!/bin/bash
# Tests for check-arm-link.sh with stub compilers, linkers and nm; no ARM
# toolchain, cargo or rustup is needed and nothing is really built.
set -u
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/armlink.XXXXXX") || exit 1
trap 'rm -rf "$tmp"' EXIT
fails=0

check() { # name rc (0 = pass)
    if [ "$2" -eq 0 ]; then echo "pass $1"; else echo "FAIL $1"; fails=$((fails + 1)); fi
}

# The script finds its root from its own location: run a copy in a fake repo.
root=$tmp/repo
mkdir -p "$root/scripts"
cp "$here/check-arm-link.sh" "$root/scripts/"
script=$root/scripts/check-arm-link.sh

# A PATH holding only the basic tools the script uses, so a real ARM
# toolchain on the host is never seen; stubs are added per case.
sys=$tmp/sys
mkdir -p "$sys"
for t in bash sh basename dirname mkdir grep awk ls head cat touch; do
    p=$(command -v "$t") && ln -s "$p" "$sys/$t"
done

mkstubs() { # dir: generic stubs shared by both toolchains
    local b=$1 t
    mkdir -p "$b"
    for t in cargo rustup; do printf '#!/bin/sh\nexit 0\n' > "$b/$t"; done
    cat > "$b/rustc" <<'EOS'
#!/bin/sh
echo "$FAKE_SYSROOT"
EOS
    # compiler / linker stub: create the -o file (and the map file)
    cat > "$b/fakecc" <<'EOS'
#!/bin/sh
case "$*" in *-print-resource-dir*) echo /fake/res; exit 0 ;; esac
echo "$(basename "$0") $*" >> "$FAKE_LOG"
[ -n "${FAKE_CC_FAIL:-}" ] && exit 1
out=
for a in "$@"; do
    case "$a" in
    -Wl,-Map=*) : > "${a#-Wl,-Map=}" ;;
    -Map=*) : > "${a#-Map=}" ;;
    esac
done
while [ $# -gt 0 ]; do
    [ "$1" = -o ] && out=$2
    shift
done
[ -n "$out" ] && : > "$out"
exit 0
EOS
    # nm stub: prints $FAKE_NM_DIR/<basename of the file>
    cat > "$b/fakenm" <<'EOS'
#!/bin/sh
f=$FAKE_NM_DIR/$(basename "$1")
[ -f "$f" ] && cat "$f"
exit 0
EOS
    chmod +x "$b"/*
}

# Stubs are real copies, not links: they name themselves by $0 in the log.
gccbin=$tmp/gccbin
mkstubs "$gccbin"
cp "$gccbin/fakecc" "$gccbin/arm-none-eabi-gcc"
cp "$gccbin/fakenm" "$gccbin/arm-none-eabi-nm"

clbin=$tmp/clbin
mkstubs "$clbin"
cp "$clbin/fakecc" "$clbin/clang"
cp "$clbin/fakecc" "$clbin/ld.lld"
cp "$clbin/fakenm" "$clbin/llvm-nm"

nmd=$tmp/nm
export FAKE_NM_DIR=$nmd FAKE_LOG=$tmp/log FAKE_SYSROOT=$tmp/nosysroot

setnm() { # obj-nm-text image-nm-text
    rm -rf "$nmd"; mkdir -p "$nmd"
    printf '%s' "$1" > "$nmd/ce_twoplane.o"
    printf '%s' "$2" > "$nmd/image.elf"
}

run() { # path-dir args... ; sets rc, out, err
    local b=$1
    shift
    : > "$FAKE_LOG"
    out=$(PATH="$b:$sys" "$script" "$@" 2>"$tmp/err")
    rc=$?
    err=$(cat "$tmp/err")
}

memclr_obj='         U __aeabi_memclr4
00000000 T ce_plan
         U ce_other
'
good_img='08000000 T __aeabi_memclr4
08000010 T ce_plan
'

# gcc, object calls memclr4, image defines it: pass
setnm "$memclr_obj" "$good_img"
run "$gccbin" "$tmp/o1"
check gcc-pass-rc "$rc"
echo "$out" | grep -q '^toolchain: gcc (arm-none-eabi-nm)$'; check gcc-toolchain-line $?
echo "$out" | grep -q '^== cortex-m7 (thumbv7em-none-eabihf)$'; check m7-section $?
echo "$out" | grep -q '^== cortex-r5 (armv7r-none-eabihf)$'; check r5-section $?
echo "$out" | grep -q 'ok: __aeabi_memclr4 defined in the image'; check memclr-ok $?
echo "$out" | grep -q 'ok: no undefined symbols in cortex-r5 image'; check noundef-ok $?
[ -d "$tmp/o1/cortex-m7" ] && [ -d "$tmp/o1/cortex-r5" ]; check out-dir-arg $?
grep -q -- '-mcpu=cortex-m7 .*-Wall -Wextra -Werror' "$FAKE_LOG"; check m7-flags $?
grep -q -- '-mcpu=cortex-r5' "$FAKE_LOG"; check r5-flags $?
grep -q -- '-lgcc' "$FAKE_LOG"; check gcc-links-libgcc $?

# memclr4 required but missing from the image: fail, rc 1, both targets
setnm "$memclr_obj" '08000010 T ce_plan
'
run "$gccbin" "$tmp/o2"
[ "$rc" -eq 1 ]; check memclr-missing-rc $?
echo "$err" | grep -q 'fail: __aeabi_memclr4 is not defined in cortex-m7 image'; check memclr-missing-m7 $?
echo "$err" | grep -q 'fail: __aeabi_memclr4 is not defined in cortex-r5 image'; check memclr-missing-r5 $?

# object does not call memclr4: it calls memset, the image defines it: pass
setnm '         U memset
         U ce_x
00000000 T ce_plan
' '08000020 T memset
'
run "$gccbin" "$tmp/o3"
check memset-pass-rc "$rc"
echo "$out" | grep -q 'memset defined in the image'; check memset-defined $?
echo "$out" | grep -q 'object does not call __aeabi_memclr4'; check memclr-not-needed $?

# ...and memset is missing from the image: fail
setnm '         U memset
' ''
run "$gccbin" "$tmp/o4"
[ "$rc" -eq 1 ]; check memset-missing-rc $?
echo "$err" | grep -q 'fail: memset is not defined in cortex-m7 image'; check memset-missing-msg $?

# undefined symbol left in the image: fail even if memclr4 is defined
setnm "$memclr_obj" '08000000 T __aeabi_memclr4
         U stray_symbol
'
run "$gccbin" "$tmp/o5"
[ "$rc" -eq 1 ]; check undef-rc $?
echo "$err" | grep -q 'fail: undefined symbols in cortex-m7 image'; check undef-msg $?
echo "$err" | grep -q 'stray_symbol'; check undef-lists-symbol $?

# a weak definition counts as defined
setnm "$memclr_obj" '08000000 W __aeabi_memclr4
'
run "$gccbin" "$tmp/o6"
check weak-def-rc "$rc"

# a compile failure aborts (set -e) with a non-zero status
setnm "$memclr_obj" "$good_img"
FAKE_CC_FAIL=1 run "$gccbin" "$tmp/o7"
[ "$rc" -ne 0 ]; check compile-fail-rc $?
echo "$out" | grep -q 'ok: no undefined'; [ $? -ne 0 ]; check compile-fail-stops $?

# CC_KIND validation
CC_KIND=bogus run "$gccbin" "$tmp/o8"
[ "$rc" -eq 2 ]; check bad-kind-rc $?
echo "$err" | grep -q 'fail: CC_KIND must be gcc or clang'; check bad-kind-msg $?

# no gcc on PATH: falls back to clang + ld.lld
setnm "$memclr_obj" "$good_img"
run "$clbin" "$tmp/o9"
check clang-pass-rc "$rc"
echo "$out" | grep -q '^toolchain: clang (llvm-nm)$'; check clang-toolchain-line $?
grep -q -- '--target=thumbv7em-none-eabihf' "$FAKE_LOG"; check clang-target-m7 $?
grep -q -- '--target=armv7r-none-eabihf' "$FAKE_LOG"; check clang-target-r5 $?
grep -q -- '-nostdinc -isystem /fake/res/include' "$FAKE_LOG"; check clang-freestanding $?
grep -q -- '^ld.lld -nostdlib --gc-sections' "$FAKE_LOG"; check clang-lld-link $?

# CC_KIND=clang is honoured even with gcc on PATH
mkdir -p "$tmp/both"
cp "$clbin"/* "$tmp/both/"
cp "$gccbin/fakecc" "$tmp/both/arm-none-eabi-gcc"
CC_KIND=clang run "$tmp/both" "$tmp/o10"
echo "$out" | grep -q '^toolchain: clang'; check cc-kind-forces-clang $?

# clang with no linker at all: rc 2
mkdir -p "$tmp/nolld" "$FAKE_SYSROOT"
cp "$clbin"/* "$tmp/nolld/"
rm "$tmp/nolld/ld.lld"
setnm "$memclr_obj" "$good_img"
run "$tmp/nolld" "$tmp/o11"
[ "$rc" -eq 2 ]; check no-linker-rc $?
echo "$err" | grep -q 'fail: no ld.lld or rust-lld'; check no-linker-msg $?

# the Rust sysroot's rust-lld is used when ld.lld is absent
mkdir -p "$FAKE_SYSROOT/lib/rustlib/x86_64-fake/bin"
cp "$clbin/fakecc" "$FAKE_SYSROOT/lib/rustlib/x86_64-fake/bin/rust-lld"
run "$tmp/nolld" "$tmp/o12"
check rust-lld-rc "$rc"
grep -q -- '^rust-lld -flavor gnu -nostdlib' "$FAKE_LOG"; check rust-lld-used $?

echo
if [ "$fails" -eq 0 ]; then echo "all passed"; else echo "$fails failed"; exit 1; fi
