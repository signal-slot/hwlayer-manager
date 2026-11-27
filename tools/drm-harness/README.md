# drm-harness

A minimal producer harness for `composition-drm` (SoW WP2, M1). It drives
the engine on one output of a real DRM device, on a descriptor that is DRM
master, a kernel DRM lease or a lease from AGL drm-lease-manager, with
buffers it allocates itself (dumb buffers; NV12 through a DMA-BUF export and
import) or that separate producer processes supply as DMA-BUFs, and prints
what happened as one-line records.

It is a board tool: `cargo test` only runs its unit tests (statistics,
record formatting, command line, and the two socket protocols over socket
pairs) and never opens a device.

## Build

On the board's architecture, statically linked, with no cross C toolchain:

```sh
rustup target add aarch64-unknown-linux-musl
cargo build --release -p composition-drm-harness --target aarch64-unknown-linux-musl \
    --config 'target.aarch64-unknown-linux-musl.linker="rust-lld"'
# target/aarch64-unknown-linux-musl/release/drm-harness
```

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
| `all`          | All of the above, in that order; `revoke` only with `--source kernel-lease`. |

| Option | Default | |
|--------|---------|---|
| `--device <PATH>` | `/dev/dri/card0` | device node to open |
| `--connector <NAME>` | first connected | connector name or prefix, e.g. `LVDS` |
| `--profile generic\|imx95` | `generic` | `DriverProfile` handed to the backend |
| `--seconds <N>` | 5 | how long each picture is held for a camera |
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
- `resources`: wall time, user and system CPU time, CPU %, RSS and peak RSS
  (`VmHWM`) for the scenario.
- `VERDICT <scenario> PASS|FAIL <reason>` ends every scenario; `all` adds
  `SUMMARY passed=.. failed=..`. The exit status is 0 only if all passed.

Measurement notes:

- Latency runs from just before `Engine::commit` to the moment the harness
  sees the `Presented` event; the kernel's vblank timestamp is not available
  through `composition-drm`, so wake-up time is included.
- The harness waits for each frame's latch before drawing the next one, so a
  missed vblank in `rotate` or `move` may be the harness's own drawing and
  validation time, not the display's.
- The first frame of `rotate` includes the modeset and is left out of the
  latency and interval statistics (`warmup_frames=1`).

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
first client.

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
`revoke`, which needs `--source kernel-lease`.
