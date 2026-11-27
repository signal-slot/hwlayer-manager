#!/usr/bin/env bash
# Cross-build for aarch64 Linux, print the executable paths on stdout, and
# finish by running `file` on them (on stderr). Exit status is non-zero unless
# every executable is an ARM aarch64 ELF.
#
#   scripts/cross-aarch64.sh                       # probe + drm-harness
#   scripts/cross-aarch64.sh -p composition-drm --example probe
#   scripts/cross-aarch64.sh -p <pkg> --bin <name>  # any cargo build args
#
# With no arguments it builds the composition-drm probe example and the
# drm-harness binary (package composition-drm-harness), the two tools of the
# i.MX95 M1 runs. Arguments are passed to `cargo build --release`.
#
# Target: aarch64-unknown-linux-musl, fully static, linked by rust-lld
# (self-contained, no aarch64 gcc needed). Fallback: aarch64-unknown-linux-gnu
# with a system cross linker (aarch64-linux-gnu-gcc and friends) if one exists.
# Force a choice with HWL_AARCH64_TARGET=musl|gnu.
#
# Where the musl toolchain comes from, in this order (the script prints
# "toolchain: ..." with the path it took):
#   1. active   the active toolchain already has the musl target
#   2. rustup   `rustup target add aarch64-unknown-linux-musl` succeeded
#   3. private  a private rustup under the cache directory (below), installed
#               with the official rustup-init (--profile minimal, no PATH
#               change) and the musl target. Used when 1 and 2 fail, e.g. a
#               system Rust or a rustup with a file conflict, or when
#               HWLAYER_XBUILD_FORCE_PRIVATE=1. Needs static.rust-lang.org and
#               curl or wget on the first run only; later runs reuse the cache.
# Path 3 never changes the user's default toolchain (own RUSTUP_HOME and
# CARGO_HOME, own downloaded rustup-init).
#
# Cache: ${HWLAYER_XBUILD_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/hwlayer-manager/xbuild}
# (a few hundred MB). Clear it by removing that directory. It must not be
# inside the repository or under /tmp or /var/tmp; the script refuses those.
# Private toolchain version: HWLAYER_XBUILD_TOOLCHAIN, default the workspace
# rust-version of Cargo.toml, else stable.
#
# Linker: with the musl target, the rust-lld of the toolchain in use
# (lib/rustlib/<host>/bin), set through CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_*
# environment variables for this cargo invocation only; there is deliberately
# no committed .cargo/config.toml (it would affect host builds). Output goes to
# <workspace>/target/. Progress and cargo diagnostics go to stderr; stdout
# carries only the executable paths.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
export CARGO_TARGET_DIR="$root/target"

want=${HWL_AARCH64_TARGET:-auto}
musl=aarch64-unknown-linux-musl
gnu=aarch64-unknown-linux-gnu
linker=""
how=""

have_target() { # <target>: std for the target exists in the active sysroot
    local dir
    dir=$(rustc --print target-libdir --target "$1" 2>/dev/null) || return 1
    compgen -G "$dir/libcore-*.rlib" >/dev/null
}

ensure_target() { # <target>: sets $how
    if have_target "$1"; then how=active; return 0; fi
    if command -v rustup >/dev/null 2>&1; then
        echo "adding rustup target $1" >&2
        if rustup target add "$1" >&2 && have_target "$1"; then how=rustup; return 0; fi
    fi
    return 1
}

find_gnu_linker() {
    local c
    for c in aarch64-linux-gnu-gcc aarch64-unknown-linux-gnu-gcc aarch64-none-linux-gnu-gcc; do
        if command -v "$c" >/dev/null 2>&1; then echo "$c"; return 0; fi
    done
    return 1
}

