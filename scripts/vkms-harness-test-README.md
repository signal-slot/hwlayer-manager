# vkms-harness-test.sh

Runs `tools/drm-harness` (the `drm-harness` binary) against the kernel's `vkms` virtual DRM device inside a throwaway QEMU VM, and prints the harness output.

This is a virtual device. **A vkms result is never a hardware result**: it says nothing about any SoC's planes, scanout, latch or timing, and it is not evidence for any milestone on a board. vkms has no zpos or alpha properties and offers one primary, eight overlay and one cursor plane, so some refusals are expected; they belong to the vkms profile, not to the harness.

## The VM-only rule

vkms runs only inside a QEMU VM, on nodes that can boot their own kernel image. Never load vkms (or any module) on a node, and never touch a node's DRM devices, udev rules or seat configuration to run it. The script follows this: `modprobe`/`insmod` happen only in the VM's PID 1 (`tools/vm-init`), no root is needed, and the host is not modified. A node that cannot boot its own kernel image (no readable kernel image and matching modules, or no `qemu-system-x86_64`) does not run vkms at all; it reports that, and does not fall back to loading the module locally.

## Prerequisites

- `qemu-system-x86_64`, `cpio`, `gzip`, `timeout`, and `cargo` with the `x86_64-unknown-linux-musl` target (the script builds `vm-init` and `drm-harness` statically for it, in release mode).
- A kernel image and the module directory of the **same kernel build**, with `vkms` built as an uncompressed `.ko` (compressed modules are refused). Both are only read.
- `/dev/kvm` is optional: if writable, the VM uses `-enable-kvm -cpu host`; otherwise it runs under emulation and is slower (see `VKMS_TIMEOUT`).

## Usage

```sh
scripts/vkms-harness-test.sh                 # the built-in command list
scripts/vkms-harness-test.sh COMMANDS-FILE   # your own command list
```

The script takes one optional positional argument and parses no flags:

| Argument | Meaning |
| --- | --- |
| `COMMANDS-FILE` | Lines for vm-init's `/commands.list`, copied into the VM as is. Must be readable, else exit 2. Default: the list the script writes itself. |

Environment variables:

| Variable | Default | Meaning |
| --- | --- | --- |
| `VKMS_KERNEL` | `/usr/src/linux/arch/x86/boot/bzImage` | Kernel image to boot. |
| `VKMS_MODULES` | `/lib/modules/$(uname -r)` | Module directory; must contain `modules.dep` and the same build as the kernel. |
| `VKMS_TIMEOUT` | `900` | Seconds allowed for the whole VM run (`timeout`). |

### Command list format

The format is that of `tools/vm-init`, one command line per line, words separated by white space, no quoting:

- `cmd args...`: run and wait; passes if it exits with 0.
- `& cmd args...`: start in the background; killed after the last line.
- `wait <path>`: wait up to 10 s for the path to exist.
- `#` starts a comment.

The harness is at `/bin/drm-harness` in the VM, the device is `/dev/dri/card0`, and `/tmp` is a tmpfs.

### Built-in command list

All with `--seconds 1`: `two-planes`, `zorder-alpha`, `move`, `combos`, `rotate`, `all`; then `modeset` and `recovery --loss master` / `--loss disable` (neither is part of `all`); then the kernel-lease scenarios (`--source kernel-lease revoke` and `all`); external DMA-BUF producers (`--producers external all`, and with `kernel-lease revoke`); finally a lease over drm-lease-manager's protocol (`drm-harness lessor` in the background, then `--source dlm --lease vkms --dlm-dir /tmp all`). The comments in the script say why each is there.

## Output

- Commands' output (from the first `--- command:` line to `VM_TEST_RESULT=`) is printed to stdout.
- The full console is kept in `target/vkms-harness/console.log`; the build tree is `target/vkms-harness/` (recreated on each run).

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | `VM_TEST_RESULT=0` was on the console: every command exited with 0. |
| 1 | The console has no `VM_TEST_RESULT=0`: a command failed, the VM timed out, or it never got far enough (check `console.log`). |
| 2 | Setup failed before the VM: kernel image not readable, no `modules.dep`, `COMMANDS-FILE` not readable, vkms module not found, or a compressed module. |
| other | `cargo build` failed (set -e), with cargo's status. |

## Relation to `scripts/vkms-vm-test.sh`

Both scripts build the same VM: the host's kernel image, an initramfs with `tools/vm-init` as PID 1, and the vkms modules loaded in dependency order (`enable_overlay=1 enable_cursor=1` on vkms only). They share the `VKMS_KERNEL` and `VKMS_MODULES` variables and the exit-2 setup errors. They differ in what they run:

| | `vkms-vm-test.sh` | `vkms-harness-test.sh` |
| --- | --- | --- |
| Runs | the ignored tests of `composition-drm`'s `vkms` test, and the `twoplane` example | `drm-harness` scenarios (`tools/drm-harness`) |
| Command list | fixed | built in, or `COMMANDS-FILE` |
| VM | 512 MB, 300 s fixed | 1024 MB, 2 CPUs, `VKMS_TIMEOUT` |
| Pass condition | `VERDICT twoplane: PASS` and `VM_TEST_RESULT=0` | `VM_TEST_RESULT=0` |
| Work directory | `target/vkms-vm/` | `target/vkms-harness/` |

## Not for

Boards, DRM-lease or compositor behaviour of a real device, timing, or any milestone claim. Those need the board and a dated measurement (see `CONTRIBUTING.md`).
