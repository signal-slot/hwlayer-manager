#!/bin/sh
# Check the AM62P column of docs/am62p-imx95-matrix.md.
#
# Every cell of the AM62P column (a table whose header has a cell starting with
# "AM62P") must carry a recognised tag:
#   observed (YYYY-MM-DD) | read from source | source | inferred | SoW | repo
#   | not recorded | a cited document abbreviation such as `TP` or `PC`
# An "observed" cell must have a real calendar date that is not in the future.
#
# --require-observed (for the M3 acceptance): at least one AM62P cell is
# observed, docs/observed/am62p.md exists, and it holds every observed date.
#
# Usage: check-am62p-matrix.sh [--require-observed] [--matrix FILE] [--observed FILE]
# TODAY=YYYY-MM-DD overrides the clock (tests). Exit 0 ok, 1 check failed, 2 usage.
set -u
here=$(cd "$(dirname "$0")" && pwd)
matrix=$here/../docs/am62p-imx95-matrix.md
observed=$here/../docs/observed/am62p.md
require=0
usage() {
    echo "usage: $0 [--require-observed] [--matrix FILE] [--observed FILE]" >&2
    exit 2
}
while [ $# -gt 0 ]; do
    case $1 in
    --require-observed) require=1 ;;
    --matrix)
        [ $# -ge 2 ] || usage
        matrix=$2
        shift
        ;;
    --observed)
        [ $# -ge 2 ] || usage
        observed=$2
        shift
        ;;
    *) usage ;;
    esac
    shift
done
today=${TODAY:-$(date +%F)}
[ -r "$matrix" ] || {
    echo "FAIL: cannot read $matrix" >&2
    exit 1
}

# awk prints "ERR <message>" per problem and "DATE <date>" per observed cell.
res=$(awk -F'|' -v today="$today" '
function trim(s) { gsub(/^[ \t]+|[ \t]+$/, "", s); return s }
function validdate(d,  y, m, dd, mx) {
    if (d !~ /^[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]$/) return 0
    y = substr(d, 1, 4) + 0; m = substr(d, 6, 2) + 0; dd = substr(d, 9, 2) + 0
    if (m < 1 || m > 12 || dd < 1) return 0
    mx = 31
    if (m == 4 || m == 6 || m == 9 || m == 11) mx = 30
    if (m == 2) mx = ((y % 4 == 0 && y % 100 != 0) || y % 400 == 0) ? 29 : 28
    return dd <= mx
}
/^[ \t]*\|/ {
    if ($0 ~ /^[ \t]*\|[ \t:-]+\|[ \t|:-]*$/) next
    if (!intable) {
        intable = 1; col = 0
        for (i = 2; i < NF; i++) if (trim($i) ~ /^AM62P/) { col = i; break }
        if (col) found = 1
        next
    }
    if (!col) next
    cell = trim($col); row = trim($2)
    if (cell == "") { print "ERR line " NR ": empty AM62P cell (" row ")"; next }
    rest = cell; ok = 0; bad = 0
    gsub(/not observed/, "", rest)
    while (match(rest, /observed[ \t]*\(?/)) {
        tok = substr(rest, RSTART, RLENGTH)
        tail = substr(rest, RSTART + RLENGTH)
        if (tok ~ /\($/ && match(tail, /^[^)]*\)/)) {
            d = substr(tail, 1, RLENGTH - 1)
            tail = substr(tail, RLENGTH + 1)
            if (validdate(d) && d <= today) { print "DATE " d; ok = 1 }
            else { print "ERR line " NR ": observed cell has malformed or future date \"" d "\" (" row ")"; bad = 1 }
        } else {
            print "ERR line " NR ": observed cell has no (YYYY-MM-DD) date (" row ")"; bad = 1
        }
        rest = tail
    }
    if (!ok && !bad && cell !~ /read from source|source|inferred|SoW|repo|not recorded|`(PC|DPU66|TP|BOARD|WP5|M1)`/)
        print "ERR line " NR ": AM62P cell has no recognised tag (" row ")"
    next
}
{ intable = 0 }
END { if (!found) print "ERR no table with an AM62P column" }
' "$matrix")

rc=0
errs=$(printf '%s\n' "$res" | sed -n 's/^ERR //p')
if [ -n "$errs" ]; then
    printf '%s\n' "$errs" | sed 's/^/FAIL: /' >&2
    rc=1
fi
dates=$(printf '%s\n' "$res" | sed -n 's/^DATE //p' | sort -u)

if [ "$require" -eq 1 ]; then
    if [ -z "$dates" ]; then
        echo "FAIL: --require-observed: no AM62P cell is observed (YYYY-MM-DD) in $matrix" >&2
        rc=1
    elif [ ! -f "$observed" ]; then
        echo "FAIL: --require-observed: $observed does not exist" >&2
        rc=1
    else
        for d in $dates; do
            grep -q -e "$d" "$observed" || {
                echo "FAIL: --require-observed: $observed does not hold the observed date $d" >&2
                rc=1
            }
        done
    fi
fi
if [ $rc -eq 0 ]; then
    echo "check-am62p-matrix: ok (observed dates: $(echo $dates))"
fi
exit $rc
