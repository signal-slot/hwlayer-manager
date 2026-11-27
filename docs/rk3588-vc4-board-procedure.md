# Board session procedure: NanoPC-T6, Raspberry Pi 5, reTerminal (CM4)

The rules a sweep or verify session follows when it runs the `composition-drm`
examples on the NanoPC-T6 (RK3588 VOP2), the Raspberry Pi 5 (BCM2712, HVS6) or
the reTerminal (CM4, BCM2711, HVS5). It is a procedure, not a result. Every run
under it is **portability evidence** for the profile work and **never counts
toward M1, M2 or M3**.

Tags: statements about board behaviour carry "observed" with a date and the
source, or "inferred", or "read from source". Anything untagged is a rule of
this procedure, not a claim about a board.

The page holds no addresses, host names, users, serial numbers, UART paths or
credentials. `<board>` is the ssh destination from the farm lease; `<dir>` is the
session directory under `/tmp` on the board.

## 1. Rules

1. DRM access only as **DRM master** or through a **kernel lease**. No other
   source (not `dlm`, not a changed seat). Observed 2026-11-27, NanoPC-T6, kernel
   6.18.55-current-rockchip64: both sources worked, the kernel lease holding
   primary 33 and overlays 45, 51, 57, 63, 69, 75
   ([`linux-board-scenarios.md`](linux-board-scenarios.md)).
2. Never change DRM, udev, seat or boot configuration. Never install packages.
   Never edit anything outside `<dir>`.
3. Copy binaries and write run output only under `/tmp` on the board.
4. A display manager or compositor that holds the display is stopped for the run
   and shown active again at the end (section 4).
5. No board address, host name, IP, serial number, UART path or credential goes
   into any file, commit message or report (section 6).
6. Results are portability evidence only: they never count toward M1, M2 or M3.
7. Do not use the Verdin iMX95, Verdin AM62P, Armadillo X2 or STM32MP257F-DK, or
   their consoles and cameras. The Raspberry Pi 4 board used by plan #457 is also
   off limits here.
8. Probe and TEST_ONLY runs never need to change what the display shows; the
   `probe` example only reads resources (read from source,
   `crates/composition-drm/examples/probe.rs`). Examples that commit need master
   or a lease.

## 2. Build on the host

Cross-build for aarch64 with [`scripts/cross-aarch64.sh`](../scripts/cross-aarch64.sh).
It prints the executable paths on stdout and builds into `<workspace>/target/`,
never `/tmp`. It prefers `aarch64-unknown-linux-musl` (static, linked by
rust-lld); `HWL_AARCH64_TARGET=musl|gnu` forces a choice (read from source,
the script header).

```sh
scripts/cross-aarch64.sh -p composition-drm --example probe
scripts/cross-aarch64.sh -p composition-drm --example twoplane
scripts/cross-aarch64.sh -p composition-drm --example scale_probe
```

Other examples of the plan (`kms_sweep`, `profile_check`) are built the same way
once they exist on the branch under test. Whether a given farm node can run the
cross build at all is a separate check (plan item aarch64-cross-check); if it
fails, report that and stop, do not install a toolchain by hand.

## 3. Copy to `/tmp` only

```sh
ssh <board> 'mkdir -p /tmp/<dir>'
scp <executable paths> <board>:/tmp/<dir>/
```

