#!/bin/sh
# Structural check for dated Linux board results docs.
# Usage: scripts/check-board-results.sh FILE...
# Layout and rules: scripts/check-board-results-README.md.
# Prints "file:line: reason" per problem; exit 1 on findings, 2 on usage errors.

case ${1:-} in
    ''|-*) echo "usage: $0 FILE..." >&2; exit 2 ;;
esac
for f in "$@"; do
    [ -f "$f" ] || { echo "$0: not a file: $f" >&2; exit 2; }
done

rc=0
for f in "$@"; do
    out=$(awk -v F="$f" '
    function trim(s) { gsub(/^[ \t]+|[ \t]+$/, "", s); return s }
    function close_board(   r) {
        if (board == "") return
        for (r = 1; r <= 6; r++)
            if (!seen["A" r])
                printf "%s:%d: board \"%s\": no row A%d\n", F, bline, board, r
    }
    BEGIN { nb = 0; board = "" }
    {
        l = tolower($0)
        if (l ~ /portability evidence/ && l ~ /never count toward m1, m2 or m3/) disc = 1
    }
    /^#+[ \t]/ {
        close_board()
        board = ""
        if ($0 ~ /^##[ \t]+Board:[ \t]*[^ \t]/) {
            board = trim(substr($0, index($0, ":") + 1))
            bline = NR; nb++
            split("", seen)
        }
        next
    }
    board != "" && $0 ~ /^[ \t]*\|/ {
        s = trim($0)
        sub(/^\|/, "", s); sub(/\|$/, "", s)
        split(s, c, "|")
        id = trim(c[1])
        if (id !~ /^A[0-9]+$/) next      # header, separator, other tables
        if (id !~ /^A[1-6]$/) {
            printf "%s:%d: unknown row id %s\n", F, NR, id; next
        }
        if (seen[id]) printf "%s:%d: duplicate row %s in board \"%s\"\n", F, NR, id, board
        seen[id] = 1
        v = trim(c[2]); t = trim(c[3]); g = tolower(trim(c[4]))
        if (v !~ /^(PASS|FAIL|BLOCKED|SKIPPED)$/)
            printf "%s:%d: row %s: verdict \"%s\" is not PASS, FAIL, BLOCKED or SKIPPED\n", F, NR, id, v
        if (t !~ /^[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9](Z|[+-][0-9][0-9]:[0-9][0-9])?$/)
            printf "%s:%d: row %s: host time \"%s\" is not ISO 8601 YYYY-MM-DDThh:mm:ss[Z|+hh:mm]\n", F, NR, id, t
        if (g != "observed" && g != "read from source" && g != "inferred")
            printf "%s:%d: row %s: tag \"%s\" is not observed, read from source or inferred\n", F, NR, id, g
    }
    END {
        close_board()
        if (!nb) printf "%s:1: no board section (\"## Board: NAME\")\n", F
        if (!disc) printf "%s:1: missing disclaimer (portability evidence ... never count toward M1, M2 or M3)\n", F
    }' "$f")
    if [ -n "$out" ]; then
        printf '%s\n' "$out"
        rc=1
    fi
done
exit "$rc"
