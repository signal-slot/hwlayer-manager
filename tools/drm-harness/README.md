# drm-harness

A minimal producer harness for `composition-drm` (SoW WP2, M1). It drives
the engine on one output of a real DRM device, on a descriptor that is DRM
master, a kernel DRM lease or a lease from AGL drm-lease-manager, with
buffers it allocates itself (dumb buffers; NV12 through a DMA-BUF export and
import) or that separate producer processes supply as DMA-BUFs, and prints
what happened as one-line records.

It is a board tool: `cargo test` only runs its unit tests (statistics,
record formatting, command line, and the two socket protocols over socket
pairs) and never opens a device. `scripts/vkms-harness-test.sh` runs it on
vkms in a throwaway QEMU VM (see [On vkms](#on-vkms)); that says nothing
about any hardware.

## Build

On the board's architecture, statically linked, with no cross C toolchain, use
the repository's script (it prints the executable paths on stdout):

```sh
scripts/cross-aarch64.sh        # probe example + drm-harness, no arguments
# or only the harness, as CI does:
scripts/cross-aarch64.sh --locked -p composition-drm-harness --bin drm-harness
# target/aarch64-unknown-linux-musl/release/drm-harness
```

If the active Rust cannot provide the musl target or a linker, the script
falls back to a private rustup in a cache directory,
`${HWLAYER_XBUILD_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/hwlayer-manager/xbuild}`
(a few hundred MB, first run needs network). Remove that directory to clear
it. Details: `scripts/cross-aarch64-README.md`. A bare `cargo build` is not
the recipe.

Copy the binary to the board (for example to `/tmp`) and run it as root.
Nothing else may be DRM master of the device: stop the compositor that owns
it first, or the harness stops with `not DRM master`.

## Usage

```
drm-harness [OPTIONS] <SCENARIO>
```

| Scenario       | What it does |
|----------------|--------------|
| `combos`       | Builds layer combinations (1 to N layers, one more than the output has planes, plane alpha, blend modes, up/downscaling, rotation, off-screen, NV12, an unsupported format) and reports for each whether the planner refuses it (with its reason) or whether atomic `TEST_ONLY` accepts or refuses it. Nothing is committed. |
| `two-planes`   | A full-screen buffer and a half-size buffer, allocated separately, overlapping, on two planes through `Engine::validate` and `Engine::commit`. Prints the layer → plane assignment and holds the picture. |
| `zorder-alpha` | Two overlapping rectangles (red, green): red over green, green over red, red at 50 % plane alpha, then a red buffer whose right half has 50 % premultiplied pixel alpha. Each step is labelled, says what should be seen, and is held. |
| `move`         | Moves the top plane across the screen for `--frames` frames over a checkerboard bottom buffer that is never redrawn: per-frame plane ids, the bottom buffer's checksum before and after, and that it is never released. |
| `rotate`       | `--buffers` buffers per plane on two planes for `--frames` frames. A buffer is redrawn only once the engine reported it `Released`; the harness checks Submitted → Latched → Released for every buffer and counts dropped frames, missed vblanks and violations. Measures submit → latch latency, frame interval, validate and commit time, CPU and memory. |
| `revoke`       | `--source kernel-lease` only. Rotates `--buffers` buffers per plane on two planes for `--frames` frames (default 60), then the in-process lessor revokes the lease between two frames. Checks that the next frame fails with `DeviceLost`, that a second one fails with `UnknownOutput` for the lost output (the backend reports no outputs once the loss is applied), that `DeviceLost` is reported exactly once, that the two buffers on screen are released exactly once and nothing else, that no buffer stays submitted or scanning, and that no output or plane is reported afterwards (as `leased_output_exposes_only_leased_planes_and_reports_revocation` in `crates/composition-drm/tests/vkms.rs`). |
| `modeset`      | Commits the `two-planes` picture on the connector's current mode for 1 s, then switches to the first other mode of the connector's list (differing in size or refresh, progressive, each size and refresh once) that atomic `TEST_ONLY` accepts. The switch is the harness's own blocking `ALLOW_MODESET` commit on the same descriptor: the new mode, a black buffer of the new size on the primary plane, the CRTC's other planes off. A new engine on that descriptor then finds the new size, plans for it and commits for `--seconds`. Then the same kind of commit puts the original mode back, and the CRTC's mode is read to check it. BLOCKED when the connector offers only one mode (no other progressive one). Not run by `all`. |
| `recovery`     | Display recovery without a lease (row A6). Commits the `two-planes` picture for `--seconds`, then loses the display while the engine commits (`--loss`). `master` (default): the engine's descriptor drops DRM master, and a second descriptor on the same node takes master and turns the CRTC off (`ACTIVE=0`), as a compositor taking the display would. `disable`: the CRTC is turned off with a commit on the engine's own descriptor, which stays master. Then one frame is offered; the engine must report the loss within 2 s: an error from `validate` or `commit`, or a `DeviceLost`, `LatchTimeout` (the engine's latch timeout is set to 500 ms and the harness passes the time with `advance_time`) or `Dropped` event. A latch, or nothing, fails. With `master`, the second descriptor is closed, master is taken back and a second frame is offered on the same engine (the CRTC is still off); only a silent stall fails there. Every buffer of the old engine must then be idle (no buffer `Submitted` or `Scanning`). The old engine and its descriptor are dropped, a new engine run opens the device again (its first commit is a modeset, which turns the CRTC on) and commits a different picture (green, yellow rectangle) for `--seconds`; at least one commit must latch, and the CRTC must be on at the end (if it is not, the harness turns it back on with the original mode). `--loss master` needs `--source master`. Not run by `all`. |
| `all`          | All of the above but `modeset` and `recovery`, in that order; `revoke` only with `--source kernel-lease`. |

| Option | Default | |
|--------|---------|---|
| `--device <PATH>` | `/dev/dri/card0` | device node to open |
| `--connector <NAME>` | first connected | connector name or prefix, e.g. `LVDS` |
| `--profile generic\|imx95` | `generic` | `DriverProfile` handed to the backend |
| `--seconds <N>` | 5 | how long each picture is held for a camera (in `revoke`: the screen after the revocation; in `modeset`: the commits after the switch; in `recovery`: the commits before the loss, and again those of the new run); at most 86400 |
| `--loss master\|disable` | `master` | how `recovery` loses the display, see the table above |
| `--frames <N>` | 240 (`move`), 600 (`rotate`) | frame count |
| `--buffers <K>` | 3 | buffers per plane in `rotate` and `revoke` (3 to 16) |
| `--source master\|kernel-lease\|dlm` | `master` | where the descriptor comes from, see [Sources](#sources) |
| `--planes <IDS>` | every overlay of the CRTC | `kernel-lease`: overlay plane ids to lease, comma-separated |
| `--lease <NAME>` | | `dlm`: lease name; the socket is `<dir>/<NAME>` |
| `--dlm-dir <DIR>` | `$DLM_RUNTIME_PATH`, else `/var/run/drm-lease-manager` | `dlm`: socket directory |
| `--producers local\|external` | `local` | who allocates and draws the buffers, see [Producers](#producers) |

Example for a camera run on the LVDS panel:

```sh
/tmp/drm-harness --connector LVDS --profile imx95 --seconds 10 all | tee /tmp/harness.log
```

## Output

Every line is `kind key=value ...`; values with spaces are double-quoted.
The main kinds:

- `session`, `output`, `plane`, `resource_group`: device, driver, master
  state, connectors with mode, resolution and refresh, and the planes as the
  engine sees them (after the profile).
- `step`: a labelled picture and what should be visible; `hold`: it stays
  for `seconds`.
- `assign`, `plan`: the plane assignment of a commit (layer, buffer, plane
  id and kind, zpos, alpha, blend, source and destination).
- `commit`: validate time, commit time and latency of one commit.
- `frame` (`move`), `combo` (`combos`), `progress`, `violation`,
  `move_result`, `combos_result`, `rotate_result`.
- `modeset`: `connector_modes` (the connector's list, `WxH@Hz`),
  `mode_candidate` (a mode tried with `TEST_ONLY`), `modeset step=switch` and
  `step=restore` (the harness's commit, its time, result and the CRTC's mode
  afterwards), `replan` (the size the new engine reports), `commit` records
  with `step=modeset-before` and `step=modeset-after`, and `modeset_result`
  (`from`, `to`, commits and latches before and after, `restored`).
- `recovery`: `hold` and `commit` records with `step=recovery-before` and
  `step=recovery-after`, `before_loss` (the buffers on screen), `loss`
  (`method`, `result`, whether the CRTC is still active), `loss_attempt`
  per frame offered after the loss (`report` is `error:<text>`,
  `event:<name>`, `latched` or `silent`; `after_ms`; the backend's failure
  log, `device_lost`, `pending_latch`, the events), `master_back`,
  `buffers_after_loss` (each buffer's state, `busy`), and `recovery_result`
  (`first`, `first_ms`, `reported`, `second`, `still_busy`, `master_back`,
  `relatched`, `crtc_active_end`).
- `resources`: wall time, user and system CPU time, CPU %, RSS and peak RSS
  (`VmHWM`) for the scenario.
- `VERDICT <scenario> PASS|FAIL|BLOCKED <reason>` ends every scenario; `all`
  adds `SUMMARY passed=.. failed=.. blocked=..`. BLOCKED means the scenario
  could not run here (`modeset` on a connector with one mode); it is never a
  pass. The exit status is 0 only if all passed, 1 if one failed, 3 if none
  failed but one was BLOCKED.
- SIGTERM and SIGINT stop the run instead of killing it, so that it still
  prints `resources` and `VERDICT` (the session ends, so the buffers are
  released). A stop never turns a failure into a pass:
  - `modeset` puts the original mode back before it ends, also on a stop.
  - `recovery` stopped before the loss is a cut-off. Once the display is
    lost, the CRTC is on again before the run ends: turned on by the new
    run, or, if a stop came before its first commit (also a cut-off), by
    the harness.
  - A stop during a hold (`two-planes`, `zorder-alpha`, `revoke`, after their
    checks; `modeset` once a commit latched on the second mode) ends the hold; the verdict is what the checks found, with
    `stopped by SIGTERM or SIGINT during the hold, buffers released` added
    to the reason. A PASS exits 0, a FAIL exits 1.
  - A stop before the checks were made is `FAIL ... stopped by SIGTERM or
    SIGINT before the checks were made`, exit 1.
  - Either way no further scenario runs; `all` prints `SUMMARY` for those
    that did, and exits non-zero if any of them failed.

Measurement notes:

- Latency runs from just before `Engine::commit` to the moment the harness
  sees the `Presented` event; the kernel's vblank timestamp is not available
  through `composition-drm`, so wake-up time is included.
- The harness waits for each frame's latch before drawing the next one, so a
  missed vblank in `rotate` or `move` may be the harness's own drawing and
  validation time, not the display's.
- The first frame of `rotate` includes the modeset and is left out of the
  latency and interval statistics (`warmup_frames=1`).

Pictures that are opaque are drawn in `XRGB8888` with the default
`BlendMode::Opaque`, so they need no blending and fit planes without a
`pixel blend mode` property (KMS blends those as premultiplied). Only the
layers that test pixel alpha or a blend mode (`pixel-alpha-premultiplied`,
the `argb-opaque` and `blend-*` combos) use `ARGB8888`.

## Sources

The descriptor comes from `src/source.rs` (`Source::open`); everything after
that works on the descriptor alone.

- `--source master` (default): the device node, made DRM master. Nothing
  else may be master: stop the compositor first.
- `--source kernel-lease`: an in-process lessor (`src/lease.rs`) opens the
  device node, becomes DRM master and leases, with
  `DRM_IOCTL_MODE_CREATE_LEASE` (the `drm` crate's `create_lease`), the
  chosen connector, the CRTC `DrmBackend` would pair with it, that CRTC's
  primary plane and the overlay planes (`--planes`, default every overlay
  whose `possible_crtcs` includes the CRTC; cursor planes are not leased).
  The engine gets only the lease descriptor (`DrmBackend::from_fd`). The
  `lease` record lists the lessee id and the leased object ids; the `plane`
  records are the planes the engine sees; `lease_check` compares the two and
  the session fails if the engine sees a plane outside the lease.
- `--source dlm --lease <NAME> [--dlm-dir <DIR>]`: a minimal client of AGL
  drm-lease-manager's `libdlmclient` protocol (`src/wire.rs`, read from
  drm-lease-manager at commit `68c48758c46d`, described in
  `docs/dlm-aarch64.md`): `SOCK_SEQPACKET` socket `<dir>/<NAME>`, a 4-byte
  `DLM_GET_LEASE` request, a 1-byte reply carrying the lease descriptor as
  `SCM_RIGHTS`; a closed connection is a refusal. The lease is released
  (`DLM_RELEASE_LEASE`) when the session ends. The `revoke` scenario does
  not apply: the lease manager, not the harness, is the lessor.

### `drm-harness lessor`

A small leaser in a separate process, speaking the same protocol, so that
`--source dlm` can be used without drm-lease-manager and the lease
descriptor crosses a process boundary:

```sh
mkdir -p /tmp/dlm
/tmp/drm-harness lessor --lease lvds --dlm-dir /tmp/dlm --connector LVDS &
/tmp/drm-harness --source dlm --lease lvds --dlm-dir /tmp/dlm --profile imx95 all
```

Each client that asks gets a fresh kernel lease, revoked when the client
releases it or disconnects; one lease at a time. `--once` exits after the
first client. A client that closes its lease descriptor before it
disconnects (as the harness does) has already ended the lease: the kernel
destroys it with the lessee's last descriptor, and the lessor records
`lease_ended` instead of `lease_revoked`.

## Producers

With `--producers external` (`src/producer.rs`) the buffers come from
separate processes: one `drm-harness producer` per layer, spawned by the
harness, its `SOCK_SEQPACKET` socket on standard input. A producer opens
`--device` and gives up DRM master if it got it, allocates its own dumb
buffers, draws them (the scenarios describe pictures as drawing steps that
travel over the socket), exports them as PRIME DMA-BUF descriptors and sends
them with size, format, modifier, offset and pitch. The harness imports
them with `DrmBackend::import_dmabuf`.

A buffer the producer sent is lent to the harness. When the engine reports
it `Released`, the harness passes that on (`Released` message) and only
then asks the producer to redraw it; a producer refuses to draw into a lent
buffer, and a scenario fails if any producer refused a request or did not
exit cleanly (`producer_exit`, `producers` records). A buffer that was drawn
but never shown is handed back before it is redrawn (`returned_unshown`).
The bottom buffer of `move` is checksummed by its producer. NV12 and
non-32-bit buffers of `combos` stay with the harness. The wire format is in
the module documentation of `src/producer.rs`.

Every scenario runs with every source and both producer modes, except
`revoke`, which needs `--source kernel-lease`, and `recovery --loss master`,
which needs `--source master`.

## On vkms

`scripts/vkms-harness-test.sh` builds the harness statically for
`x86_64-unknown-linux-musl` and runs it as root in the same throwaway VM as
`scripts/vkms-vm-test.sh` (the host's kernel, an initramfs with
`tools/vm-init`, vkms loaded with overlay and cursor planes). Nothing is
loaded on the host. By default it runs every master-descriptor scenario,
`kernel-lease` with `revoke` and `all`, `--producers external`, and
`--source dlm` against `drm-harness lessor`, each with `--seconds 1`; a
file of other command lines can be given instead (the format is in
`tools/vm-init/src/main.rs`). It prints the records and exits with 0 only
if every command did.

vkms has no `zpos`, `alpha` or `pixel blend mode` properties (premultiplied
only) and no scaling or rotation, so the planner refuses `plane-alpha-50`
of `zorder-alpha`, and the alpha, coverage, scaling and rotation combos:
`zorder-alpha` fails there, which is a property of vkms, not of the
harness. A vkms run says nothing about any hardware.
