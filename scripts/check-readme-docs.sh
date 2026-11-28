#!/bin/sh
# Check that README.md links every top-level entry of docs/.
# Usage: check-readme-docs.sh [repo-root]
# Prints "missing docs/<name>" for each entry README.md does not mention and
# exits 1; prints "ok" and exits 0 when all are mentioned. A missing README.md
# or docs/ directory is an error (exit 2).
set -u

root=${1:-$(cd "$(dirname "$0")/.." && pwd)}
status=0

if [ ! -f "$root/README.md" ]; then
    echo "error: $root/README.md not found" >&2
    exit 2
fi
if [ ! -d "$root/docs" ]; then
    echo "error: $root/docs not found" >&2
    exit 2
fi

for entry in "$root"/docs/* "$root"/docs/.[!.]*; do
    [ -e "$entry" ] || continue
    name=${entry##*/}
    if ! grep -qF -- "docs/$name" "$root/README.md"; then
        echo "missing docs/$name"
        status=1
    fi
done

[ "$status" -eq 0 ] && echo ok
exit "$status"
