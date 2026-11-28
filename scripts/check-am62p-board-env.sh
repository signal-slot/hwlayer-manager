#!/bin/sh
# Say whether this node can build the AM62P R5F board-test image.
#
# Usage: check-am62p-board-env.sh
#
# Reads three environment variables (read-only; nothing is written or copied):
#   MCUI_SHOWCASE_DIR       directory with zephyr/module.yml (a Zephyr module)
#   ZEPHYR_WORKSPACE        west workspace: .west/ and zephyr/VERSION
#   ZEPHYR_SDK_INSTALL_DIR  Zephyr SDK: cmake/ or sdk_version
# and looks for west and cmake on PATH. Layout: docs/zephyr-workspace.md.
#
# Output, one line each (values and paths are never printed, they are
# node-specific):
#   ENV <NAME> ok | missing | unusable <what is absent>
#   TOOL <name> ok | missing
#   BOARD-ENV OK                     (exit 0)
#   BOARD-ENV MISSING <names...>     (exit 1)
set -u

bad=""

# check_env NAME VALUE SPEC...
# SPEC: "f:path" (file), "d:path" (directory) or "any:path|path" (one exists)
check_env() {
    name=$1
    val=$2
    shift 2
    if [ -z "$val" ]; then
        echo "ENV $name missing"
        bad="$bad $name"
        return
    fi
    if [ ! -d "$val" ]; then
        echo "ENV $name unusable not a directory"
        bad="$bad $name"
        return
    fi
    absent=""
    for spec in "$@"; do
        kind=${spec%%:*}
        rest=${spec#*:}
        case $kind in
        f) [ -f "$val/$rest" ] || absent="$absent $rest;" ;;
        d) [ -d "$val/$rest" ] || absent="$absent $rest/;" ;;
        any)
            found=0
            oldifs=$IFS
            IFS='|'
            for alt in $rest; do
                [ -e "$val/$alt" ] && found=1
            done
            IFS=$oldifs
            [ "$found" -eq 1 ] || absent="$absent $rest;"
            ;;
        esac
    done
    if [ -n "$absent" ]; then
        absent=${absent# }
        absent=${absent%;}
        echo "ENV $name unusable $absent"
        bad="$bad $name"
    else
        echo "ENV $name ok"
    fi
}

check_tool() {
    if command -v "$1" >/dev/null 2>&1; then
        echo "TOOL $1 ok"
    else
        echo "TOOL $1 missing"
        bad="$bad $1"
    fi
}

check_env MCUI_SHOWCASE_DIR "${MCUI_SHOWCASE_DIR-}" f:zephyr/module.yml
check_env ZEPHYR_WORKSPACE "${ZEPHYR_WORKSPACE-}" d:.west f:zephyr/VERSION
check_env ZEPHYR_SDK_INSTALL_DIR "${ZEPHYR_SDK_INSTALL_DIR-}" 'any:cmake|sdk_version'
check_tool west
check_tool cmake

if [ -z "$bad" ]; then
    echo "BOARD-ENV OK"
    exit 0
fi
echo "BOARD-ENV MISSING$bad"
exit 1
