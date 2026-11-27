# drm-harness

A minimal producer harness for `composition-drm` (SoW WP2, M1). It drives
the engine on one output of a real DRM device as DRM master, with buffers it
allocates itself (dumb buffers; NV12 through a DMA-BUF export and import),
and prints what happened as one-line records.

It is a board tool: `cargo test` only runs its unit tests (statistics and
record formatting) and never opens a device.

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
| `all`          | All of the above, in that order. |

| Option | Default | |
|--------|---------|---|
| `--device <PATH>` | `/dev/dri/card0` | device node to open |
| `--connector <NAME>` | first connected | connector name or prefix, e.g. `LVDS` |
| `--profile generic\|imx95` | `generic` | `DriverProfile` handed to the backend |
| `--seconds <N>` | 5 | how long each picture is held for a camera |
| `--frames <N>` | 240 (`move`), 600 (`rotate`) | frame count |
| `--buffers <K>` | 3 | buffers per plane in `rotate` (3 to 16) |

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

The descriptor comes from `src/source.rs` (`Source::open`). Only a device
node used as DRM master exists today; a DRM lease or a lease-manager client
is meant to be added there as another `Source` variant.
