# rk3588-session-run.sh

One-pass session runner for the NanoPC-T6, to keep a board lease short.
Not run on a board by this change; only its `--dry-run` plan is host-tested.

```sh
export BOARD_SSH=<login given by the lease>   # never printed or written to a file
scripts/rk3588-session-run.sh --bin-dir <dir with the aarch64 examples> \
  --out "$TMPDIR/t6-run" \
  --step "props /dev/dri/card0" \
  --step "kms_sweep combos /dev/dri/card0 --sets 32"
```

- Steps: `--step "EXAMPLE ARGS..."` (repeatable) or `--steps FILE` (one per line, `#` comments). Examples: probe, props, kms_sweep, scale_probe, plane_commit, pending_probe, zpos_probe, twoplane, mode_test. Binaries must be cross-built already (`--bin-dir`); only those used are copied.
- Per step, on the board: `display-guard.sh stop`, the example, `display-guard.sh restore`. Output goes to `OUT/NN-EXAMPLE.txt` with a host-UTC header, the raw stdout/stderr, `EXAMPLE-EXIT=` and the guard lines.
- A fresh `/tmp/rk3588-session.*` directory holds the binaries and the guard state; it is removed on exit, also on error.
- The last line says whether the display manager is active again. Exit is non-zero if a step or a restore failed.
- `BOARD_SUDO` (optional, e.g. `sudo -n`) prefixes the guard and example commands on the board.
- `--dry-run` prints the plan without ssh and without needing `BOARD_SSH`.
- It does not reboot, load modules, or change DRM, udev, seat or boot configuration. Output files hold only what the examples print; run `scripts/check-no-board-secrets.sh` before committing any of it.
