#!/bin/sh
# Self-test: a modified copy of the doc must make check-am62p-doc-scenes.sh FAIL.
here=$(cd "$(dirname "$0")" && pwd)
root=$(dirname "$here")
doc=$root/docs/board-test-am62p-r5f.md
tmpdir=${TMPDIR:-/tmp}
work=$(mktemp -d "$tmpdir/doc-scenes-test.XXXXXX") || exit 1
trap 'rm -rf "$work"' EXIT HUP INT TERM

command -v cargo >/dev/null 2>&1 || { echo "SKIP: cargo not found"; exit 0; }

expect_fail() { # label sed-expr
    sed "$2" "$doc" >"$work/doc.md"
    if cmp -s "$doc" "$work/doc.md"; then echo "test bug: edit '$1' changed nothing"; exit 1; fi
    out=$(sh "$here/check-am62p-doc-scenes.sh" "$work/doc.md")
    case $(echo "$out" | tail -n 1) in
        FAIL:*) echo "ok: $1" ;;
        *) echo "FAIL (test): '$1' not detected"; echo "$out" | tail -n 3; exit 1 ;;
    esac
}

expect_fail "commit count" 's/3 commits (buffer/4 commits (buffer/'
expect_fail "scene name" 's/`app-replaced`/`app-swapped`/'
expect_fail "release count" '/^| 3 |/s/1 release/2 releases/'
expect_fail "status" '/^| 7 |/s/CE_ERR_BACKEND_REJECTED/CE_OK/'
expect_fail "missing scene" '/^| 8 |/d'

last=$(sh "$here/check-am62p-doc-scenes.sh" | tail -n 1)
[ "$last" = PASS ] || { echo "FAIL (test): unmodified doc gave '$last'"; exit 1; }
echo "all ok"
