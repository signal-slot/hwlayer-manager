#!/usr/bin/env bash
# Build libliftoff at the pinned commit, the fake libdrm and the C driver of
# the libliftoff cross-check (docs/libliftoff-crosscheck.md sections 2 and 8).
#
# Usage: build-liftoff.sh [--build-dir DIR] [--allow-other-commit] [LIBLIFTOFF_SRC]
#
# The libliftoff checkout is taken from the argument, else $LIBLIFTOFF_SRC,
# else $CONNECTED_DEPS_DIR/libliftoff. If that directory does not exist it is
# fetched from upstream (git clone, or the snapshot archive of the pinned
# commit). libliftoff is never vendored into this repository.
#
# Everything is built under the build directory (default
# $TMPDIR/liftoff-crosscheck-build), which must be outside the git tree and
# outside /tmp and /var/tmp. The last line printed is the driver's path.
set -euo pipefail

readonly PINNED_COMMIT=c4226a79b7a52f59bb51789b1eda112658609125 # c4226a7
readonly LIFTOFF_GIT=https://gitlab.freedesktop.org/emersion/libliftoff.git
readonly LIFTOFF_ARCHIVE="https://gitlab.freedesktop.org/emersion/libliftoff/-/archive/${PINNED_COMMIT}/libliftoff-${PINNED_COMMIT}.tar.gz"
readonly LIBDRM_VERSION=2.4.124
readonly LIBDRM_ARCHIVE="https://dri.freedesktop.org/libdrm/libdrm-${LIBDRM_VERSION}.tar.xz"
# Written into an archive-fetched checkout, which has no .git to ask.
readonly MARKER=.liftoff-crosscheck-commit

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$here/../.." && pwd)"

die() {
	echo "build-liftoff.sh: $*" >&2
	exit 1
}
log() {
	echo "build-liftoff.sh: $*" >&2
}

allow_other=0
build_dir=""
src_arg=""
while [ $# -gt 0 ]; do
	case "$1" in
	--allow-other-commit) allow_other=1 ;;
	--build-dir)
		[ $# -ge 2 ] || die "--build-dir needs a value"
		build_dir="$2"
		shift
		;;
	--build-dir=*) build_dir="${1#--build-dir=}" ;;
	-h | --help)
		sed -n '2,14p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
		exit 0
		;;
	-*) die "unknown option $1" ;;
	*)
		[ -z "$src_arg" ] || die "only one libliftoff path may be given"
		src_arg="$1"
		;;
	esac
	shift
done

deps_dir="${CONNECTED_DEPS_DIR:-}"
if [ -n "$src_arg" ]; then
	src="$src_arg"
elif [ -n "${LIBLIFTOFF_SRC:-}" ]; then
	src="$LIBLIFTOFF_SRC"
else
	[ -n "$deps_dir" ] || die "give a libliftoff path, LIBLIFTOFF_SRC or CONNECTED_DEPS_DIR"
	src="$deps_dir/libliftoff"
fi

if [ -z "$build_dir" ]; then
	[ -n "${TMPDIR:-}" ] || die "TMPDIR is not set; give --build-dir"
	build_dir="$TMPDIR/liftoff-crosscheck-build"
