# cross-aarch64.sh

Cross-builds for aarch64 Linux (`aarch64-unknown-linux-musl`, static, linked by the toolchain's own `rust-lld`; no aarch64 gcc needed) and ends by running `file` on every executable it built. It exits non-zero unless all of them are `ARM aarch64`.

```sh
scripts/cross-aarch64.sh                                    # probe + drm-harness
scripts/cross-aarch64.sh -p composition-drm --example probe # any cargo build args
```

Executable paths go to stdout; progress, the `file` output and cargo diagnostics go to stderr.

## Where the toolchain comes from

The script prints `toolchain: <path>` for the route it took:

1. `active`: the active toolchain already has the musl target.
2. `rustup`: `rustup target add aarch64-unknown-linux-musl` succeeded.
3. `private`: otherwise (a system Rust, or a rustup with a file conflict on the target) a private rustup is bootstrapped with the official `rustup-init` (`--no-modify-path -y --profile minimal`, toolchain = the workspace `rust-version`, or `stable`; override with `HWLAYER_XBUILD_TOOLCHAIN`) plus the musl target. It has its own `RUSTUP_HOME` and `CARGO_HOME`, so the user's default toolchain and `PATH` are untouched. The first run needs `static.rust-lang.org` and `curl` or `wget`; later runs reuse the cache.

`HWLAYER_XBUILD_FORCE_PRIVATE=1` skips routes 1 and 2 (to test the fallback).

## Cache

`${HWLAYER_XBUILD_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/hwlayer-manager/xbuild}`, a few hundred MB, holding `rustup/`, `cargo/` and the downloaded `rustup-init`. To clear it, remove that directory. The script refuses a cache inside the repository or under `/tmp` or `/var/tmp`.

## Linker

With the musl target the linker is the `rust-lld` found under `lib/rustlib/<host>/bin` of the toolchain in use, set through `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER` and `..._RUSTFLAGS` for this cargo run only. No `.cargo/config.toml` is committed, so host builds are unaffected. `HWL_AARCH64_TARGET=gnu` selects `aarch64-unknown-linux-gnu` with a system cross gcc instead.
