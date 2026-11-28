#!/bin/sh
# am62p-state-snapshot.sh - read-only snapshot/compare of Verdin AM62P state.
# Run ON the board. See am62p-state-snapshot-README.md.
#   am62p-state-snapshot.sh [--root DIR] snapshot FILE
#   am62p-state-snapshot.sh [--root DIR] compare BEFORE AFTER   (SAME or diff; 1 on difference)
# STATE_SYSTEMCTL / STATE_DOCKER override the systemctl / docker commands.
set -u
ROOT=
while [ $# -gt 0 ]; do
    case $1 in
    --root) [ $# -ge 2 ] || { echo "--root needs DIR" >&2; exit 2; }; ROOT=${2%/}; shift 2 ;;
    *) break ;;
    esac
done
SYSTEMCTL=${STATE_SYSTEMCTL:-systemctl}
DOCKER=${STATE_DOCKER:-docker}
FMT='{{.Names}}'
usage() { echo "usage: $0 [--root DIR] snapshot FILE | compare BEFORE AFTER" >&2; exit 2; }
[ $# -ge 1 ] || usage

rd() { [ -r "$1" ] && tr -d '\n' <"$1" || printf '?'; }

body() {
    for d in "$ROOT"/sys/class/remoteproc/remoteproc*; do
        [ -d "$d" ] || continue
        echo "remoteproc name=$(rd "$d/name") state=$(rd "$d/state") firmware=$(rd "$d/firmware")"
    done
    # Bound devices per driver; the leading hex unit address is dropped.
    for d in "$ROOT"/sys/bus/platform/drivers/*; do
        [ -d "$d" ] || continue
        n=${d##*/}
        case $n in
        *tidss*|*dss*|*panel*|*bridge*|*lvds*|*dsi*|*remoteproc*|*rproc*) ;;
        *) continue ;;
        esac
        found=0
        for l in "$d"/*; do
            [ -L "$l" ] || continue
            b=${l##*/}
            [ "$b" = module ] && continue
            echo "driver $n bound $(printf '%s' "$b" | sed 's/^[0-9a-fA-F]*\.//')"
            found=1
        done
        [ $found = 1 ] || echo "driver $n bound none"
    done
    $SYSTEMCTL list-units --all --plain --no-legend --no-pager --type=service 2>/dev/null |
        awk '{print $1, $3}' | grep -Ei '^[^ ]*(docker|weston|portainer|display|tidss|wayland|cog)[^ ]* ' |
        sed 's/^/unit /'
    if names=$($DOCKER ps --format "$FMT" 2>/dev/null); then
        if [ -z "$names" ]; then echo "container none"; else printf '%s\n' "$names" | sed 's/^/container /'; fi
    else
        echo "container docker-unreachable"
    fi
    for c in "$ROOT"/sys/class/drm/card*-*; do
        [ -d "$c" ] || continue
        echo "connector $(printf '%s' "${c##*/}" | sed 's/^card[0-9]*-//') $(rd "$c/status")"
    done
    if [ -r "$ROOT/proc/cmdline" ]; then
        for w in $(cat "$ROOT/proc/cmdline"); do
            case $w in ostree=*) echo "boot $w" ;; esac
        done
    fi
    if [ -r "$ROOT/etc/os-release" ]; then
        grep -E '^(ID|VARIANT_ID|VERSION_ID|BUILD_ID)=' "$ROOT/etc/os-release" | sed 's/^/os /'
    fi
}

case $1 in
snapshot)
    [ $# -eq 2 ] || usage
    body | LC_ALL=C sort >"$2" || exit 1
    ;;
compare)
    [ $# -eq 3 ] || usage
    if diff -u "$2" "$3"; then echo SAME; else exit 1; fi
    ;;
*) usage ;;
esac
