#!/usr/bin/env bash
# Cross-build AGL drm-lease-manager (and libdlmclient) for aarch64 Linux,
# statically if possible, so the binaries can run from /tmp on a board
# without installing anything into the image.  See docs/dlm-aarch64.md.
#
# Usage: scripts/build-dlm-aarch64.sh [DEPS_DIR]
#   DEPS_DIR  where sources are fetched (default: $CONNECTED_DEPS_DIR).
#             Never /tmp or /var/tmp.
# Environment:
#   BUILD_DIR       build tree (default: $TMPDIR/dlm-aarch64-build, else
#                   <repo>/target/dlm-aarch64-build).  Never /tmp or /var/tmp.
#   DLM_CC          force a compiler command line, e.g. "zig cc -target
#                   aarch64-linux-musl"; ar/strip from DLM_AR/DLM_STRIP.
#   DLM_SYSROOT     aarch64 Linux sysroot (headers, crt*.o, libc.a) for the
#                   clang route.
#   DLM_COMMIT, LIBDRM_COMMIT, TOML_COMMIT
#                   override a pin; each must be a full 40-hex commit hash.
#   DLM_URL, LIBDRM_URL, TOML_URL
#                   override a source URL (e.g. a mirror).
# Output: $BUILD_DIR/out/{bin,lib}; `file` of the executables is printed.
#
# Every source is pinned by commit hash only, never by tag or branch name:
# the hash is fetched directly (or, if the server refuses that, after a full
# fetch of branches and tags), checked out detached and compared with HEAD.
set -euo pipefail

