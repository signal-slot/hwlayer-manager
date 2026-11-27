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

## Tests

- Board tests are Zephyr applications or small programs kept in this
  repository, not runs of external projects.
- Changes to the DRM code should also run `scripts/vkms-vm-test.sh` where it
  can run (a throwaway QEMU VM; see the script header). It needs no root and
  touches nothing on the host. Never change DRM, udev or seat configuration on
  a developer machine.