private_toolchain() {
    local cache=${HWLAYER_XBUILD_CACHE:-${XDG_CACHE_HOME:-${HOME:?HOME not set}/.cache}/hwlayer-manager/xbuild}
    case "$cache" in /*) ;; *) echo "error: cache directory must be absolute: $cache" >&2; return 1 ;; esac
    case "$cache/" in
        "$root"/* | /tmp/* | /var/tmp/*)
            echo "error: cache directory must not be in the repository, /tmp or /var/tmp: $cache" >&2
            return 1 ;;
    esac
    local ver=${HWLAYER_XBUILD_TOOLCHAIN:-}
    if [ -z "$ver" ]; then
        ver=$(sed -n 's/^rust-version *= *"\([^"]*\)".*/\1/p' Cargo.toml | head -n1)
        ver=${ver:-stable}
    fi
    export RUSTUP_HOME="$cache/rustup" CARGO_HOME="$cache/cargo"
    export PATH="$CARGO_HOME/bin:$PATH"
    unset RUSTUP_TOOLCHAIN RUSTC CARGO
    mkdir -p "$cache"
    if [ ! -x "$CARGO_HOME/bin/rustup" ]; then
        local host url init
        case "$(uname -m)" in
            x86_64) host=x86_64-unknown-linux-gnu ;;
            aarch64 | arm64) host=aarch64-unknown-linux-gnu ;;
            *) echo "error: no rustup-init known for host $(uname -m)" >&2; return 1 ;;
        esac
        url=https://static.rust-lang.org/rustup/dist/$host/rustup-init
        init=$cache/rustup-init
        echo "downloading $url" >&2
        if command -v curl >/dev/null 2>&1; then
            curl --proto '=https' --tlsv1.2 -fsSL -o "$init.part" "$url" >&2
        else
            wget -q -O "$init.part" "$url" >&2
        fi
        mv "$init.part" "$init"
        chmod +x "$init"
        "$init" --no-modify-path -y --profile minimal --default-toolchain "$ver" >&2
    fi
    if ! have_target "$musl"; then
        echo "adding target $musl to the private toolchain ($ver)" >&2
        rustup target add --toolchain "$ver" "$musl" >&2
    fi
    have_target "$musl" || { echo "error: private toolchain lacks $musl" >&2; return 1; }
    how="private ($cache)"
}

target=""
if [ "$want" = auto ] || [ "$want" = musl ]; then
    if [ "${HWLAYER_XBUILD_FORCE_PRIVATE:-0}" = 1 ]; then
        private_toolchain && target=$musl
    elif ensure_target "$musl" || private_toolchain; then
        target=$musl
    fi
fi
if [ -z "$target" ] && { [ "$want" = auto ] || [ "$want" = gnu ]; }; then
    if linker=$(find_gnu_linker) && ensure_target "$gnu"; then
        target=$gnu
    fi
fi
if [ -z "$target" ]; then
    echo "error: no $musl toolchain (active, rustup or private) and no system aarch64 gcc" >&2
    exit 1
fi
echo "toolchain: $how, target: $target, $(rustc --version)" >&2

if [ "$target" = "$musl" ]; then
    # Static, self-contained musl (bundled crt and libc) linked by the rust-lld
    # of the toolchain in use.
    lld=$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin/rust-lld
    if [ ! -x "$lld" ]; then lld=rust-lld; fi
    echo "linker: $lld" >&2
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=$lld
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_RUSTFLAGS="-C linker-flavor=ld.lld -C link-self-contained=yes -C target-feature=+crt-static -C relocation-model=static"
else
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER="$linker"
    echo "linker: $linker" >&2
fi
# Caller RUSTFLAGS would be ignored next to the target-specific ones; drop them.
unset RUSTFLAGS CARGO_ENCODED_RUSTFLAGS

outs=()
# Run cargo in the current shell so a failure stops the script, then extract paths.
mkdir -p "$root/target"
json=$(mktemp "$root/target/.xbuild.XXXXXX")
trap 'rm -f "$json"' EXIT
run() {
    echo "building for $target: $*" >&2
    cargo build --release --target "$target" --message-format=json-render-diagnostics "$@" >"$json"
    local p
    while IFS= read -r p; do outs+=("$p"); done < <(sed -n 's/.*"executable":"\([^"]*\)".*/\1/p' "$json")
}
defaults=0
if [ $# -eq 0 ]; then
    defaults=1
    run -p composition-drm --example probe
    run -p composition-drm-harness --bin drm-harness
else
    run "$@"
fi

[ "${#outs[@]}" -eq 0 ] || printf '%s\n' "${outs[@]}"
echo "file:" >&2
bad=0
# With no arguments both tools must come out; with arguments a library build
# (no executable) is fine.
[ "${#outs[@]}" -gt 0 ] || [ "$defaults" -eq 0 ] || bad=1
for p in "${outs[@]}"; do
    d=$(file -b "$p")
    echo "$p: $d" >&2
    case "$d" in *"ARM aarch64"*) ;; *) bad=1 ;; esac
done
if [ "$bad" -ne 0 ]; then
    echo "error: not every output is an ARM aarch64 executable" >&2
    exit 1
fi
