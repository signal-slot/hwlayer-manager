#!/bin/sh
# Prepare a pinned Zephyr west workspace and a minimal Zephyr SDK (arm-zephyr-eabi only).
#
#   scripts/prepare-zephyr.sh [--dry-run] [--clean-git-config] <dest-root>
#
# Everything is written under <dest-root>/zephyr:
#   venv/                  python venv with west and Zephyr's base requirements
#   workspace/             west workspace (workspace/zephyr, workspace/modules/...)
#   sdk/zephyr-sdk-<ver>/  minimal SDK bundle + the arm-zephyr-eabi toolchain
#   downloads/             SDK archives (removed after a successful install)
# and the two lines to export are printed at the end. Re-running is safe: finished
# steps are detected and skipped. Nothing is written outside <dest-root>/zephyr
# (the SDK's CMake package registration, which writes to ~/.cmake, is not run).
# Versions and the reasons for them: docs/zephyr-workspace.md.
set -eu

ZEPHYR_TAG=v4.5.0
SDK_VERSION=1.0.1
WEST_VERSION=1.5.0
# west projects fetched besides zephyr itself; everything else in west.yml is skipped.
WEST_MODULES="cmsis cmsis_6 hal_nxp"
SDK_BASE=https://github.com/zephyrproject-rtos/sdk-ng/releases/download/v$SDK_VERSION
SDK_MINIMAL=zephyr-sdk-${SDK_VERSION}_linux-x86_64_minimal.tar.xz
SDK_MINIMAL_SHA256=ca9bc0ff66fafca1dac9d592a36d953cf16d096a9d09b1c0357f021cf9f6a7eb
SDK_ARM=toolchain_gnu_linux-x86_64_arm-zephyr-eabi.tar.xz
SDK_ARM_SHA256=21b85981cb5a1818d9bc53d82af80f208946ec038b982ff1907287572ed3a634

usage() {
	echo "usage: $0 [--dry-run] [--clean-git-config] <dest-root>" >&2
	echo "  --dry-run           print the steps, change nothing" >&2
	echo "  --clean-git-config  ignore ~/.gitconfig and the system git config (GIT_CONFIG_GLOBAL=/dev/null)" >&2
	exit 2
}

DRY=0
DEST=
while [ $# -gt 0 ]; do
	case $1 in
	--dry-run) DRY=1 ;;
	--clean-git-config) GIT_CONFIG_GLOBAL=/dev/null; GIT_CONFIG_NOSYSTEM=1; export GIT_CONFIG_GLOBAL GIT_CONFIG_NOSYSTEM ;;
	-h|--help) usage ;;
	-*) usage ;;
	*) [ -z "$DEST" ] || usage; DEST=$1 ;;
	esac
	shift
done
[ -n "$DEST" ] || usage

ROOT=${DEST%/}/zephyr
VENV=$ROOT/venv
WS=$ROOT/workspace
SDK=$ROOT/sdk/zephyr-sdk-$SDK_VERSION
DL=$ROOT/downloads

case $(uname -s)-$(uname -m) in
Linux-x86_64) ;;
*) echo "error: only Linux x86_64 hosts are pinned (SDK archives above)" >&2; exit 1 ;;
esac

# say <description> ; run <cmd...>: print in dry-run, execute otherwise.
say() { echo "==> $*"; }
run() {
	if [ "$DRY" = 1 ]; then echo "    $*"; else "$@"; fi
}

need() {
	command -v "$1" >/dev/null 2>&1 || { echo "error: '$1' not found in PATH" >&2; exit 1; }
}
if [ "$DRY" = 0 ]; then
	for t in git python3 cmake ninja dtc tar sha256sum; do need $t; done
	if command -v curl >/dev/null 2>&1; then FETCH="curl -fsSL -o"
	elif command -v wget >/dev/null 2>&1; then FETCH="wget -q -O"
	else echo "error: need curl or wget" >&2; exit 1; fi
else
	FETCH="curl -fsSL -o"
	echo "dry run: nothing is created. Host tools needed: git python3 (with venv) cmake ninja dtc tar sha256sum curl|wget"
fi

say "destination: $ROOT"
run mkdir -p "$ROOT"

# 1. python venv with west and Zephyr's base requirements
say "python venv with west==$WEST_VERSION"
if [ "$DRY" = 1 ] || [ ! -x "$VENV/bin/west" ]; then
	run python3 -m venv "$VENV"
	run "$VENV/bin/pip" install --quiet "west==$WEST_VERSION"
