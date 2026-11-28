#!/bin/sh
# Check that tests/board/README.md links or names every board app.
# Usage: check-board-apps-index.sh [repo-root]
# An app is a directory directly under tests/board/ that holds a
# CMakeLists.txt. The out-of-tree board root is tests/board/boards/ (if it
# exists) and must be mentioned as "boards/". An app name counts as mentioned
# only as a whole word, so "imx95_m7" is not satisfied by "imx95_m7_engine".
# Prints "missing <entry>" for each entry the README does not mention and
# exits 1; prints "ok" and exits 0 otherwise. A missing tests/board/README.md
# or tests/board is an error (exit 2).
set -u

root=${1:-$(cd "$(dirname "$0")/.." && pwd)}
dir=$root/tests/board
readme=$dir/README.md
status=0

if [ ! -d "$dir" ]; then
    echo "error: $dir not found" >&2
    exit 2
fi
if [ ! -f "$readme" ]; then
    echo "error: $readme not found" >&2
    exit 2
fi

for entry in "$dir"/*/; do
    [ -d "$entry" ] || continue
    name=$(basename "$entry")
    if [ "$name" = boards ]; then
        if ! grep -qE '(^|[^A-Za-z0-9_-])boards/' "$readme"; then
            echo "missing boards/ (out-of-tree board root)"
            status=1
        fi
        continue
    fi
    [ -f "$entry/CMakeLists.txt" ] || continue
    if ! grep -qE -- "(^|[^A-Za-z0-9_-])$name(\$|[^A-Za-z0-9_])" "$readme"; then
        echo "missing $name"
        status=1
    fi
done

[ "$status" -eq 0 ] && echo ok
exit "$status"
