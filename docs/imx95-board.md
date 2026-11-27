# Toradex Verdin iMX95: cross-building and running on the board

The i.MX95 board runs Toradex's Reference Multimedia Image (Yocto, glibc
userland, NXP kernel 6.6), not Torizon. Binaries are cross-built on the host
and copied to `/tmp` on the board. Nothing is installed into the image, so the
binaries must run there as they are.

This page uses placeholders only. `<board>` stands for the ssh destination
(`user@host`) you were given for the board. Never write real addresses, host
names, users or serial paths into the repository, commits or reports.

## Cross-build

```sh
scripts/cross-aarch64.sh -p composition-drm --example probe
scripts/cross-aarch64.sh -p <pkg> --bin <name>
```

The script passes every argument to `cargo build --release`, builds into
`target/` inside the workspace, and prints the paths of the executables it
produced on stdout (progress and cargo diagnostics go to stderr).

- Preferred target: `aarch64-unknown-linux-musl`, fully static. It is linked by
  `rust-lld` with self-contained musl objects, so no aarch64 gcc is needed and
  the result does not depend on the board's libc. The script runs
  `rustup target add` if the target is missing (needs network once).
- Fallback: `aarch64-unknown-linux-gnu` with a system cross linker
  (`aarch64-linux-gnu-gcc` or similar) if one is on `PATH`. That binary is
  dynamically linked against glibc; it needs a glibc on the board at least as
  new as the one it was linked against (the board's is recent enough for
  ordinary toolchains, but check with `ldd`).
- `HWL_AARCH64_TARGET=musl|gnu` forces one of the two.
- The linker settings are environment variables set inside the script. There is
  deliberately no `.cargo/config.toml`, which would also affect host builds.

Check the result:

```sh
file target/aarch64-unknown-linux-musl/release/examples/probe
# ELF 64-bit LSB executable, ARM aarch64, ... statically linked
```

## Copy to the board and run

```sh
scp target/aarch64-unknown-linux-musl/release/examples/probe <board>:/tmp/probe
ssh <board> 'chmod +x /tmp/probe && /tmp/probe'
```

Binaries and any output files go to `/tmp` only. `/tmp` is memory: remove what
you copied when done (`ssh <board> 'rm -f /tmp/probe'`). DRM programs need the
display, so see the Weston rule below.

## Board rules

- Write only under `/tmp`. Do not modify the root filesystem.
- Do not install packages (no opkg/dnf/pip, no copying into `/usr`, `/etc`).
- Do not change the U-Boot environment (no `fw_setenv`, no `saveenv`).
- Never write `mmcblk0boot0` or `mmcblk0boot1` (the eMMC boot partitions).
- Weston owns the display by default and holds DRM master. Taking the display
  for a DRM test means stopping it (for example `systemctl stop weston` or the
  unit the image uses). Restore it before you leave and confirm it is running
  again (`systemctl start weston`, then `systemctl is-active weston`). If you
  cannot restore it, say so in the report.
