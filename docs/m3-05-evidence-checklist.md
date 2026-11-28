# M3-05 evidence checklist: what a real firmware link must show

Date: 2026-11-28. A checklist, not a result. Nothing here was built or run for this document, and no row claims that a check passed. It maps every step and check of `docs/m3-05-firmware-link-plan.md` §7 to the build product to inspect, the command, the expected result, and whether a host script already covers it. The expected results are the plan's pass conditions. The commands are the usual binutils and Zephyr ones and are **inferred**, not taken from a run.

Marks:

- **host-checkable**: needs only this repository, a Rust toolchain with `armv7r-none-eabihf`, and `arm-none-eabi-gcc` or `clang` plus `lld`.
- **needs the Zephyr workspace**: needs the firmware's own build (`MCUI_SHOWCASE_DIR`, `ZEPHYR_WORKSPACE`, `ZEPHYR_SDK_INSTALL_DIR`; see `docs/board-test-am62p-r5f.md`). Rows that also need the board say **and the board**.

## Build products to collect

`<b>` is the west build directory of the firmware. Names are Zephyr's defaults [inferred]; record the real paths in the report.

| Product | Where | Used by |
| --- | --- | --- |
| Build log with the cargo step | the output of `west build`, kept as a file | C1 |
| Engine library | `<b>/cargo/armv7r-none-eabihf/release/libcomposition_engine.a` (the board test's `CMakeLists.txt` puts it there) | S2, C4, C8 |
| Final ELF | `<b>/zephyr/zephyr.elf` | C2, C3, C4, C8 |
| Link map | `<b>/zephyr/zephyr.map` | C3 |
| Run output | the remoteproc trace buffer (see `docs/board-test-am62p-r5f.md`) | C5, C6, C7 |

Binutils: with the Zephyr SDK they are `$ZEPHYR_SDK_INSTALL_DIR/gnu/arm-zephyr-eabi/bin/arm-zephyr-eabi-{nm,readelf,size,objdump}`. On a host with Arm GCC they are `arm-none-eabi-*`. Below, `NM`, `READELF`, `SIZE` stand for one family; use one family for the whole session and write it down.

## Checklist

| ID | What it shows | Product and command | Expected result | Host script | Mark |
| --- | --- | --- | --- | --- | --- |
| Step 0 | CI job `arm-backend` is green on `main` | GitHub Actions run list for `.github/workflows/ci.yml`; record run id and result | green; if red or never run, fix first (a separate item) | none (a web lookup) | host-checkable (no build; needs network) |
| S1 | Backend plus engine link freestanding for both cores | `CC_KIND=clang scripts/check-arm-link.sh`, and `CC_KIND=gcc scripts/check-arm-link.sh` if Arm GCC is present | exit 0 for `thumbv7em` and `armv7r`; no undefined symbol | `scripts/check-arm-link.sh` (CI `arm-backend`) | host-checkable |
| S1a | Core and two-plane crates build for the R5F target (not in the plan's list; narrows S1 failures) | `scripts/check-armv7r-build.sh --strict` | last line `ARMV7R PASS` | `scripts/check-armv7r-build.sh` | host-checkable |
| S1b | The engine library links into a Cortex-R5 C program: no undefined symbol, `ce_engine_init` defined (the `ce_*` half of C2, on a stub image) | `scripts/check-ffi-r5f-link.sh` | last line `PASS` (`SKIP` means no toolchain, not a pass) | `scripts/check-ffi-r5f-link.sh` | host-checkable |
| S2 | The cargo step works on the firmware build host | `cargo build --locked --release -p composition-ffi-staticlib --no-default-features --features panic-handler --target armv7r-none-eabihf` on the Zephyr build host | `libcomposition_engine.a` exists under the target directory; no network or registry error | `scripts/check-ffi-r5f-link.sh` runs the same cargo command, on the host only | host-checkable on a host; **needs the Zephyr workspace** for "on the firmware build host" |
| C1 | The firmware build used the plan's cargo flags | `grep -n 'cargo build' <build log>` and read the line | contains `--locked --release -p composition-ffi-staticlib --no-default-features --features panic-handler --target armv7r-none-eabihf` | none | needs the Zephyr workspace |
| C2 | Final ELF: no undefined symbol; the engine's `ce_*` entry points are defined | `NM <b>/zephyr/zephyr.elf \| grep -E '^ +U '` (expect nothing); `NM <b>/zephyr/zephyr.elf \| grep -E ' [TtWw] ce_'` | no `U` lines except those the loader resolves, which the report names; every `ce_*` function the app calls is defined (at least `ce_engine_init`, `ce_present`, `ce_poll_event`) | the same `nm` rule on a stub image: `scripts/check-ffi-r5f-link.sh`, `scripts/check-arm-link.sh` | needs the Zephyr workspace (the real ELF); host scripts cover the stub image |
| C3 | Which input defines `__aeabi_memclr4`, `memset`, `memcpy` | `grep -n -B1 -A1 -E '__aeabi_memclr4\|memcpy\|memset' <b>/zephyr/zephyr.map`; `NM <b>/zephyr/zephyr.elf \| grep -E ' (__aeabi_memclr4\|memcpy\|memset)$'` | each symbol defined once in the ELF; the map names the providing member (`compiler_builtins` in `libcomposition_engine.a`, or the firmware's libc or libgcc); no `multiple definition` error in the log | `scripts/check-arm-link.sh` prints the provider of `__aeabi_memclr4` for its stub image | needs the Zephyr workspace |
| C4 | No allocator symbols from the engine | `NM <b>/zephyr/zephyr.elf \| grep -E '__rust_alloc\|__rdl_alloc\|__rust_alloc_error_handler'`; on the library: `NM -A <b>/cargo/armv7r-none-eabihf/release/libcomposition_engine.a \| grep -E '__rust_alloc\|__rdl_alloc'` | no match in either | none; the library grep can run on the host library built by S1b | library half host-checkable; ELF half needs the Zephyr workspace |
| C4a | No unwind or personality symbols (plan §4.1: abort on panic; pass condition is no undefined symbol) | `NM <b>/zephyr/zephyr.elf \| grep -E 'rust_eh_personality\|_Unwind_'`; `READELF -S <b>/zephyr/zephyr.elf \| grep -E '\.ARM\.ex(idx\|tab)'` | no undefined `rust_eh_personality` or `_Unwind_*`; unwind sections, if present, come from the firmware's C (record which) | undefined ones are caught by the `U` rule of C2 | needs the Zephyr workspace (host half: same grep on the S1b image) |
| C4b | One halting panic handler, no second copy of `core` (plan §4.1, §6 item 4) | `NM <b>/zephyr/zephyr.elf \| grep -i panic` | one handler from the library (the plan's `loop {}`); no `core::panicking` symbols from another Rust library | none | needs the Zephyr workspace |
| C5 | Stack high-water mark of the main thread and the interrupt after all scenes | the firmware's thread-analyzer or stack-space output in the trace buffer; configured size is `CONFIG_MAIN_STACK_SIZE=16384` in `tests/board/am62p_r5f/prj.conf` | below 75 % of the configured size (a proposal, plan §8); the value is printed in the report | none | needs the Zephyr workspace **and the board** |
| C6 | `ce_abi_check(CE_ABI_CHECK())` is true at start-up | source: `grep -n 'ce_abi_check' tests/board/am62p_r5f/src/*.c`; ELF: `NM <b>/zephyr/zephyr.elf \| grep ce_abi_check` shows it linked; run: the trace-buffer line it prints | the call exists before the first scene and the log says it returned true. `CE_ABI_CHECK_ADVISE()` is a separate check, needed only if the firmware uses `ce_advise` | none for the run; a host Rust test of `ce_abi_check` is a separate item | needs the Zephyr workspace **and the board** (the source grep is host-checkable) |
| C7 | Board SCENE/RESULT lines equal the host run's | host: `cargo test --workspace` (scene sequence behind the fake panel); board: the trace-buffer lines; compare status codes and counts | same status codes and counts | host side `cargo test --workspace`; `scripts/am62p-scenes.py` holds the expected counts | needs the Zephyr workspace **and the board** |
| C8 | Size | `SIZE <b>/zephyr/zephyr.elf`; `SIZE -t <b>/cargo/armv7r-none-eabihf/release/libcomposition_engine.a`; `READELF -S <b>/zephyr/zephyr.elf` | the numbers are recorded; no limit is set (plan §4.3) | the host `.a` can be measured after S1b, but it differs from the firmware's | ELF needs the Zephyr workspace; the `.a` is host-checkable |
| Pin | Header, library and `ce_twoplane.c` come from one commit (plan §6 item 6) | `git -C <checkout used by the build> rev-parse HEAD`; compare with the commit pinned in the showcase's `west.yml` | one commit, written into the report | none | needs the Zephyr workspace (the pin is in the showcase) |

## Float ABI (plan §3, §6 item 1)

Not a numbered check, but a link can pass and fault at run time. Inspect `READELF -A <b>/zephyr/zephyr.elf` and the same on the library members: both should list the hard-float VFP attributes, and the link log should have no ABI mismatch warning. Needs the Zephyr workspace; a host half runs on the S1b image when built with Arm GCC. The plan says the matching FPU claim for the AM62P R5F is not verified in this repository.

## Done condition (copied from plan §7; not met by this document)

M3-05 is done when S1, S2, C1 to C4, C6 and C7 are recorded as passed in `docs/m3-am62p-report.md` for one named commit of this repository in one real firmware build, and C5 and C8 have recorded values. If only the board test's image was built, the report says "linked in a Zephyr build" and the showcase half stays open. A green `arm-backend` job alone, a link without a run, or a host run do not close it.

## Order

Host-checkable now: Step 0, S1, S1a, S1b, S2 (on a host), the library halves of C4 and C8, and the source half of C6. Everything else waits for the firmware build; C5, the run half of C6, and C7 need the board.
