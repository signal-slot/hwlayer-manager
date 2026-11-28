# vc4-hvs-session-run.sh

One-pass session runner for the Raspberry Pi HVS boards: the RPi5 (BCM2712 HVS6) and the reTerminal CM4 (BCM2711 HVS5). It shortens a board lease: copy, run all steps under the display guard, capture, clean up, report.

## Usage

```sh
BOARD_SSH='<ssh options and target>' [BOARD_SUDO='sudo -n'] \
scripts/vc4-hvs-session-run.sh [--dry-run] --bin-dir DIR --out DIR \
    --step 'NAME:BINARY ARGS...' [--step ...]
```

- `BOARD_SSH` (environment, not needed for `--dry-run`): the ssh target and options, split on whitespace. It is never written to the output files; every word of it (and the host part of `user@host`) is replaced by `<board>` in the captured text. A host name printed by a tool itself is not covered; check the outputs with `scripts/check-no-board-secrets.sh` before committing anything.
- `BOARD_SUDO` (optional): prefix for the guard and the steps when the login is not root.
- `SSH_BIN` (optional): the ssh program, default `ssh`; the host test sets it to a fake.
- `--bin-dir DIR`: already cross-built aarch64 binaries. The runner copies those present of `probe props scale_probe pending_probe mode_test downscale_probe` (`downscale_probe` is optional) and `scripts/display-guard.sh`.
- `--step NAME:CMD`: `CMD` starts with one of the copied binaries, e.g. `--step 'q1:scale_probe /dev/dri/card0 --max-factor 64'`. Allowed characters: letters, digits and `_ . / = , : + -` and space. No shell metacharacters; anything else is refused with exit 2. Steps run in the order given.
- `--dry-run`: validates the arguments and prints the commands with `ssh <board>`; runs no ssh, creates nothing.

## What it does

1. `mktemp -d /tmp/hvs-session.XXXXXX` on the board; copies the binaries and `display-guard.sh` there with `tar` over ssh.
2. `uname -r`; the session start in host UTC.
3. `display-guard.sh stop` (a display manager is stopped only for the run).
4. Each step in the work directory. Per step `OUT/NN-NAME.stdout`, `.stderr`, `.exit` (the exit code) and `.header` (host-UTC `DATE`, `KERNEL`, step name, command). The `.stdout` file holds only the tool's own output.
5. `display-guard.sh restore`, then `detect`, then `rm -rf` of the work directory and a check that it is gone. This also runs when the runner is interrupted.
6. `OUT/summary.txt` and stdout: the guard lines (`DISPLAY-GUARD restored <unit> active`), `DISPLAY-MANAGER: active again` or `NOT confirmed active`, and `CLEANUP:` removed or FAILED.

Exit status: 0 all steps exited 0 and guard and cleanup succeeded; 1 a step exited non-zero (recorded, the run continues); 2 usage error; 3 the guard restore or the cleanup failed, or the copy failed.

## What it never does

No reboot, no module loading, no change to DRM, udev, seat or boot configuration: the only remote commands are `mktemp`, `tar`, `uname -r`, `display-guard.sh stop|restore|detect`, the steps themselves and `rm -rf` of its own directory. Steps use the master or kernel-lease source as the examples do.

## Tags

The host test (`scripts/vc4-hvs-session-run.test.sh`) checks the runner against a fake ssh only. The runner has not been run on a board; results it captures become *observed* only in the session file that quotes them with date, host UTC and kernel.
