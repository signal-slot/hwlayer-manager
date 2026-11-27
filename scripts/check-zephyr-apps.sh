#!/bin/sh
# Lint the layout of the Zephyr board-test apps under tests/board/*/ and
# tests/zephyr/*/. Usage: check-zephyr-apps.sh [repo-root]
# Prints "ok <dir>" or "fail <dir>: <reason>" per app; exits 1 on any failure.
set -u

root=${1:-$(cd "$(dirname "$0")/.." && pwd)}
status=0

check_app() {
    app=$1
    reasons=""
    for f in CMakeLists.txt prj.conf README.md; do
        [ -f "$app/$f" ] || reasons="$reasons missing $f;"
    done
    if [ ! -d "$app/src" ]; then
        reasons="$reasons missing src/;"
    elif [ -z "$(find "$app/src" -type f \( -name '*.c' -o -name '*.cpp' -o -name '*.cc' -o -name '*.S' -o -name '*.rs' \) | head -n 1)" ]; then
        reasons="$reasons src/ has no source file;"
    fi
    if [ -f "$app/README.md" ] && ! grep -q 'west build' "$app/README.md"; then
        reasons="$reasons README.md has no west build command;"
    fi
    if grep -rIqE '(^|[^0-9.])([0-9]{1,3}\.){3}[0-9]{1,3}([^0-9.]|$)' "$app" 2>/dev/null; then
        reasons="$reasons contains an IPv4 address;"
    fi
    if grep -rIqF '/dev/tty' "$app" 2>/dev/null; then
        reasons="$reasons contains /dev/tty;"
    fi
    if grep -rIqi 'password' "$app" 2>/dev/null; then
        reasons="$reasons contains password;"
    fi
    if [ -n "$reasons" ]; then
        reasons=${reasons# }
        reasons=${reasons%;}
        echo "fail $app: $reasons"
        status=1
    else
        echo "ok $app"
    fi
}

for parent in "$root/tests/board" "$root/tests/zephyr"; do
    [ -d "$parent" ] || continue
    for app in "$parent"/*/; do
        [ -d "$app" ] || continue
        check_app "${app%/}"
    done
done
exit "$status"
