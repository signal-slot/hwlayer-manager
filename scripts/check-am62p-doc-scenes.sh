#!/bin/sh
# Check that the scene table of docs/board-test-am62p-r5f.md matches the
# SCENE lines printed by the host test board_scenes_behind_a_fake_panel.
# Usage: check-am62p-doc-scenes.sh [doc-path]
#
# The doc has a per-scene table (| # | Name | Frame | Firmware expects | ...).
# Per scene it checks: the number, the name in backticks, the status
# (CE_OK = 0, CE_ERR_BACKEND_REJECTED = -16), the commit count ("N commit(s)")
# and the release count ("N release(s)", none stated = 0) against the host
# line's "expected" values, and that the host line ends in ": ok".
# Last line: PASS, FAIL: <reason> or SKIP: <reason>. Exit 0 for PASS/SKIP.
here=$(cd "$(dirname "$0")" && pwd)
root=$(dirname "$here")
doc=${1:-$root/docs/board-test-am62p-r5f.md}

fail() { echo "FAIL: $1"; exit 1; }

[ -f "$doc" ] || fail "doc not found: $doc"
command -v cargo >/dev/null 2>&1 || { echo "SKIP: cargo not found"; exit 0; }

tmpdir=${TMPDIR:-/tmp}
tmp=$(mktemp "$tmpdir/check-am62p-doc-scenes.XXXXXX") || fail "mktemp"
trap 'rm -f "$tmp"' EXIT HUP INT TERM
(cd "$root" && cargo test -p composition-twoplane board_scenes -- --nocapture) >"$tmp" 2>&1 \
    || { tail -n 20 "$tmp"; fail "host test failed"; }

lines=$(grep '^SCENE ' "$tmp")
[ -n "$lines" ] || fail "host test printed no SCENE lines"

# Doc rows: n|name|status|commits|releases
rows=$(awk -F'|' '
    $2 ~ /^ *[0-9]+ *$/ && NF >= 6 {
        n = $2; gsub(/ /, "", n)
        name = $3; gsub(/[ `]/, "", name)
        e = $5
        st = "?"
        if (e ~ /CE_OK/) st = 0
        else if (e ~ /CE_ERR_BACKEND_REJECTED/) st = -16
        c = 0; r = 0
        if (match(e, /[0-9]+ commits?/)) { s = substr(e, RSTART, RLENGTH); sub(/ .*/, "", s); c = s }
        if (match(e, /[0-9]+ releases?/)) { s = substr(e, RSTART, RLENGTH); sub(/ .*/, "", s); r = s }
        print n "|" name "|" st "|" c "|" r
    }' "$doc")
[ -n "$rows" ] || fail "no scene table rows in $doc"

count=0
while IFS= read -r l; do
    count=$((count + 1))
    n=$(echo "$l" | awk '{print $2}')
    name=$(echo "$l" | awk '{sub(/:$/, "", $3); print $3}')
    st=$(echo "$l" | sed -n 's/.*(expected \(-\{0,1\}[0-9]*\)), commits.*/\1/p')
    d=$(echo "$l" | sed -n 's/.*(+[0-9]*, expected +\([0-9]*\)).*/\1/p')
    r=$(echo "$l" | sed -n 's/.*released [0-9]* (expected \([0-9]*\)).*/\1/p')
    case $l in *": ok") ;; *) fail "host scene $n is not ok: $l";; esac
    row=$(echo "$rows" | awk -F'|' -v n="$n" '$1 == n')
    [ -n "$row" ] || fail "scene $n missing from the doc table"
    want="$n|$name|$st|$d|$r"
    [ "$row" = "$want" ] || fail "scene $n: doc (number|name|status|commits|releases) is '$row', host test expects '$want'"
done <<EOT
$lines
EOT

drows=$(echo "$rows" | wc -l | tr -d ' ')
[ "$drows" = "$count" ] || fail "doc table has $drows scenes, host test printed $count"
echo "PASS"
