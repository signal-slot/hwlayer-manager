#!/usr/bin/env bash
# Cross-build a cargo package/example/bin for aarch64 Linux and print the
# resulting executable paths. All arguments are passed to `cargo build`.
#
#   scripts/cross-aarch64.sh -p composition-drm --example probe
#   scripts/cross-aarch64.sh -p <pkg> --bin <name>
#
# Preferred: aarch64-unknown-linux-musl, fully static, linked by rust-lld
# (self-contained, no aarch64 gcc needed). Fallback: aarch64-unknown-linux-gnu
# with a system cross linker (aarch64-linux-gnu-gcc and friends) if one exists.
# Force a choice with HWL_AARCH64_TARGET=musl|gnu.
#
# Linker settings live here, as environment for this cargo invocation only;
# there is deliberately no committed .cargo/config.toml (it would affect host
# builds). Output goes to <workspace>/target/, never /tmp. Progress and cargo
# diagnostics go to stderr; stdout carries only the executable paths.
set -euo pipefail

if [ $# -eq 0 ]; then
    echo "usage: $0 <cargo build args, e.g. -p composition-drm --example probe>" >&2
    exit 2
fi

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
export CARGO_TARGET_DIR="$root/target"

want=${HWL_AARCH64_TARGET:-auto}
musl=aarch64-unknown-linux-musl
gnu=aarch64-unknown-linux-gnu
linker=""

ensure_target() {
    if ! rustup target list --installed | grep -qx "$1"; then
        echo "adding rustup target $1" >&2
        rustup target add "$1" >&2
    fi
}

find_gnu_linker() {
    local c
    for c in aarch64-linux-gnu-gcc aarch64-unknown-linux-gnu-gcc aarch64-none-linux-gnu-gcc; do
        if command -v "$c" >/dev/null 2>&1; then echo "$c"; return 0; fi
    done
    return 1
}

target=""
if [ "$want" = auto ] || [ "$want" = musl ]; then
    if ensure_target "$musl"; then target=$musl; fi
fi
if [ -z "$target" ] && { [ "$want" = auto ] || [ "$want" = gnu ]; }; then
    if linker=$(find_gnu_linker); then
        ensure_target "$gnu"
        target=$gnu
    fi
fi
if [ -z "$target" ]; then
    echo "error: neither $musl (rustup target add failed) nor a system aarch64 gcc is available" >&2
    exit 1
fi

if [ "$target" = "$musl" ]; then
    # Static, self-contained musl (bundled crt and libc) linked by rust-lld.
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_RUSTFLAGS="-C link-self-contained=yes -C target-feature=+crt-static -C relocation-model=static"
else
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER="$linker"
fi
# Caller RUSTFLAGS would be ignored next to the target-specific ones; drop them.
unset RUSTFLAGS CARGO_ENCODED_RUSTFLAGS

echo "building for $target" >&2
cargo build --release --target "$target" --message-format=json-render-diagnostics "$@" |
    sed -n 's/.*"executable":"\([^"]*\)".*/\1/p'
