#!/bin/sh
# Runs tools/drm-harness on vkms inside a throwaway QEMU VM.
#
# Same VM as scripts/vkms-vm-test.sh: the host's kernel image, an initramfs
# with tools/vm-init as PID 1, the vkms modules and, here, the statically
# linked drm-harness at /bin/drm-harness. vm-init runs the command lines of
# /commands.list (see tools/vm-init) instead of the vkms tests. Nothing on
# the host is touched: no module is loaded, no root is needed.
#
# The results are vkms only. vkms has no zpos or alpha properties and one
# primary, eight overlay and one cursor plane, so some refusals are expected
# and are the profile's, not the harness's.
#
# Usage: scripts/vkms-harness-test.sh [COMMANDS-FILE]
#   COMMANDS-FILE  lines for vm-init's /commands.list [default: the list below]
#
# Environment:
#   VKMS_KERNEL   kernel image       (default: /usr/src/linux/arch/x86/boot/bzImage)
#   VKMS_MODULES  module directory   (default: /lib/modules/$(uname -r))
#   VKMS_TIMEOUT  seconds for the whole VM run (default: 900)
# The two must belong to the same kernel build.
#
# The full console is kept in target/vkms-harness/console.log; the output
# of the commands is printed. The exit status is 0 only if every command
# exited with 0.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
kernel=${VKMS_KERNEL:-/usr/src/linux/arch/x86/boot/bzImage}
modules=${VKMS_MODULES:-/lib/modules/$(uname -r)}
vm_timeout=${VKMS_TIMEOUT:-900}
target=x86_64-unknown-linux-musl
work=$root/target/vkms-harness
commands=${1:-}

[ -r "$kernel" ] || { echo "kernel image not readable: $kernel" >&2; exit 2; }
[ -r "$modules/modules.dep" ] || { echo "no modules.dep in $modules" >&2; exit 2; }
[ -z "$commands" ] || [ -r "$commands" ] || { echo "not readable: $commands" >&2; exit 2; }

cd "$root"
cargo build --quiet -p vm-init --target "$target" --release
cargo build --quiet -p composition-drm-harness --bin drm-harness --target "$target" --release

rm -rf "$work"
mkdir -p "$work/root/bin" "$work/root/modules"
cp "target/$target/release/vm-init" "$work/root/init"
cp "target/$target/release/drm-harness" "$work/root/bin/drm-harness"

if [ -n "$commands" ]; then
    cp "$commands" "$work/root/commands.list"
else
    h="/bin/drm-harness --seconds 1"
    cat > "$work/root/commands.list" <<EOF
# DRM master descriptor, buffers allocated by the harness.
$h two-planes
$h zorder-alpha
$h move
$h combos
$h rotate
$h all
# Kernel DRM lease made in-process, with the revocation scenario.
$h --source kernel-lease revoke
$h --source kernel-lease all
# Buffers from separate producer processes, as DMA-BUFs.
$h --producers external all
$h --source kernel-lease --producers external revoke
# A lease received over drm-lease-manager's protocol from drm-harness lessor.
& /bin/drm-harness lessor --lease vkms --dlm-dir /tmp
wait /tmp/vkms
$h --source dlm --lease vkms --dlm-dir /tmp all
EOF
fi

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
timeout "$vm_timeout" qemu-system-x86_64 $accel -smp 2 -m 1024 -nographic -vga none -no-reboot \
    -kernel "$kernel" -initrd "$work/initramfs.gz" \
    -append "console=ttyS0 quiet panic=-1 rdinit=/init" \
    < /dev/null | tr -d '\r' > "$work/console.log" || true

# Show the commands' output only; the full console is kept in console.log.
sed -n '/^--- command: /,/^VM_TEST_RESULT=/p' "$work/console.log"
grep -q '^VM_TEST_RESULT=0' "$work/console.log"