Run from `/tmp/<dir>`; keep logs there. Fetch the logs back to the host
workspace (not `/tmp` on the host: use the task's `$TMPDIR`), then remove
`/tmp/<dir>` on the board at the end. Use the access details the lease gives;
do not copy them into any file.

## 4. DRM access and the display owner

Before the first run, find out who holds the display and whether the card is
free (all commands read-only):

```sh
ssh <board> 'systemctl is-active display-manager gdm lightdm sddm weston 2>&1'
ssh <board> 'ls /dev/dri; (fuser -v /dev/dri/card* 2>&1 || true)'
```

- Observed 2026-11-27, NanoPC-T6: display-manager, gdm, lightdm and sddm were
  all inactive before and after; nothing was stopped and no process held the
  device ([`linux-board-scenarios.md`](linux-board-scenarios.md)). Other boards
  are not known; treat them as **inferred** unless the session records the
  output.
- If a unit is active and holds the display, stop it for the run and restore it
  afterwards. Use the unit that was active:

```sh
ssh <board> 'sudo systemctl stop <unit>'     # before the first run
# ... runs ...
ssh <board> 'sudo systemctl start <unit> && systemctl is-active <unit>'
```

  The report shows the `systemctl is-active` output after the runs, and it must
  say `active`. If it does not, say so in the report. Stopping and starting a
  unit is allowed; disabling it, editing its configuration or changing seat
  assignment is not (rule 2).
- Where the board does not use systemd, use the equivalent service command and
  show its status line the same way.
- Choose the source per run: `master` after the owner is stopped, or
  `kernel-lease` through a lessor. Say in the report which one each run used.
- Card numbers differ between boards and between boots (**inferred**); take the
  card from the `probe` output, not from a remembered number.

## 5. Run

```sh
ssh <board> '/tmp/<dir>/probe --raw /dev/dri/cardN' > probe-raw.txt
ssh <board> '/tmp/<dir>/twoplane /dev/dri/cardN --frames 600 --profile <name>' > twoplane.txt
```

`probe [--raw]` and `twoplane` options are listed in the headers of
`crates/composition-drm/examples/probe.rs` and `twoplane.rs` (read from source);
`twoplane --profile NAME` takes the profile names that exist on the branch under
test. Run the examples the session item names with the options it gives.

## 6. Scrub and check the output

Scrub each text before it leaves the session directory
([`scripts/scrub-kms-dump-README.md`](../scripts/scrub-kms-dump-README.md)):

```sh
python3 -I scripts/scrub-kms-dump.py --hostname <board-name> probe-raw.txt > probe-raw.scrubbed.txt
```

The script removes blob bytes, MAC and IP addresses, serial values, tty paths,
`user@host` and credentials. It lowers the risk but does not prove a dump
clean: read the scrubbed file, and list host names with `--hostname` (only
names given are found; keep the name itself out of every file).

Then run the project checks on the branch that carries the data:

```sh
scripts/check-no-board-secrets.sh        # added lines and commit messages vs the base ref
scripts/check-test-data.sh               # dated board data under crates/composition-drm/tests/data
scripts/check-doc-links.sh
scripts/check-doc-tags.sh docs/<file>.md
```

[`check-test-data.sh`](../scripts/check-test-data.sh) needs, per `tests/data/<soc>/`
folder, a README, every file named in it, and a date in each file's name, first
10 lines or README line. Board data files therefore start with a dated header
(for example `# observed YYYY-MM-DD, kernel <version>, probe --raw`).
[`check-no-board-secrets.sh`](../scripts/check-no-board-secrets.sh) scans the
diff, so run it after the data is staged or committed, not before.

## 7. Judge and summarise

| Evidence | Tool | Use |
|---|---|---|
| KVM picture of the panel | [`scripts/camera-check.py`](../scripts/camera-check.py), [README](../scripts/camera-check-README.md) | `python3 -I scripts/camera-check.py PICTURE EXPECT.json`; add `--compare PICTURE2` for a move test. Exit 0 PASS, 1 FAIL, 2 input error. The camera picture is upside down by default (`--rotation`, default 180). |
| `EXPECT` lines of `twoplane` | [`scripts/twoplane-expect.py`](../scripts/twoplane-expect.py) | `python3 -I scripts/twoplane-expect.py twoplane.txt --out <dir>` writes one `expect-<phase>.json` per phase that ran (skipped phases are listed, not written); feed each to `camera-check.py`. |
| planner vs TEST_ONLY lines of `profile_check` | [`scripts/profile-check-expect.py`](../scripts/profile-check-expect.py), [README](../scripts/profile-check-expect-README.md) | `python3 -I scripts/profile-check-expect.py run.log --expect-profile <name> [--markdown]`; ends with `VERDICT: PASS` or `FAIL`. Exit 1 on more DISAGREE lines than `--allow-disagree` (default 0). |
| `scale_probe`, `kms_sweep`, `profile_check` lines | [`scripts/sweep-summary.py`](../scripts/sweep-summary.py), [README](../scripts/sweep-summary-README.md) | `python3 -I scripts/sweep-summary.py run1.txt run2.txt` (or `--json`) prints markdown tables: scale limits, combination results, modifier acceptance, pending-commit replacement, agreement. |

- The camera needs the `kvm` resource of the board; the CM4 has no camera, so its
  verdict rests on TEST_ONLY agreement alone.
- A picture taken before the run's frame is on the panel shows the old content
  and FAILs; observed 2026-11-27, NanoPC-T6: two early pictures still showed the
  console and failed, the third passed
  ([`linux-board-scenarios.md`](linux-board-scenarios.md)). Take pictures after
  the first commit and keep the camera still between pictures of a compare.
- Report a result as observed only with its date and kernel version (`uname -r`
  on the board, kept as text). Anything carried over from the design docs stays
  tagged read from source or inferred.

## 8. End of session

1. Remove `/tmp/<dir>` on the board.
2. Show the display owner active again (section 4) when it was stopped, or state
   that none was running.
3. Run the section 6 checks.
4. The report lists: board, kernel, source used per run, display owner state
   before and after, tools and versions (commit) of the examples, verdicts, and
   the sentence that the runs are portability evidence that never count toward
   M1, M2 or M3.

If a rule stops the session (for example a unit cannot be restored, or the build
fails on the node), say which rule and stop that step; do not work around it.
