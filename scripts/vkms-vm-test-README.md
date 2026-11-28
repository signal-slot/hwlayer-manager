# vkms-vm-test

Runs the DRM backend against vkms inside a throwaway QEMU VM. POSIX `sh`.
The VM boots the node's own kernel image and loads the node's own vkms
modules from a tiny initramfs, so nothing is loaded or changed on the node.

```
sh scripts/vkms-vm-test.sh
```

It takes no arguments. Two environment variables select the kernel:

| Variable | Default | Meaning |
|---|---|---|
| `VKMS_KERNEL` | `/usr/src/linux/arch/x86/boot/bzImage` | kernel image to boot |
| `VKMS_MODULES` | `/lib/modules/$(uname -r)` | module directory |

The two must belong to the same kernel build.

## What it does

1. Builds, for `x86_64-unknown-linux-musl` (static), the `vm-init` binary
   (release), the `vkms` test binary of `composition-drm` (`--no-run`) and
   the `twoplane` example.
2. Builds an initramfs under `target/vkms-vm/`: the `vm-init` binary as
   `/init`, the two test programs, a `commands.list`, and the vkms module
   with its dependencies from `modules.dep` (dependencies first). Only
   uncompressed `.ko` modules are supported. The vkms module gets
   `enable_overlay=1 enable_cursor=1`, set inside the VM only.
3. Starts `qemu-system-x86_64` (512 MB, no graphics, 300 s timeout;
   `-enable-kvm -cpu host` only if `/dev/kvm` is writable). The VM runs, in
   order:
   - `/opt/vkms --ignored --test-threads=1`: the ignored tests of
     `crates/composition-drm/tests/vkms.rs`;
   - `/opt/twoplane /dev/dri/card0 --frames 60 --unsupported`: the
     `twoplane` example for 60 frames with its unsupported-combination list.
4. Prints the test section of the console log (`--- command:` to
   `VM_TEST_RESULT=`) and a closing line saying none of it is a hardware
   result. The full console is kept in `target/vkms-vm/console.log`.

## What it needs on the node

- `cargo` with the `x86_64-unknown-linux-musl` target;
- `qemu-system-x86_64`, `cpio`, `gzip`, `timeout`;
- a readable kernel image and a module directory with `modules.dep`
  containing an uncompressed `vkms.ko`;
- optionally write access to `/dev/kvm` (faster; no root is otherwise needed).

## Exit codes

- `0`: the console log contains `VERDICT twoplane: PASS` and
  `VM_TEST_RESULT=0`.
- `2`: a precondition failed (kernel image or `modules.dep` unreadable, vkms
  module not found, compressed module, a binary did not build).
- Any other non-zero (from `set -e` or `grep -q`): a build step failed, or the
  console log lacks one of the two lines (VM timeout, a failed command, no
  PASS verdict).

## Limits

- It is not a review command. The review gate is the CI of `docs/ci.md`
  (`scripts/check-all.sh` runs those checks locally); this script is not part
  of CI.
- It does not run on every node: it needs an x86_64 node with the kernel
  image, matching vkms modules and QEMU. Without them it exits 2 or fails.
- A vkms result is never a hardware result. vkms is a virtual device and says
  nothing about any SoC's planes, scanout or timing.
- Never load vkms, or touch DRM, udev or seat configuration, on the node
  itself. The script exists so that the VM does it instead.
