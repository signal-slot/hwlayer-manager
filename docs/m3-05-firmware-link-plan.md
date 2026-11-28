# M3-05 plan: the engine linked into a real firmware build

Date: 2026-11-28. A desk plan. Nothing here was run on a board, and nothing was built for this document. Every claim carries a tag:

- **read from source**: read in a file of this repository; the file is named.
- **inferred**: a conclusion drawn from what was read, or a general fact about the toolchain that this repository does not state. It needs a check.

The showcase (a private repository that pins a commit of this repository in its `west.yml`) was not read. Nothing below says what it contains. Where its build must decide something, the plan names the decision and does not guess the answer.

## 1. What M3-05 asks

`docs/open-issues.md` M3-05: CI compiles the C backend for the two Arm targets and links it into a freestanding image with `__aeabi_memclr4` resolved (job `arm-backend`, host only, nothing run). Not done: "the link in a real firmware build". Closing evidence asked for: "the link in the firmware build (M3 board run)". [read from source: `docs/open-issues.md`, M3-05]

SoW §9 M3 acceptance conditions are: two-plane composition, correct VSYNC latch tracking, safe frame release, no visible intermediate-state regression, and a recorded difference matrix versus i.MX95. [read from source: `SoW.md` §9, M3] M3-05 adds no new behaviour. It asks that the engine and the backend, as built by the firmware's own build system, are the same code that CI checks, and that the image runs.

## 2. What a firmware build needs from this repository

| Need | Where | Tag |
| --- | --- | --- |
| The engine as a static library, `libcomposition_engine.a` (crate `composition-ffi-staticlib`, `[lib] name = "composition_engine"`, `crate-type = ["staticlib"]`) | `crates/composition-ffi-staticlib/Cargo.toml` | read from source |
| Cargo features: `std` (default) links Rust std; `panic-handler` adds a halting panic handler for `no_std` builds. A firmware build uses `--no-default-features --features panic-handler` | same | read from source |
| The C header `composition_engine.h`. It is in `crates/composition-ffi/include/`, not in a top-level `include/` (that directory does not exist) and not in the staticlib crate. It is written by hand; `ce_abi_check` checks the structure sizes at run time | `crates/composition-ffi/include/composition_engine.h`, `crates/composition-ffi/src/lib.rs` (module comment) | read from source |
| The two-plane backend, written in C: `backends/twoplane/ce_twoplane.c` and `ce_twoplane.h`. The header includes `<composition_engine.h>`, `<stdbool.h>`, `<stdint.h>`; the source includes `<stddef.h>` only. The firmware compiles the `.c` file itself ("built by the firmware that uses it") | `backends/twoplane/ce_twoplane.[ch]`, `backends/twoplane/Cargo.toml` (comment) | read from source |
| The port: the firmware fills `struct ce_twoplane_hw` and the port (including `irq_lock` and `irq_unlock`, which in Zephyr are `irq_lock` and `irq_unlock`). This is firmware code, not part of this repository | `backends/twoplane/ce_twoplane.h`; `tests/board/am62p_r5f/src/hw_am62p.c` for the AM62P binding | read from source |
| A Zephyr module description (`zephyr/module.yml`) | none: `git ls-files` lists no such file, and no `CMakeLists.txt` or `Kconfig` at the repository root | read from source |
| A pin of this repository in the showcase's `west.yml` | in the showcase, not available | not read; do not guess |

Consequences:

