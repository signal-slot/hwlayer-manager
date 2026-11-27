# M2 scene on Linux: the `m2scene` example

Usage note for `crates/composition-drm/examples/m2scene.rs`, the Linux (A55, DRM backend) side of the WP3 / M2 Linux vs Zephyr comparison (`SoW.md` §5.1, WP3). Written 2026-11-28.

**No measurement has been made with this example.** It has not been run on any board, and no timing, count or picture from it exists in this repository. Nothing here is a hardware or timing claim. Its log is input for the comparison of [m2-linux-zephyr-comparison.md](m2-linux-zephyr-comparison.md), not a result.

Tags: everything below about the example's behaviour is **read from source** (`m2scene.rs`, `composition-drm/src/lib.rs` at the commit this was written on); the scene values are **read from source** (`m2_scene.rs`), whose 1280 x 800 mode is itself **inferred** there. Nothing about any display has been observed with it.

## What it does

It builds stage B of the scene in [m2-linux-zephyr-comparison.md](m2-linux-zephyr-comparison.md) §2 (pinned by `crates/composition-core/tests/m2_scene.rs`) at fixed geometry: `base`, full output, `ARGB8888`, z_order 0; `top`, `W/3` x `H/3`, `ARGB8888`, premultiplied, plane alpha `0x8000`, z_order 1, at (500, 300) on a 1280 x 800 output (scaled to the mode the backend reports otherwise). The move test is not part of it. It commits `--frames` frames (default 300) through `composition-drm`, each a non-blocking atomic commit with a page-flip event, and waits for that frame's page flip before the next one. Two buffer slots per layer alternate; the pixels are static.

On stdout it writes:

- a `RUN date=YYYY-MM-DD side=linux stage=B tool=m2scene-<version> time=<UTC> frames=N mode=device|dry-run` line, with the date from the system clock (UTC), in the form `scripts/m2-compare-report.py` reads;
- `DEVICE`, `KERNEL`, `DRIVER`, `MODE` and `SCENE` lines;
- `CETRACE v=1` lines in the format of [trace-format.md](trace-format.md): one `hdr`, then `validate`, `commit`, `latched`, `released` (and `dropped`, `latch_timeout`, `device_lost`, `caps_changed` if they happen). `t` and `at` are `CLOCK_MONOTONIC` nanoseconds; `latched` carries `tev` (the page-flip event's vblank time) and the extra field `vseq` (the CRTC vblank counter); `validate` and `commit` carry `dur`. The planner's `check` entries are left out. If the process cannot read its monotonic time, `hdr` says `clock=rel_ns` and the vblank time is written as `vbl_ns` instead of `tev` (see the example's module doc);
- `RESULT frames= committed= latched= dropped= timeouts=` and `VERDICT m2scene: DONE|FAIL <reason>`.

Exit status: 0 after `DONE`, 1 for a failed run, 2 for a usage error, a device that cannot be opened or no DRM master. The example never stops other processes.

## Build

Host (and the dry run, which opens no device and prints the RUN line, the scene and the plan the planner makes on the modelled i.MX95 DPU topology of `m2_scene.rs`):

```sh
cargo run -p composition-drm --example m2scene -- --dry-run
```

aarch64 Linux, with the existing cross script (static musl binary; the script prints its path):

```sh
scripts/cross-aarch64.sh --locked -p composition-drm --example m2scene
```

The binary is `target/aarch64-unknown-linux-musl/release/examples/m2scene`.

## Run

```sh
/tmp/m2scene --device /dev/dri/card0 --frames 300 --profile imx95 > /tmp/m2scene.log
```

Options: `--device PATH` (default `/dev/dri/card0`), `--frames N` (default 300), `--output N` (default 0), `--profile generic|imx95` (default `generic`), `--dry-run`.

The example needs DRM master. On an i.MX95 board the rules of [imx95-board.md](imx95-board.md) apply: copy the binary to `/tmp` and write the log there, install nothing, and if the compositor has to be stopped for the run, stop and restore it with `scripts/display-guard.sh` and confirm it is active afterwards. The comparison method (modes, frame counts, what counts) is in [m2-linux-zephyr-comparison.md](m2-linux-zephyr-comparison.md) and [m2-comparison-plan.md](m2-comparison-plan.md); the plan asks for at least 1000 frames for latency, so pass `--frames` accordingly.

## Status

| Check | Result |
| --- | --- |
| Host dry run | runs and prints the RUN line, scene and plan (2026-11-28, host only) |
| aarch64 cross build | builds (2026-11-28, host only; not run) |
| Run on vkms | not run (`scripts/vkms-vm-test.sh` runs the backend's vkms tests, not this example) |
| Run on a board | not run; no measurement |