else
	echo "    already present"
fi

# 2. west workspace pinned to the release tag
say "west init -m https://github.com/zephyrproject-rtos/zephyr --mr $ZEPHYR_TAG $WS"
if [ "$DRY" = 1 ]; then
	echo "    $VENV/bin/west init -m https://github.com/zephyrproject-rtos/zephyr --mr $ZEPHYR_TAG $WS"
elif [ -d "$WS/.west" ]; then
	have=$(git -C "$WS/zephyr" describe --tags --exact-match 2>/dev/null || echo none)
	if [ "$have" != "$ZEPHYR_TAG" ]; then
		echo "error: $WS exists but zephyr is at '$have', not $ZEPHYR_TAG; remove it or pick another destination" >&2
		exit 1
	fi
	echo "    already initialised at $have"
else
	"$VENV/bin/west" init -m https://github.com/zephyrproject-rtos/zephyr --mr "$ZEPHYR_TAG" "$WS"
fi

say "west update $WEST_MODULES (shallow; all other west.yml projects are skipped)"
if [ "$DRY" = 1 ]; then
	echo "    cd $WS && $VENV/bin/west update --narrow --fetch-opt=--depth=1 $WEST_MODULES"
else
	(cd "$WS" && "$VENV/bin/west" update --narrow --fetch-opt=--depth=1 $WEST_MODULES)
fi

say "blobs: none fetched (hal_nxp blobs are Wi-Fi/BT/NPU libraries and EVK boot-firmware images, none linked into an i.MX95 M7 build)"

say "pip install -r zephyr/scripts/requirements-base.txt"
if [ "$DRY" = 1 ]; then
	echo "    $VENV/bin/pip install --quiet -r $WS/zephyr/scripts/requirements-base.txt"
elif [ ! -e "$VENV/.requirements-$ZEPHYR_TAG" ]; then
	"$VENV/bin/pip" install --quiet -r "$WS/zephyr/scripts/requirements-base.txt"
	: > "$VENV/.requirements-$ZEPHYR_TAG"
else
	echo "    already installed"
fi

# 3. minimal SDK with the arm-zephyr-eabi toolchain
fetch_checked() { # file sha256
	if [ -e "$DL/$1" ] && echo "$2  $DL/$1" | sha256sum -c - >/dev/null 2>&1; then return 0; fi
	$FETCH "$DL/$1.part" "$SDK_BASE/$1"
	if ! echo "$2  $DL/$1.part" | sha256sum -c - >/dev/null 2>&1; then
		rm -f "$DL/$1.part"
		echo "error: checksum mismatch for $1" >&2
		exit 1
	fi
	mv "$DL/$1.part" "$DL/$1"
}

say "SDK $SDK_VERSION minimal bundle ($SDK_MINIMAL, sha256 $SDK_MINIMAL_SHA256)"
if [ "$DRY" = 1 ]; then
	echo "    download $SDK_BASE/$SDK_MINIMAL, verify sha256, tar -xf into $ROOT/sdk"
elif [ -f "$SDK/sdk_version" ]; then
	echo "    already extracted"
else
	mkdir -p "$DL" "$ROOT/sdk"
	fetch_checked "$SDK_MINIMAL" "$SDK_MINIMAL_SHA256"
	tar -xf "$DL/$SDK_MINIMAL" -C "$ROOT/sdk"
fi

say "SDK toolchain arm-zephyr-eabi ($SDK_ARM, sha256 $SDK_ARM_SHA256)"
if [ "$DRY" = 1 ]; then
	echo "    download $SDK_BASE/$SDK_ARM, verify sha256, tar -xf into $SDK/gnu"
elif [ -d "$SDK/gnu/arm-zephyr-eabi" ]; then
	echo "    already installed"
else
	mkdir -p "$DL" "$SDK/gnu"
	fetch_checked "$SDK_ARM" "$SDK_ARM_SHA256"
	tar -xf "$DL/$SDK_ARM" -C "$SDK/gnu"
fi

say "remove downloaded archives"
if [ "$DRY" = 1 ]; then
	echo "    rm -rf $DL"
else
	rm -rf "$DL"
fi

say "done; export:"
echo "export ZEPHYR_WORKSPACE=$WS"
echo "export ZEPHYR_SDK_INSTALL_DIR=$SDK"
echo "# build: . $VENV/bin/activate && cd \$ZEPHYR_WORKSPACE && west build -b imx95_evk/mimx9596/m7 zephyr/samples/hello_world"
