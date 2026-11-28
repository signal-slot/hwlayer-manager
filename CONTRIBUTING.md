# Contributing

## Licence

The project is under the Apache License, Version 2.0 (see `LICENSE`; the
workspace `Cargo.toml` says `license = "Apache-2.0"`). A contribution is
accepted under those terms.

## Changes land through pull requests

Every change goes to `main` through a pull request. The pull request must pass
the CI described in `docs/ci.md` and defined in `.github/workflows/ci.yml`.
Run the same checks locally first:

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked -p composition-core --target thumbv7em-none-eabihf
cargo build --locked -p composition-ffi-staticlib --no-default-features \
    --features panic-handler --target thumbv7em-none-eabihf --release
```

(`rustup target add thumbv7em-none-eabihf` once.) CI also cross-builds the DRM
backend, probe and harness for aarch64 Linux with `scripts/cross-aarch64.sh`;
see `docs/ci.md` for the exact steps.

## Local checks

Run the CI commands above yourself before opening a pull request, or run them
all with `sh scripts/check-all.sh` (`--keep-going` continues after a failure,
`--cross` adds the aarch64 cross-builds). `sh scripts/check-all.sh --lint`
also runs the lint scripts that are not CI steps. The lints can be run one by
one:

- `scripts/check-ffi-header.sh [BASE_REF]`: `include/composition_engine.h`
  moved with `crates/composition-ffi/src/lib.rs`.
- `scripts/check-commit-subjects.sh [BASE_REF]`: commit subjects follow the
  style below.
- `scripts/check-no-board-secrets.sh [BASE_REF]`: no board addresses or
  credentials in files or commit messages.
- `scripts/check-doc-links.sh [--anchors]`: relative Markdown links resolve.
- `scripts/check-doc-tags.sh [FILE...]`: evidence tags, dates and source pins
  in hardware docs.

## Commit messages

The subject is either a component prefix (`engine: ...`, `ffi: ...`,
`drm: ...`, likewise `fake:`, `docs:`, `CI:`) or a plain sentence, and states
the result. The body explains the design: why this approach, what was
rejected, what it does not cover.

## Code rules

- Every crate has `#![forbid(unsafe_code)]` except `composition-ffi`, the one
  crate that needs `unsafe` for the C ABI. Check with
  `grep -rL 'forbid(unsafe_code)' crates/*/src/lib.rs backends/*/src/lib.rs`
  (only `composition-ffi` may be listed). The workspace default is
  `unsafe_code = "deny"`.
- `composition-core` is `no_std` and has no dependencies; keep its
  `Cargo.toml` free of a `[dependencies]` section.
- `crates/composition-ffi/include/composition_engine.h` is written by hand and
  must change in the same commit as `crates/composition-ffi/src/lib.rs`
  whenever the C ABI changes (`ce_abi_check` guards the layouts).

## Measurements and hardware claims

- A timing or hardware claim is a dated measurement taken on the named
  hardware, or it is marked *not measured*. Host, CI and vkms runs are never
  evidence for hardware timing or behaviour.
- Keep the tags used in `docs/platform-capabilities.md` honest: *observed*
  (seen on a device, with the date), *read from source* (with the source and
  revision), *inferred* (reasoned, not seen). Never claim parity from another
  SoC.

## Board access and secrets

Files and commits contain no board addresses, host names, IPs, serial numbers,
UART or device paths, or credentials. They come from the environment at run
time and stay out of the repository.

## Board test apps

- A board test is a Zephyr application or a small program kept in this
  repository under `tests/board/`; [tests/board/README.md](tests/board/README.md)
  indexes them.
- The Zephyr workspace and SDK come from the environment
  (`ZEPHYR_WORKSPACE`, `ZEPHYR_SDK_INSTALL_DIR`);
  [scripts/prepare-zephyr.sh](scripts/prepare-zephyr.sh) can set them up. CI
  does not build board apps.
- A board session follows [docs/imx95-m7-runbook.md](docs/imx95-m7-runbook.md)
  and the board rules in [docs/imx95-board.md](docs/imx95-board.md): files only
  under `/tmp`, no packages, no U-Boot environment changes, never write the
  eMMC boot areas, and restore Weston and check that it is active again.
- Results are dated observations, or are marked *not measured*.
- No board addresses, host names, IPs, users, serial or UART paths, or
  credentials in files or commits.

## Tests

- Board tests are Zephyr applications or small programs kept in this
  repository, not runs of external projects.

  Adding a board app:

  1. Create a directory under `tests/board/` with `CMakeLists.txt`,
     `prj.conf`, `src/` and a `README.md` that gives the `west build`
     command. `scripts/check-zephyr-apps.sh` enforces this layout.
  2. Add a row for it to `tests/board/README.md`.
  3. Build it with `scripts/build-zephyr-apps.sh`.
  4. When running it on a board, follow the board rules in
     [docs/imx95-board.md](docs/imx95-board.md) and
     [docs/imx95-m7-runbook.md](docs/imx95-m7-runbook.md).
  5. Real trace logs carry a `# date:` line; see
     [docs/trace-format.md](docs/trace-format.md).
- Changes to the DRM code should also run `scripts/vkms-vm-test.sh` where it
  can run (a throwaway QEMU VM; see the script header). It needs no root and
  touches nothing on the host. Never change DRM, udev or seat configuration on
  a developer machine.
