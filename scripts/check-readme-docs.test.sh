#!/bin/bash
# Tests for check-readme-docs.sh using fake repositories in a temporary directory.
set -u
here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-readme-docs.sh
tmp=$(mktemp -d "${TMPDIR:-$here}/readmedocs.XXXXXX") || exit 1
trap 'rm -rf "$tmp"' EXIT
fails=0

expect() { # name want_rc pattern root
    local out rc
    out=$(sh "$script" "$4" 2>&1); rc=$?
    if [ "$rc" -ne "$2" ] || ! printf '%s\n' "$out" | grep -qE -- "$3"; then
        echo "FAIL $1 (rc=$rc): $out"
        fails=$((fails + 1))
    else
        echo "pass $1"
    fi
}

# all linked
mkdir -p "$tmp/all/docs"
: > "$tmp/all/docs/a.md"
: > "$tmp/all/docs/b.txt"
printf 'See docs/a.md and docs/b.txt.\n' > "$tmp/all/README.md"
expect all-linked 0 '^ok$' "$tmp/all"

# one missing
mkdir -p "$tmp/one/docs"
: > "$tmp/one/docs/a.md"
: > "$tmp/one/docs/b.md"
printf 'See docs/a.md.\n' > "$tmp/one/README.md"
expect one-missing 1 '^missing docs/b.md$' "$tmp/one"
out=$(sh "$script" "$tmp/one")
case $out in
*"docs/a.md"*) echo "FAIL a.md reported"; fails=$((fails + 1)) ;;
esac

# directory entry
mkdir -p "$tmp/dir/docs/sub"
: > "$tmp/dir/docs/sub/x.md"
printf 'nothing\n' > "$tmp/dir/README.md"
expect dir-missing 1 '^missing docs/sub$' "$tmp/dir"
printf 'See docs/sub/ here.\n' > "$tmp/dir/README.md"
expect dir-linked 0 '^ok$' "$tmp/dir"

# missing README.md
mkdir -p "$tmp/nor/docs"
: > "$tmp/nor/docs/a.md"
expect no-readme 2 'README.md not found' "$tmp/nor"

if [ "$fails" -ne 0 ]; then
    echo "$fails failure(s)"
    exit 1
fi
echo "all passed"