DLM_URL=${DLM_URL:-https://gerrit.automotivelinux.org/gerrit/src/drm-lease-manager}
DLM_COMMIT=${DLM_COMMIT:-68c48758c46d93c8e83abb2fb1ec168fa13d1a53}  # master, 0.1.0
LIBDRM_URL=${LIBDRM_URL:-https://gitlab.freedesktop.org/mesa/libdrm.git}
LIBDRM_COMMIT=${LIBDRM_COMMIT:-25dec5b91fe4d2638787d033a0b22b6c1dc145e0}  # tag libdrm-2.4.123
TOML_URL=${TOML_URL:-https://github.com/cktan/tomlc99}
TOML_COMMIT=${TOML_COMMIT:-62ce93ceb713dc49909d0fef1411f9d37bef83f2}  # master

die() { echo "build-dlm-aarch64: $*" >&2; exit 1; }
say() { echo "build-dlm-aarch64: $*"; }
have() { command -v "$1" >/dev/null 2>&1; }

not_tmp() { # refuse /tmp and /var/tmp (memory-backed on some machines)
  case "$(realpath -m "$2")" in
    /tmp|/tmp/*|/var/tmp|/var/tmp/*) die "$1 must not be under /tmp or /var/tmp: $2" ;;
  esac
}

# ---- pins and directories (checked before anything else) -------------------
for v in DLM_COMMIT LIBDRM_COMMIT TOML_COMMIT; do
  [[ ${!v} =~ ^[0-9a-f]{40}$ ]] || die "$v must be a full 40-hex commit hash, got '${!v}'"
done
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
DEPS=${1:-${CONNECTED_DEPS_DIR:-}}
[ -n "$DEPS" ] || die "no source directory: pass one as argument or set CONNECTED_DEPS_DIR"
BUILD=${BUILD_DIR:-${TMPDIR:+$TMPDIR/dlm-aarch64-build}}
BUILD=${BUILD:-$ROOT/target/dlm-aarch64-build}
not_tmp "source directory" "$DEPS"
not_tmp "build directory" "$BUILD"

# Download Debian stable packages (SHA256 checked against the Packages index
# fetched over https) and unpack them: arm64 libc/gcc runtime into $1/sysroot,
# x86-64 aarch64-binutils into $1/host.  Idempotent; records versions in
# $1/VERSIONS.
bootstrap_toolchain() {
  local tc=$1
  [ -x "$tc/host/usr/bin/aarch64-linux-gnu-ld" ] &&
    [ -e "$tc/host/usr/lib/x86_64-linux-gnu/libjansson.so.4" ] &&
    [ -f "$tc/sysroot/usr/lib/aarch64-linux-gnu/libc.a" ] && return 0
  say "fetching Debian arm64 sysroot and aarch64 binutils into $tc"
  mkdir -p "$tc"
  python3 -I - "$tc" <<'PY'
import sys, os, io, re, lzma, hashlib, subprocess, tarfile, urllib.request
tc = sys.argv[1]
base = "https://deb.debian.org/debian/"
sets = [
    ("arm64", "sysroot", ["libc6-dev", "libc6", "linux-libc-dev", "libgcc-14-dev",
                          "libgcc-s1", "libcrypt-dev", "libcrypt1"]),
    ("amd64", "host", ["binutils-aarch64-linux-gnu", "binutils-common", "libjansson4",
                       "libsframe1"]),
]
versions = []
for arch, dest, pkgs in sets:
    idx = lzma.decompress(urllib.request.urlopen(base + f"dists/stable/main/binary-{arch}/Packages.xz").read()).decode()
    ents = {}
    for blk in idx.split("\n\n"):
        f = {k: re.search(rf"^{k}: (.+)$", blk, re.M) for k in ("Package", "Filename", "SHA256", "Version")}
        if all(f.values()):
            ents[f["Package"].group(1)] = tuple(f[k].group(1) for k in ("Filename", "SHA256", "Version"))
    out = os.path.join(tc, dest)
    os.makedirs(out, exist_ok=True)
    for p in pkgs:
        fn, sha, ver = ents[p]
        data = urllib.request.urlopen(base + fn).read()
        if hashlib.sha256(data).hexdigest() != sha:
            sys.exit(f"sha256 mismatch for {fn}")
        tmp = os.path.join(tc, "pkg.deb")
        open(tmp, "wb").write(data)
        names = subprocess.run(["ar", "t", tmp], capture_output=True, text=True, check=True).stdout.split()
        m = next(n for n in names if n.startswith("data.tar"))
        raw = subprocess.run(["ar", "p", tmp, m], capture_output=True, check=True).stdout
        if m.endswith(".zst"):
            raw = subprocess.run(["zstd", "-dc"], input=raw, capture_output=True, check=True).stdout
        with tarfile.open(fileobj=io.BytesIO(raw)) as t:
            t.extractall(out, filter="tar")
        os.remove(tmp)
        versions.append(f"{arch} {p} {ver}")
        print("  ", arch, p, ver)
open(os.path.join(tc, "VERSIONS"), "w").write("\n".join(versions) + "\n")
PY
}

# ---- cross toolchain detection (before any network access) -----------------
if [ -n "${DLM_CC:-}" ]; then
  ROUTE="DLM_CC ($DLM_CC)"; CC_CMD=$DLM_CC
  AR=${DLM_AR:-ar}; STRIP=${DLM_STRIP:-strip}
elif have aarch64-linux-gnu-gcc; then
  ROUTE="aarch64-linux-gnu-gcc (glibc, -static)"; CC_CMD=aarch64-linux-gnu-gcc
  AR=aarch64-linux-gnu-ar; STRIP=aarch64-linux-gnu-strip
elif have aarch64-linux-musl-gcc; then
  ROUTE="aarch64-linux-musl-gcc (musl, -static)"; CC_CMD=aarch64-linux-musl-gcc
  AR=aarch64-linux-musl-ar; STRIP=aarch64-linux-musl-strip
elif have zig; then
  ROUTE="zig cc -target aarch64-linux-musl (musl, -static)"
  CC_CMD="zig cc -target aarch64-linux-musl"
  AR="zig ar"; STRIP=true
elif have clang && [ -n "${DLM_SYSROOT:-}" ] && [ -d "$DLM_SYSROOT/usr/include" ]; then
  have ld.lld || die "clang route needs ld.lld to link for aarch64"
  ROUTE="clang --target=aarch64-linux-gnu --sysroot=$DLM_SYSROOT"
  CC_CMD="clang --target=aarch64-linux-gnu --sysroot=$DLM_SYSROOT -fuse-ld=lld"
  AR=${DLM_AR:-llvm-ar}; STRIP=${DLM_STRIP:-llvm-strip}
elif have clang && have python3 && have ar && have zstd; then
  # No cross compiler anywhere: clang (codegen) + Debian arm64 glibc sysroot +
  # Debian's x86-64 build of aarch64 GNU binutils (the linker), all unpacked
  # under $DEPS/aarch64-toolchain, nothing installed.
  TC=$DEPS/aarch64-toolchain
  bootstrap_toolchain "$TC"
  ROUTE="clang --target=aarch64-linux-gnu, Debian arm64 sysroot (glibc, -static), GNU ld (aarch64)"
  CC_CMD="clang --target=aarch64-linux-gnu --sysroot=$TC/sysroot -B$TC/host/usr/bin"
  # --ld-path only at link time: meson's compile-only probes use -Werror=unused-command-line-argument
  LINK_ARGS="'--ld-path=$TC/host/usr/bin/aarch64-linux-gnu-ld', "
  AR=$TC/host/usr/bin/aarch64-linux-gnu-ar; STRIP=$TC/host/usr/bin/aarch64-linux-gnu-strip
  export LD_LIBRARY_PATH=$TC/host/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
else
  cat >&2 <<EOF
build-dlm-aarch64: no usable aarch64 Linux cross toolchain on this machine.
  Looked for, in order: \$DLM_CC, aarch64-linux-gnu-gcc, aarch64-linux-musl-gcc,
  zig, and clang with \$DLM_SYSROOT (an aarch64 sysroot with usr/include).
  clang alone is not enough: it emits aarch64 code but has no aarch64 libc
  (headers, crt1.o, libc.a) to compile and link against, and the rustup
  aarch64 targets do not ship C headers.
  Fix one of:
    - install an aarch64 cross gcc (glibc or musl) on PATH,
    - put zig on PATH (it bundles musl for aarch64),
    - point DLM_SYSROOT at an aarch64 sysroot (e.g. from the Toradex SDK), or
    - set DLM_CC to a full compiler command.
  Nothing was fetched or built.  See docs/dlm-aarch64.md.
EOF
  exit 2
fi
for t in git meson ninja pkg-config file; do have "$t" || die "missing host tool: $t"; done
say "cross toolchain: $ROUTE"

# ---- fetch pinned sources ---------------------------------------------------
fetch() { # name url commit
  local name=$1 url=$2 want=$3 dir=$DEPS/$1 head
  if [ ! -d "$dir/.git" ]; then
    [ ! -e "$dir" ] || die "$dir exists and is not a git checkout"
    git init -q "$dir"
    git -C "$dir" remote add origin "$url"
  fi
  git -C "$dir" remote set-url origin "$url"
  if ! git -C "$dir" cat-file -e "$want^{commit}" 2>/dev/null; then
    say "$name: fetching $want from $url"
    if ! git -C "$dir" fetch -q --depth 1 origin "$want"; then
      say "$name: fetch by hash refused; fetching all branches and tags"
      local deepen=()
      [ "$(git -C "$dir" rev-parse --is-shallow-repository)" = false ] || deepen=(--unshallow)
      git -C "$dir" fetch -q "${deepen[@]}" origin \
        '+refs/heads/*:refs/remotes/origin/*' '+refs/tags/*:refs/tags/*'
    fi
    git -C "$dir" cat-file -e "$want^{commit}" 2>/dev/null ||
      die "$name: commit $want is not available from $url"
  fi
  [ -z "$(git -C "$dir" status --porcelain --untracked-files=no)" ] ||
    die "$name: $dir has local changes; clean or remove it"
  git -C "$dir" checkout -q --detach "$want"
  head=$(git -C "$dir" rev-parse HEAD)
  [ "$head" = "$want" ] || die "$name is at $head, not the pinned commit $want"
  say "$name at $head"
}
mkdir -p "$DEPS" "$BUILD"
fetch drm-lease-manager "$DLM_URL" "$DLM_COMMIT"
fetch libdrm "$LIBDRM_URL" "$LIBDRM_COMMIT"
fetch tomlc99 "$TOML_URL" "$TOML_COMMIT"

# ---- meson cross file -------------------------------------------------------
OUT=$BUILD/out
mkdir -p "$OUT/bin" "$OUT/include" "$OUT/lib/pkgconfig"
to_arr() { local s="" w; for w in $1; do s="$s${s:+, }'$w'"; done; echo "[$s]"; }
CROSS=$BUILD/aarch64.cross
cat >"$CROSS" <<EOF
[binaries]
c = $(to_arr "$CC_CMD")
ar = $(to_arr "$AR")
strip = $(to_arr "$STRIP")
pkg-config = 'pkg-config'

[properties]
needs_exe_wrapper = true
pkg_config_libdir = ['$OUT/lib/pkgconfig']

[built-in options]
c_link_args = [${LINK_ARGS:-}'-static']

[host_machine]
system = 'linux'
cpu_family = 'aarch64'
cpu = 'aarch64'
endian = 'little'
EOF
# Keep the host's .pc files out of the cross build; only our prefix is searched.
export PKG_CONFIG_LIBDIR=$OUT/lib/pkgconfig PKG_CONFIG_PATH=

# ---- libdrm (static, no GPU drivers, no tests) ------------------------------
say "building libdrm"
rm -rf "$BUILD/libdrm"
meson setup "$BUILD/libdrm" "$DEPS/libdrm" --cross-file "$CROSS" \
  --prefix "$OUT" --libdir lib --default-library static --buildtype release \
  -Dintel=disabled -Dradeon=disabled -Damdgpu=disabled -Dnouveau=disabled \
  -Dvmwgfx=disabled -Dfreedreno=disabled -Dvc4=disabled -Detnaviv=disabled \
  -Dexynos=disabled -Domap=disabled -Dtegra=disabled \
  -Dcairo-tests=disabled -Dvalgrind=disabled -Dman-pages=disabled \
  -Dtests=false -Dinstall-test-programs=false
ninja -C "$BUILD/libdrm" install

# ---- tomlc99 (libtoml): required by drm-lease-manager, no wrap in its tree --
# meson.build has dependency('libtoml'), so provide libtoml.a and libtoml.pc.
say "building tomlc99"
mkdir -p "$BUILD/toml"
# shellcheck disable=SC2086 # CC_CMD/AR may be multi-word commands
$CC_CMD -std=c99 -O2 -DNDEBUG -c "$DEPS/tomlc99/toml.c" -o "$BUILD/toml/toml.o"
rm -f "$OUT/lib/libtoml.a"
# shellcheck disable=SC2086
$AR rcs "$OUT/lib/libtoml.a" "$BUILD/toml/toml.o"
cp "$DEPS/tomlc99/toml.h" "$OUT/include/toml.h"
cat >"$OUT/lib/pkgconfig/libtoml.pc" <<PC
prefix=$OUT
libdir=\${prefix}/lib
includedir=\${prefix}/include
Name: libtoml
Description: tomlc99 at $TOML_COMMIT
Version: 1.0
Libs: -L\${libdir} -ltoml
Cflags: -I\${includedir}
PC

# ---- drm-lease-manager ------------------------------------------------------
# meson_options.txt at the pinned commit: enable-docs (Doxygen), enable-tests
# (check + fff), enable-systemd; all off, so neither Doxygen nor check is needed.
say "building drm-lease-manager"
rm -rf "$BUILD/dlm"
meson setup "$BUILD/dlm" "$DEPS/drm-lease-manager" --cross-file "$CROSS" \
  --prefix "$OUT" --libdir lib --default-library static --buildtype release \
  --localstatedir var -Denable-docs=false -Denable-tests=false -Denable-systemd=false
# Upstream's install script does `install -d $DESTDIR/var/run/drm-lease-manager`
# with an absolute path, so install into a staging DESTDIR, then copy.
rm -rf "$BUILD/stage"
DESTDIR=$BUILD/stage ninja -C "$BUILD/dlm" install
cp -a "$BUILD/stage$OUT/." "$OUT/"
# examples are not installed; the test client is handy on the board
if [ -f "$BUILD/dlm/examples/dlm-client-test/dlm-client-test" ]; then
  cp "$BUILD/dlm/examples/dlm-client-test/dlm-client-test" "$OUT/bin/"
fi

# ---- result -----------------------------------------------------------------
EXE=$OUT/bin/drm-lease-manager
[ -f "$EXE" ] || die "build finished but $EXE is missing"
for b in "$OUT"/bin/*; do $STRIP "$b" || true; done
file "$OUT"/bin/*
ls "$OUT"/lib/libdlmclient* "$OUT"/include/libdlmclient/* 2>/dev/null || true
file "$EXE" | grep -q 'ARM aarch64' || die "$EXE is not an aarch64 executable"
file "$EXE" | grep -q 'statically linked' ||
  say "warning: $EXE is not statically linked; check its loader exists on the board"
say "pins: drm-lease-manager $DLM_COMMIT, libdrm $LIBDRM_COMMIT, tomlc99 $TOML_COMMIT"
say "copy $EXE to the board's /tmp; see docs/dlm-aarch64.md"