fi
mkdir -p "$build_dir"
build_dir="$(cd "$build_dir" && pwd -P)"
case "$build_dir/" in
/tmp/* | /var/tmp/*) die "build dir $build_dir is under /tmp or /var/tmp; use \$TMPDIR or --build-dir" ;;
esac
case "$build_dir/" in
"$(cd "$repo_root" && pwd -P)/"*) die "build dir $build_dir is inside the git tree" ;;
esac

# --- fetch libliftoff -------------------------------------------------------

fetch_liftoff() {
	local dest="$1" parent tmp
	parent="$(dirname "$dest")"
	mkdir -p "$parent"
	tmp="$(mktemp -d "$parent/.libliftoff-fetch.XXXXXX")"
	log "fetching libliftoff into $dest"
	if GIT_TERMINAL_PROMPT=0 git -c credential.helper= clone --quiet "$LIFTOFF_GIT" "$tmp/src" &&
		git -C "$tmp/src" -c advice.detachedHead=false checkout --quiet "$PINNED_COMMIT"; then
		mv "$tmp/src" "$dest"
	else
		log "git clone failed; trying the snapshot archive of $PINNED_COMMIT"
		rm -rf "$tmp/src"
		curl -fsSL -o "$tmp/libliftoff.tar.gz" "$LIFTOFF_ARCHIVE" || die "could not fetch $LIFTOFF_ARCHIVE"
		mkdir "$tmp/unpack"
		tar -xzf "$tmp/libliftoff.tar.gz" -C "$tmp/unpack"
		[ -d "$tmp/unpack/libliftoff-$PINNED_COMMIT" ] || die "archive does not hold libliftoff-$PINNED_COMMIT/"
		echo "$PINNED_COMMIT" >"$tmp/unpack/libliftoff-$PINNED_COMMIT/$MARKER"
		mv "$tmp/unpack/libliftoff-$PINNED_COMMIT" "$dest"
	fi
	rm -rf "$tmp"
}

if [ ! -e "$src" ]; then
	fetch_liftoff "$src"
fi
if [ ! -f "$src/meson.build" ] || [ ! -f "$src/include/libliftoff.h" ]; then
	die "$src is not a libliftoff source tree"
fi
src="$(cd "$src" && pwd -P)"

# --- check the commit -------------------------------------------------------

commit=""
if [ -e "$src/.git" ]; then
	commit="$(git -C "$src" rev-parse HEAD)"
elif [ -f "$src/$MARKER" ]; then
	commit="$(tr -d '[:space:]' <"$src/$MARKER")"
else
	case "$(basename "$src")" in
	*"$PINNED_COMMIT"*) commit="$PINNED_COMMIT" ;;
	esac
fi
if [ "$commit" != "$PINNED_COMMIT" ]; then
	if [ "$allow_other" -eq 1 ]; then
		log "WARNING: $src is at '${commit:-unknown}', not the pinned $PINNED_COMMIT (--allow-other-commit)"
	else
		die "$src is at '${commit:-unknown}', not the pinned $PINNED_COMMIT; pass --allow-other-commit to build it anyway"
	fi
fi

# --- tools: meson and ninja -------------------------------------------------

if ! command -v meson >/dev/null || ! command -v ninja >/dev/null; then
	[ -n "$deps_dir" ] || die "meson/ninja missing and CONNECTED_DEPS_DIR unset"
	venv="$deps_dir/meson-venv"
	if [ ! -x "$venv/bin/meson" ] || [ ! -x "$venv/bin/ninja" ]; then
		log "installing meson and ninja into $venv"
		python3 -m venv "$venv"
		"$venv/bin/pip" install --quiet meson ninja
	fi
	PATH="$venv/bin:$PATH"
	export PATH
fi
cc="${CC:-cc}"

# --- libdrm headers ---------------------------------------------------------

# libdrm is needed for its headers only: the fake library provides the symbols.
if pkg-config --exists 'libdrm >= 2.4.108' 2>/dev/null; then
	drm_cflags="$(pkg-config --cflags libdrm)"
	log "libdrm headers: system pkg-config libdrm $(pkg-config --modversion libdrm)"
else
	[ -n "$deps_dir" ] || die "no system libdrm >= 2.4.108 and CONNECTED_DEPS_DIR unset"
	drm_src="$deps_dir/libdrm"
	if [ ! -f "$drm_src/xf86drmMode.h" ]; then
		drm_src="$deps_dir/libdrm-$LIBDRM_VERSION"
		if [ ! -f "$drm_src/xf86drmMode.h" ]; then
			log "fetching libdrm $LIBDRM_VERSION headers into $drm_src"
			tmp="$(mktemp -d "$deps_dir/.libdrm-fetch.XXXXXX")"
			curl -fsSL -o "$tmp/libdrm.tar.xz" "$LIBDRM_ARCHIVE" || die "could not fetch $LIBDRM_ARCHIVE"
			tar -xJf "$tmp/libdrm.tar.xz" -C "$tmp"
			mv "$tmp/libdrm-$LIBDRM_VERSION" "$drm_src"
			rm -rf "$tmp"
		fi
	fi
	drm_version="$(sed -n "s/^ *version *: *'\([0-9.]*\)'.*/\1/p" "$drm_src/meson.build" | head -n1)"
	drm_version="${drm_version:-$LIBDRM_VERSION}"
	mkdir -p "$build_dir/pkgconfig"
	cat >"$build_dir/pkgconfig/libdrm.pc" <<EOF
Name: libdrm
Description: libdrm headers for the cross-check (symbols come from the fake library)
Version: $drm_version
Cflags: -I$drm_src -I$drm_src/include/drm
Libs: -L$build_dir/fakedrm -ldrm
EOF
	PKG_CONFIG_PATH="$build_dir/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
	export PKG_CONFIG_PATH
	drm_cflags="$(pkg-config --cflags libdrm)"
	log "libdrm headers: $drm_src (version $drm_version)"
fi

# --- build ------------------------------------------------------------------

liftoff_build="$build_dir/liftoff"
if [ ! -f "$liftoff_build/build.ninja" ]; then
	meson setup "$liftoff_build" "$src" --buildtype=debugoptimized \
		-Ddefault_library=static -Dwerror=false >&2
fi
ninja -C "$liftoff_build" >&2
liftoff_lib="$(find "$liftoff_build" -maxdepth 1 -name 'libliftoff.a' -print -quit)"
[ -n "$liftoff_lib" ] || die "libliftoff.a not found in $liftoff_build"

warn=(-std=c11 -Wall -Wextra -Werror -O2 -g)
# Named libdrm.so so that -ldrm resolves to it, in place of the system libdrm,
# the way libliftoff's own tests link their mock.
mkdir -p "$build_dir/fakedrm"
# shellcheck disable=SC2086 # pkg-config output is a list of flags
"$cc" "${warn[@]}" -fPIC -shared -Wl,-soname,libdrm.so \
	-o "$build_dir/fakedrm/libdrm.so" "$here/c/fake_drm.c" $drm_cflags
# shellcheck disable=SC2086
"$cc" "${warn[@]}" -o "$build_dir/liftoff-xcheck-driver" "$here/c/driver.c" \
	-I"$src/include" $drm_cflags "$liftoff_lib" \
	-L"$build_dir/fakedrm" -ldrm -Wl,-rpath,"$build_dir/fakedrm"

echo "$commit" >"$build_dir/liftoff-commit"
log "libliftoff commit: $commit"
echo "$build_dir/liftoff-xcheck-driver"
