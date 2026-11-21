#!/bin/sh
# Runs the vkms integration tests inside a throwaway QEMU VM.
#
# Nothing on the host is touched: no module is loaded, no root is needed
# (only access to /dev/kvm, and even that is optional). The VM boots the
# host's kernel image with an initramfs holding a tiny init, the vkms
# modules and the statically linked test binary.
#
# Environment:
#   VKMS_KERNEL   kernel image       (default: /usr/src/linux/arch/x86/boot/bzImage)
#   VKMS_MODULES  module directory   (default: /lib/modules/$(uname -r))
# The two must belong to the same kernel build.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
kernel=${VKMS_KERNEL:-/usr/src/linux/arch/x86/boot/bzImage}
modules=${VKMS_MODULES:-/lib/modules/$(uname -r)}
target=x86_64-unknown-linux-musl
work=$root/target/vkms-vm

[ -r "$kernel" ] || { echo "kernel image not readable: $kernel" >&2; exit 2; }
[ -r "$modules/modules.dep" ] || { echo "no modules.dep in $modules" >&2; exit 2; }

cd "$root"
cargo build --quiet -p vm-init --target "$target" --release
test_exe=$(cargo test -p composition-drm --test vkms --target "$target" --no-run 2>&1 |
    sed -n 's/.*Executable.*(\(.*\))$/\1/p')
[ -x "$test_exe" ] || { echo "could not build the vkms test binary" >&2; exit 2; }

rm -rf "$work"
mkdir -p "$work/root/tests" "$work/root/modules"
cp "target/$target/release/vm-init" "$work/root/init"
cp "$test_exe" "$work/root/tests/vkms"

# vkms and its dependencies, dependencies first.
vkms=$(sed -n 's|^\(kernel/.*/vkms\.ko[^:]*\):.*|\1|p' "$modules/modules.dep")
[ -n "$vkms" ] || { echo "vkms module not found in $modules" >&2; exit 2; }
deps=$(sed -n "s|^$vkms: *||p" "$modules/modules.dep")
: > "$work/root/modules.list"
n=0
for module in $(echo "$deps" | tr ' ' '\n' | sed '1!G;h;$!d') "$vkms"; do
    case $module in
        *.ko) ;;
        *) echo "compressed module not supported: $module" >&2; exit 2 ;;
    esac
    n=$((n + 1))
    name=$(printf '%02d-%s' "$n" "$(basename "$module")")
    cp "$modules/$module" "$work/root/modules/$name"
    params=
    [ "$module" = "$vkms" ] && params=" enable_overlay=1 enable_cursor=1"
    echo "/modules/$name$params" >> "$work/root/modules.list"
done

(cd "$work/root" && find . | cpio --quiet -o -H newc | gzip -1) > "$work/initramfs.gz"

accel=
[ -w /dev/kvm ] && accel="-enable-kvm -cpu host"
# shellcheck disable=SC2086
timeout 300 qemu-system-x86_64 $accel -m 512 -nographic -vga none -no-reboot \
    -kernel "$kernel" -initrd "$work/initramfs.gz" \
    -append "console=ttyS0 quiet panic=-1 rdinit=/init" \
    < /dev/null | tr -d '\r' > "$work/console.log" || true

# Show the test output only; the full console is kept in console.log.
sed -n '/--- \/tests\//,/^VM_TEST_RESULT=/p' "$work/console.log"
grep -q '^VM_TEST_RESULT=0' "$work/console.log"
