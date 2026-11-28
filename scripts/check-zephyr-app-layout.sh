#!/bin/sh
# Check the layout of every Zephyr app under tests/board/ (or the root in $1).
# An app is an immediate subdirectory with a CMakeLists.txt. Checks: prj.conf
# exists; src/ exists and is not empty (hidden files such as .gitkeep do not count); README.md exists and holds a
# `west build` command; no file refers to the showcase (MCUI_SHOWCASE_DIR).
# Prints one line per directory: "ok <name>", "fail <name>: <reasons>" or
# "skip <name>: <why>". Exits 1 if any app fails, 2 on a bad root, else 0.
set -u

root=${1:-$(cd "$(dirname "$0")/.." && pwd)/tests/board}
[ -d "$root" ] || { echo "error: no such directory: $root" >&2; exit 2; }
status=0

for app in "$root"/*/; do
    [ -d "$app" ] || continue
    app=${app%/}
    name=${app##*/}
    if [ ! -f "$app/CMakeLists.txt" ]; then
        echo "skip $name: not an app (no CMakeLists.txt)"
        continue
    fi
    reasons=""
    [ -f "$app/prj.conf" ] || reasons="$reasons missing prj.conf;"
    if [ ! -d "$app/src" ]; then
        reasons="$reasons missing src/;"
    elif [ -z "$(find "$app/src" -type f ! -name '.*' | head -n 1)" ]; then
        reasons="$reasons src/ is empty;"
    fi
    if [ ! -f "$app/README.md" ]; then
        reasons="$reasons missing README.md;"
    elif ! grep -q 'west build' "$app/README.md"; then
        reasons="$reasons README.md has no west build command;"
    fi
    if grep -rIq 'MCUI_SHOWCASE_DIR' "$app" 2>/dev/null; then
        reasons="$reasons refers to MCUI_SHOWCASE_DIR;"
    fi
    if [ -n "$reasons" ]; then
        reasons=${reasons# }
        reasons=${reasons%;}
        echo "fail $name: $reasons"
        status=1
    else
        echo "ok $name"
    fi
done
exit $status
