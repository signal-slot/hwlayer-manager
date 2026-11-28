#!/bin/sh
# Check that a session doc covers named questions with evidence tags.
# Usage: scripts/session-q-coverage.sh <doc.md> Q1 Q4 Q9 ...
# See scripts/session-q-coverage-README.md.
case ${1:-} in
    -h|--help) echo "usage: $0 <doc.md> Qn [Qn...]"; exit 0 ;;
esac
[ $# -ge 2 ] || { echo "usage: $0 <doc.md> Qn [Qn...]" >&2; exit 2; }
doc=$1; shift
[ -f "$doc" ] || { echo "$0: not a file: $doc" >&2; exit 2; }
for q in "$@"; do
    case $q in
        Q[0-9]*) case ${q#Q} in *[!0-9]*) echo "$0: bad question id: $q" >&2; exit 2 ;; esac ;;
        *) echo "$0: bad question id: $q" >&2; exit 2 ;;
    esac
done
awk -v QS="$*" '
function hlevel(s) { if (s !~ /^#+[ \t]/) return 0; match(s, /^#+/); return RLENGTH }
function scan(a, b,   i, l, d) {
    d = 0
    for (i = a; i <= b; i++)
        if (line[i] ~ /[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]/) d = 1
    for (i = a; i <= b; i++) {
        l = tolower(line[i])
        if (l ~ /observed/ && d) return "observed"
        if (l ~ /read from source/) return "read-from-source"
        if (l ~ /inferred/) return "inferred"
    }
    return ""
}
{ line[NR] = $0 }
END {
    nq = split(QS, qa, " ")
    rc = 0
    for (k = 1; k <= nq; k++) {
        q = qa[k]; re = "(^|[^A-Za-z0-9])" q "([^A-Za-z0-9]|$)"
        found = 0; tag = ""
        for (i = 1; i <= NR && tag == ""; i++) {
            if (line[i] !~ re) continue
            found = 1
            if (line[i] ~ /^[ \t]*\|/) { a = i; b = i }
            else {
                h = 0
                for (j = i; j >= 1; j--) if (hlevel(line[j])) { h = j; break }
                if (h == 0) { a = 1; b = NR }
                else {
                    L = hlevel(line[h]); a = h; b = NR
                    for (j = h + 1; j <= NR; j++) {
                        x = hlevel(line[j])
                        if (x && x <= L) { b = j - 1; break }
                    }
                }
            }
            tag = scan(a, b)
        }
        if (tag != "") print "COVERED " q " " tag
        else { print "MISSING " q (found ? " (named, no tag)" : ""); rc = 1 }
    }
    exit rc
}' "$doc"