1. **There is no Zephyr module layout in this repository** [read from source]. The board test reaches the repository by relative path (`HWLAYER_ROOT` is three levels above `tests/board/am62p_r5f`). A firmware that has the repository checked out by west gets the same by path, whether or not it is declared as a module. The plan does not add a module file (a non-goal here). Whether the showcase wants one is a decision to record in step 1 of §7.
2. **The cargo step needs a Rust toolchain on the firmware build host**, with the target installed. This is a requirement on the showcase's build image. [inferred: `cargo build` is the only way this repository produces the library; there is no prebuilt artefact in the tree]
3. **A west checkout must have the whole repository**, not only `backends/twoplane`: the library is built from the workspace (`Cargo.toml`, `Cargo.lock`, `crates/`). [inferred from the `cargo build -p` in the board test's `CMakeLists.txt`, §5]

## 3. The Rust target: `armv7r-none-eabihf` against the `thumbv7em` builds of CI

What CI builds today [read from source: `.github/workflows/ci.yml`, `docs/ci.md`]:

| Where | Target | What |
| --- | --- | --- |
| job `checks` | `thumbv7em-none-eabihf` | `-p composition-core`; and `-p composition-ffi-staticlib --no-default-features --features panic-handler --release` |
| job `arm-backend` | `thumbv7em-none-eabihf` and `armv7r-none-eabihf` | `scripts/check-arm-link.sh`: the staticlib for each target, linked with the backend into a freestanding image (stub start-up, linker scripts `cortex-m7.ld`, `cortex-r5.ld`, addresses of no board), symbol check with `nm` |

So the staticlib **is** already built for `armv7r-none-eabihf` in CI, in the `arm-backend` job, with the same flags a firmware needs [read from source: `docs/ci.md`; `scripts/check-arm-link.sh`, target table]. The gap is in two places:

- `composition-core` alone and the workspace checks are built only for `thumbv7em-none-eabihf` (`checks` job) [read from source]. The core has no CI build of its own for the R5F target; it is built for it only as a dependency of the staticlib [inferred]. A separate item of the plan (`am62p-armv7r-core-build`) covers a host check of that; it is not part of this plan.
- `docs/ci.md` says the `arm-backend` job was "not yet seen green on a runner" at the time `open-issues.md` was written [read from source: `docs/open-issues.md`, M3-05]. Step 0 of §7 is to read the current run status.

Differences between the two targets that matter for the link [inferred; target facts are general knowledge of the Rust toolchain, not stated in this repository]:

| | `thumbv7em-none-eabihf` | `armv7r-none-eabihf` |
| --- | --- | --- |
| Core | Cortex-M4/M7 class, Thumb only | Cortex-R4/R5/R7/R8 class, Arm and Thumb |
| FPU ABI | hard float | hard float |
| Tier | Rust tier 2, prebuilt `core` | Rust tier 2, prebuilt `core` (to be confirmed for the installed toolchain) |
| Code generated | Thumb-2 | Arm state by default; the C side is built `-mcpu=cortex-r5 -mfloat-abi=hard` (`check-arm-link.sh`) |

The target's default CPU is generic Armv7-R. The Rust library and the C objects must agree on the float ABI (both hard); the board test sets `CONFIG_FPU=y` and `CONFIG_FP_HARDABI=y` for exactly this reason [read from source: `tests/board/am62p_r5f/prj.conf`, `CMakeLists.txt` comment]. A mismatch shows as a linker error about VFP attributes or as a wrong float argument at run time; the first is checkable, the second only on the board [inferred].

Whether the AM62P MCU R5F has the FPU the hard-float ABI assumes (a VFPv3-D16 on Cortex-R5F) is a hardware fact not stated here. The board's Zephyr configuration (`CONFIG_FPU`) is the owner's choice recorded in `prj.conf` [read from source]; the matching claim about the SoC is **not verified in this repository**.

## 4. Panic handler, allocator, size and stack constraints visible in the code

### 4.1 Panic handler

- With `panic-handler` and without `std`, the staticlib defines `#[panic_handler] fn panic(...) -> ! { loop {} }`: the firmware hangs in place on a panic [read from source: `crates/composition-ffi-staticlib/src/lib.rs`]. The Cargo comment says the feature is "for bare-metal C programs that link this library and contain no other Rust code" [read from source].
- Linking the same library into a program that has its own Rust code would give a second handler and a second copy of `core`; the crate comment says to depend on `composition-ffi` instead [read from source]. For the showcase: if it links another Rust library, M3-05 must use that route and the CI job does not cover it [inferred].
- A hang on a panic is not visible to a watchdog unless one is running. The board test sets `CONFIG_WATCHDOG=n` [read from source: `prj.conf`]. For a real firmware, the choice of what a panic does (halt, reset, or report through the console) is open. [inferred]
- The release profile has no `[profile]` section: the defaults apply [read from source: root `Cargo.toml` has none]. For `armv7r-none-eabihf` the target specification uses abort on panic [inferred: general toolchain fact], which is what a `no_std` staticlib needs. If the check shows `rust_eh_personality` or unwinding symbols undefined, the profile needs `panic = "abort"`; `check-arm-link.sh` would already fail on an undefined symbol today [read from source: it fails on any undefined symbol], so the case is covered in CI for the freestanding image.

### 4.2 Allocator

- `composition-core` is "`no_std`, allocation-free and contains no `unsafe`" [read from source: `crates/composition-core/src/lib.rs` line 3]. The FFI crate uses `core` only (`use core::...`) and takes the engine storage from the caller [read from source: `crates/composition-ffi/src/lib.rs`, header of `ce_engine_init`: "caller-provided storage of at least CE_ENGINE_SIZE bytes aligned to CE_ENGINE_ALIGN"].
- So the firmware needs **no global allocator and no `alloc` crate**, and the firmware's heap (`CONFIG_HEAP_MEM_POOL_SIZE`) is not used by the engine [inferred from the above; a `nm` check for `__rust_alloc` and `__rdl_alloc` in the final image proves it, §7 check C4].
- The backend (C) includes no `<stdlib.h>` and calls no `malloc` [read from source: `ce_twoplane.c` includes `<stddef.h>` only; a search for `malloc` finds nothing].

### 4.3 Size

- `CE_ENGINE_SIZE` is 28672 bytes, aligned to `CE_ENGINE_ALIGN` = 8, in the header and in the Rust constant; a compile-time assertion keeps `size_of::<Engine<CBackend>>()` within it [read from source: `composition_engine.h`; `crates/composition-ffi/src/lib.rs` lines 89 and 118]. The firmware reserves that storage statically (`scenes.c` does: `static _Alignas(CE_ENGINE_ALIGN) unsigned char storage[CE_ENGINE_SIZE]`) [read from source].
- Fixed limits that size the data: `MAX_OUTPUTS` 4, `MAX_PLANES` 16, `MAX_LAYERS` 16, `MAX_FORMATS` 32, `MAX_BUFFERS` 64, `MAX_RESOURCE_GROUPS` 8 [read from source: `crates/composition-core/src/limits.rs`; the header has the same `CE_MAX_*`]. A `ce_capabilities` structure is larger than the engine's own tables; the structure sizes are printed by `ce_abi_check`'s inputs [inferred; the size was not computed here].
- The code size of the library (flash/TCM) has **no figure in this repository**. The CI linker scripts give 1 MiB FLASH and 256 KiB RAM at addresses of no board [read from source: `backends/twoplane/arm/cortex-r5.ld`], which are the stand-ins, not the AM62P's. The MCU R5F's real memory (TCM size, the DDR carve-out of the remoteproc image) is not stated in this repository and must come from the board's device tree and the linker script of the firmware [inferred]. M3-05 should therefore record a measured figure (`size` of the final ELF, and of the `.a` members) as a result, not as a prediction.

### 4.4 Stack

- The board test sets `CONFIG_MAIN_STACK_SIZE=16384` with the comment "ce_present and ce_poll_event run on the main thread" [read from source: `prj.conf`]. That is a setting, not a measurement: nothing in the repository measures the engine's stack use [read from source: a search for stack-use figures in `docs/` finds only that setting].
- The planner works on fixed arrays (`MAX_CANDIDATES` = 1 + 16·17/2 = 137 in `advise.rs`; `MAX_EVENTS` is 4·33 + 16 = 148) [read from source: `advise.rs` line 33, `limits.rs` line 24]. Large locals are therefore possible in `ce_present` and `ce_advise`; the stack of the calling thread has to cover them [inferred]. Whether `ce_advise` is used on the R5F at all is a firmware decision: the board test uses `ce_present` and `ce_poll_event` [read from source: `prj.conf` comment].
- The vsync interrupt calls into the backend's port (`irq_lock` protects the shared record [read from source: `ce_twoplane.h`, comment on the shared record]). The interrupt stack is the firmware's; the backend's ISR path is C and short [inferred, not measured].
- Check, see §7 C5: fill the stack with a pattern (Zephyr's `CONFIG_THREAD_ANALYZER` or `CONFIG_STACK_SENTINEL` / `k_thread_stack_space_get`) and read the high-water mark after all scenes. This needs the board; a host bound (`-fstack-usage` for the C, `cargo +nightly -Zemit-stack-sizes` or `llvm-readobj --stack-sizes` for the Rust) gives an upper hint only for non-recursive paths [inferred].

## 5. How `tests/board/am62p_r5f` already links the engine

All from `tests/board/am62p_r5f/CMakeLists.txt` [read from source], on the owner's branch as merged into this tip:

1. `CE_RUST_TARGET` is `armv7r-none-eabihf`; the cargo target directory is `${CMAKE_BINARY_DIR}/cargo`; the library is `.../armv7r-none-eabihf/release/libcomposition_engine.a`.
2. A custom target `composition_engine_build` runs `cargo build --locked --release -p composition-ffi-staticlib --no-default-features --features panic-handler --target armv7r-none-eabihf --target-dir ...`, working directory the repository root, `BYPRODUCTS` the library, `USES_TERMINAL`.
3. An imported static library `composition_engine` points to that file and depends on the custom target, so cargo runs at every build and decides itself whether it is up to date.
4. The app gets the sources `src/main.c`, `src/scenes.c`, `src/hw_am62p.c` and `backends/twoplane/ce_twoplane.c` from this repository by path, and the include directories `src`, `backends/twoplane` and `crates/composition-ffi/include`.
5. `target_link_libraries(app PRIVATE composition_engine)`.
6. `prj.conf` sets `CONFIG_FPU=y`, `CONFIG_FP_HARDABI=y` (match the hard-float Rust target), `CONFIG_MAIN_STACK_SIZE=16384`, a RAM console and no UART.

Versus CI's `arm-backend`: the same cargo command and target; CI compiles the C with clang `-ffreestanding -Wall -Wextra -Werror` and its own linker script, the board test lets Zephyr's toolchain compile and link it [read from source: `docs/ci.md`; `CMakeLists.txt`]. The compile flags of the Zephyr build, and whether the Zephyr C compile of `ce_twoplane.c` is warning-free, are **not known**: the image is described as "builds" by the owner's commit message, but that build has not been repeated in this repository's history [read from source: `docs/board-test-am62p-r5f.md` status line, as of this tip]. The item `am62p-image-build` is changing that file and may have updated the status; read it before relying on this line.

What the board test shows is therefore the first of the two M3-05 pieces (a Zephyr build links the library). It is not yet the showcase's build, and not yet a run.

Open points in the board test's approach [inferred]:

- `cargo` is called from the CMake step, so the Zephyr build host needs network or a vendored cargo registry if `Cargo.lock` names registry crates. Whether the workspace has external dependencies for the staticlib was not checked here; `--locked` guarantees the lock file is not changed, not that the registry is reachable. Check step S2.
- The Rust toolchain version is whatever `cargo` the build host has; `rust-version = "1.82"` is the minimum, and CI uses the latest stable [read from source: `Cargo.toml`; `docs/ci.md`]. A firmware build should pin a version, and the pin would be the showcase's decision.
- `__aeabi_memclr4`: with the Rust library, `compiler_builtins` inside it defines the symbol (weak). A firmware that links its own runtime (Zephyr's picolibc/newlib with libgcc) may find two definitions; weak plus strong resolves silently in favour of the strong one [read from source: `docs/ci.md`, "Where `__aeabi_memclr4` comes from"; the resolution rule is inferred]. Check C3 records which one wins.

## 6. What could go wrong that CI does not catch

[inferred; each is a risk to look for, not an observation]

1. **Mixed float ABI or FPU enablement.** The image links, then faults at the first float instruction if the core's FPU is not enabled at start-up. Zephyr's Arm Cortex-R start-up normally does this when `CONFIG_FPU=y`; the stub `start.c` of CI does not, because nothing runs [read from source: the CI image "has no clocks, interrupts or real driver"].
2. **Duplicated runtime symbols** (`memcpy`, `memset`, `__aeabi_*`) between `compiler_builtins` and the firmware's libc: the link passes, behaviour differs in speed or alignment handling.
3. **Linker garbage collection and the library.** CI uses `-Wl,--gc-sections`. Zephyr does too by default; the engine's exports (`ce_*`) must be referenced from the app, which they are by the C calls [inferred].
4. **Linking a second Rust library**, as in §4.1.
5. **Unaligned storage or an engine size mismatch** if the header and the library come from different commits. `ce_abi_check(CE_ABI_CHECK())` detects it at run time; a firmware must call it at start-up [read from source: header; a search of `tests/board/am62p_r5f/src` and `ce_twoplane.c` finds no call to it, so the board test does not check it today (read from source)].
6. **Link-time vs run-time pin drift**: the showcase pins a commit of this repository; the header, library and `ce_twoplane.c` must come from that one commit. A west update that moves only one of them is not detected unless C6 runs [inferred].

## 7. Steps and checks that show M3-05 done

Order matters: a check that needs the board comes after the board run of the M3 plan.

### Steps

0. Read the last `arm-backend` run on `main` (GitHub Actions) and record its result. If it is red or never ran, fix that first; that is a separate item.
1. In the showcase (not here), record the decisions of §2 (point 1: a module description or a path), §3 (the toolchain pin) and §4.1 (panic behaviour). The owner of the showcase decides; this repository records the decision in the M3 report.
2. Build the firmware with the showcase's build, with the library built by cargo from the pinned commit, per §5. Keep the build log.
3. Run `nm` over the final ELF (§ checks below).
4. Run the image on the board in the M3 session (plan #465 item `am62p-board-run`); the image either is the board test's or the showcase's firmware.
5. Record all of this in `docs/m3-am62p-report.md` (a section "M3-05") and only then edit the M3-05 row of `docs/open-issues.md` (a separate, last item of the plan).

### Checks

Host checks (no board):

| ID | Check | Pass |
| --- | --- | --- |
| S1 | `CC_KIND=clang scripts/check-arm-link.sh` (and `CC_KIND=gcc` if Arm GCC is present) is green for both cores, and the `arm-backend` job is green on `main` | exit 0; CI run green |
| S2 | `cargo build --locked --release -p composition-ffi-staticlib --no-default-features --features panic-handler --target armv7r-none-eabihf` works on the firmware build host | library exists; no network error |
| C1 | The firmware's build log contains the cargo step of §5 with these exact flags | the log line |
| C2 | The final ELF has no undefined symbol and the engine's `ce_*` entry points are defined | `nm` has no `U` lines (except those the loader resolves, named in the report) |
| C3 | Which input defines `__aeabi_memclr4` (and `memset`, `memcpy`) in the final map | recorded; no duplicate-symbol error |
| C4 | No allocator symbols from the engine: `nm` of the final ELF shows no `__rust_alloc`, `__rdl_alloc`, `__rust_alloc_error_handler` | none present |

Board checks (after the run):

| ID | Check | Pass |
| --- | --- | --- |
| C5 | Stack high-water mark of the main thread (and the interrupt) after all scenes, measured | below 75 % of the configured stack (the margin is a proposal; the owner may set another); the value is printed in the report |
| C6 | `ce_abi_check(CE_ABI_CHECK())` returns true at start-up on the board, and the log says so | log line `ABI` or equivalent; the board test has no such call (read from source), so the call must be added first (a separate change, not part of this plan) |
| C7 | The run of the M3 plan's scenes gives the same SCENE/RESULT lines as the host run against the fake panel (`cargo test --workspace` runs the scene sequence) | the same status codes and counts |
| C8 | Size: `size` of the final ELF and of the library members, recorded | the numbers are in the report; no limit is set here (none is stated in the repository) |

M3-05 is **done** when S1, S2, C1 to C4 and C6 and C7 are recorded as passed in the M3 report for one named commit of this repository, in one real firmware build (the board test's counts as "a Zephyr build", the showcase's as "the real one"), and C5 and C8 have recorded values. If only the board test's image was built, say so in the report: the plan then closes "linked in a Zephyr build", and the "showcase" half stays open, with the reason.

M3-05 is **not** done by: a green `arm-backend` job alone (it is the starting point), a link without a run, or a run on the host.

## 8. Not decided here

- Whether this repository should get a `zephyr/module.yml` and a `CMakeLists.txt` for the library (a code change, outside this plan).
- The panic behaviour of a production firmware.
- Rust toolchain pin and offline vendoring.
- The stack threshold of C5.
