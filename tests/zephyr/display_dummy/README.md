# display_dummy: the engine over Zephyr's dummy display, under QEMU

**This is not a hardware result.** It runs on QEMU's `qemu_cortex_m3` (Cortex-M3), against upstream Zephyr's `zephyr,dummy-dc` display, which draws nothing, has no VSYNC and no latch of its own. It says nothing about the i.MX95 M7, the DPU, any panel, or any timing. It does not count towards M1, M2 or M3.

What it checks: that the engine's backend contract maps onto Zephyr's display API as `docs/imx95-m7-backend-contract.md` describes (sections 3.1 to 3.4), off the board. The app links `libcomposition_engine.a` (crate `composition-ffi-staticlib`, built by cargo for `thumbv7m-none-eabi`, no std, `panic-handler`) through `crates/composition-ffi/include/composition_engine.h`, and implements the four backend functions in `src/main.c`:

| Backend function | What it does here |
| --- | --- |
| `capabilities` | One output sized from `display_get_capabilities()`, refresh unknown (0). Plane 0: primary, whole output only. Plane 1: overlay, a rectangle. Both ARGB8888 (the only format the dummy driver and the engine both name), opaque, no scaling. |
| `check` / `commit` | Validate every assignment (buffer known, format, source inside the buffer, destination inside the output, no scaling, primary plane covers the output) before any write, then one `display_write(x, y, ...)` per layer, lower plane first, pitch in pixels. |
| `poll_event` | Reports `LATCHED` for the last commit. **Synthesized**: the dummy display returns from `display_write()` at once and has no VSYNC, so the latch is declared at that return, as section 3.3 of the contract says a stock blocking driver permits. |

Scenes run: a two-layer frame (64x32 background, 32x16 foreground at 16,8), a background-only frame (the foreground buffer is released), refused frames (an RGB565 layer, three layers on two planes) that must reach no `display_write`, and an empty frame. The test records each `display_write` call (position, size, pitch, first pixel) and checks it against the layer. It prints one `PASS:` or `FAIL:` line per check, then `RESULT: PASS` or `RESULT: FAIL`, and the number of main-stack bytes never used.

## Build and run

Needs a Zephyr v4.5.0 workspace and the Zephyr SDK with the `arm-zephyr-eabi` toolchain (as in Zephyr's getting started guide), `cargo` with the `thumbv7m-none-eabi` target (`rustup target add thumbv7m-none-eabi`), and `qemu-system-arm`.

```sh
west build -p always -b qemu_cortex_m3 tests/zephyr/display_dummy
timeout 30 west build -t run
```

The guest does not exit by itself (the board has no clean exit for it), so `timeout` stops QEMU after the `RESULT:` line. Judge by that line. The build runs `cargo build --locked` for the engine; the result is in the build directory, not in the repository.

`CONFIG_DISPLAY` makes Zephyr start QEMU with `-display sdl` instead of `-nographic`. QEMU then waits on a window and the console is not on stdio. `CMakeLists.txt` therefore sets `QEMU_EXTRA_FLAGS` to `-display none`.

## Recorded run

Run of 2026-11-27 (UTC, the clock of the build machine), Zephyr v4.5.0, Zephyr SDK 1.0.1 (GCC 14.3.0), QEMU `qemu-system-arm`, board `qemu_cortex_m3`. Build: flash 57 KB, RAM 55608 B of 64 KB. Output:

```
*** Booting Zephyr OS build v4.5.0 ***
display_dummy: engine over the dummy display, board qemu_cortex_m3/ti_lm3s6965
NOT a hardware result: QEMU, a display that shows nothing, no VSYNC
PASS: dummy display device ready
PASS: display reports the size of the overlay node
PASS: display takes ARGB_8888
PASS: display_blanking_off
PASS: engine ABI check (header and library agree)
PASS: ce_engine_init
PASS: ce_register_buffer
PASS: ce_register_buffer
PASS: ce_register_buffer
PASS: background buffer written and finished
PASS: foreground buffer written and finished
PASS: RGB565 buffer written and finished
PASS: ce_present of the two-layer scene
PASS: two display_write calls for two layers
PASS: first write: background, full screen, background pixels
PASS: second write: foreground at (16, 8), foreground pixels
PASS: both buffers submitted or scanning, no longer writable
PASS: one PRESENTED for the submission after poll
PASS: both buffers scanning after the latch
PASS: a scanned buffer cannot be rewritten
PASS: ce_present of the background alone
PASS: one display_write for one layer
PASS: PRESENTED for the second submission
PASS: foreground released, background still shown
PASS: foreground idle again
PASS: RGB565 layer refused with a format reason
PASS: three layers on two planes refused
PASS: refused frames reached no display_write
PASS: refused frames produced no events
PASS: empty frame accepted
PASS: empty frame presented, background released
main stack: 5748 of 12288 bytes never used
31 checks, 0 failed
RESULT: PASS
```

## Notes and limits

- **Stack.** The engine plans on the caller's stack. About 6.5 KiB of the main stack were used in this run. With a 4 KiB main stack the program ended in a usage fault after printing its result (observed in earlier runs of this app, not recorded here); `prj.conf` sets 12 KiB. Any M7 app that calls `ce_present` needs the same care.
- **Why `qemu_cortex_m3`.** In Zephyr v4.5.0 the `mps2/an500` (Cortex-M7) board is not runnable under QEMU: its `board.cmake` lists only the Arm FVP. `qemu_cortex_m3` is a QEMU board, so the engine is built for `thumbv7m-none-eabi`, not for the `thumbv7em-none-eabihf` of the CI builds.
- **Dummy display.** Upstream's `zephyr,dummy-dc` supports ARGB8888 and RGB888 (plus mono formats), accepts any `display_write`, and implements no event callback. Whether the DPU driver would report a latch and what it would mean is the question of `docs/imx95-m7-backend-contract.md` section 3.4; nothing here answers it.
- **Size.** 64x32 pixels, so the buffers and the engine's 28 KiB of storage fit the 64 KiB of RAM of the board.
- Upstream Zephyr only; no showcase code. The dummy display node comes from `boards/qemu_cortex_m3.overlay`, modelled on upstream's `samples/drivers/display/dummy_dc.overlay`.
