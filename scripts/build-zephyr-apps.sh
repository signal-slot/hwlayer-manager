#!/bin/sh
# West-build every Zephyr app of the repository from ZEPHYR_WORKSPACE.
#
# An app is a directory tests/board/* or tests/zephyr/* holding CMakeLists.txt
# and prj.conf. Its board is the first '-b <board>' of a 'west build' line in
# its README.md. Each app is built with
#   west build -p always -b <board> -d <build-root>/<app> <app>
# run inside ZEPHYR_WORKSPACE.
#
# Usage: build-zephyr-apps.sh [--dry-run] [--build-root DIR] [--keep-going] [ROOT]
#   --dry-run       print the commands, run nothing (needs no environment)
#   --build-root D  build directory root (default: $TMPDIR, else <ROOT>/target;
#                   never /tmp)
#   --keep-going    build the remaining apps after a failure
#   ROOT            repository root (default: the one holding this script)
# Exit: 0 all built, 1 a build failed, 2 missing environment or bad usage.
set -u

dry=0
keep=0
build_root=
root=
while [ $# -gt 0 ]; do
    case $1 in
    --dry-run) dry=1 ;;
    --keep-going) keep=1 ;;
    --build-root)
        [ $# -ge 2 ] || { echo "error: --build-root needs a directory" >&2; exit 2; }
        build_root=$2
        shift
        ;;
    -h | --help)
        sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    -*) echo "error: unknown option $1" >&2; exit 2 ;;
    *) root=$1 ;;
    esac
    shift
done

if [ -z "$root" ]; then
    root=$(cd "$(dirname "$0")/.." && pwd)
else
    root=$(cd "$root" 2> /dev/null && pwd) || { echo "error: no such root" >&2; exit 2; }
fi

if [ -z "$build_root" ]; then
    case ${TMPDIR:-} in
    '' | /tmp | /tmp/* | /var/tmp | /var/tmp/*) build_root=$root/target/zephyr-apps ;;
    *) build_root=$TMPDIR/zephyr-apps ;;
    esac
fi

if [ "$dry" -eq 0 ]; then
    missing=
    [ -n "${ZEPHYR_WORKSPACE:-}" ] || missing="$missing ZEPHYR_WORKSPACE is not set."
    [ -n "${ZEPHYR_SDK_INSTALL_DIR:-}" ] || missing="$missing ZEPHYR_SDK_INSTALL_DIR is not set."
    command -v west > /dev/null 2>&1 || missing="$missing west not found on PATH."
    if [ -n "$missing" ]; then
        echo "error: missing environment:$missing" >&2
        echo "see scripts/prepare-zephyr.sh to prepare a workspace and SDK" >&2
        exit 2
    fi
    [ -d "$ZEPHYR_WORKSPACE" ] || { echo "error: ZEPHYR_WORKSPACE is not a directory" >&2; exit 2; }
fi

built=0
failed=0
for app in "$root"/tests/board/* "$root"/tests/zephyr/*; do
    [ -f "$app/CMakeLists.txt" ] && [ -f "$app/prj.conf" ] || continue
    name=${app#"$root"/tests/}
    name=$(printf '%s' "$name" | tr '/' '_')
    board=$(sed -n 's/.*west build.* -b \{1,\}\([^ ]*\).*/\1/p' "$app/README.md" 2> /dev/null | sed -n 1p)
    if [ -z "$board" ]; then
        echo "fail $name: no 'west build -b <board>' line in README.md" >&2
        failed=$((failed + 1))
        [ "$keep" -eq 1 ] || break
        continue
    fi
    if [ "$dry" -eq 1 ]; then
        echo "west build -p always -b $board -d $build_root/$name $app"
        continue
    fi
    echo "build $name ($board)"
    if (cd "$ZEPHYR_WORKSPACE" && west build -p always -b "$board" -d "$build_root/$name" "$app"); then
        built=$((built + 1))
    else
        echo "fail $name: west build failed" >&2
        failed=$((failed + 1))
        [ "$keep" -eq 1 ] || break
    fi
done

if [ "$dry" -eq 1 ]; then
    [ "$failed" -eq 0 ]
    exit
fi
echo "built $built, failed $failed"
[ "$failed" -eq 0 ]
