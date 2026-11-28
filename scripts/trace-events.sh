#!/bin/sh
# Report which X-03 event kinds (validate, commit, latched, released,
# dropped) appear in CETRACE logs (docs/trace-format.md). Usage:
#   sh scripts/trace-events.sh [--require <kind,...>] <log>...
# Output: one line per log
#   '<file>: validate=N commit=N latched=N released=N dropped=N other=N'
# then 'SEEN <kinds>' and 'MISSING <kinds>' ('none' if empty) over all logs.
# 'other' counts every other ev= name (hdr, check, latch_timeout, ...).
# Lines that do not start with CETRACE are ignored. CRLF is accepted.
# Exit 0 ok, 1 a --require kind is missing, 2 usage error or unreadable file.

KINDS="validate commit latched released dropped"
usage() { echo "usage: trace-events.sh [--require <kind,...>] <log>..." >&2; exit 2; }

req=
while [ $# -gt 0 ]; do
    case $1 in
        --require) [ $# -ge 2 ] || usage; req=$2; shift 2 ;;
        --) shift; break ;;
        -*) usage ;;
        *) break ;;
    esac
done
[ $# -ge 1 ] || usage

# Validate the --require list: only the five X-03 kinds are allowed.
reqs=$(printf '%s' "$req" | tr ',' ' ')
for k in $reqs; do
    case " $KINDS " in
        *" $k "*) ;;
        *) echo "trace-events.sh: unknown kind \"$k\" (use: validate,commit,latched,released,dropped)" >&2; exit 2 ;;
    esac
done
if [ -n "$req" ] && [ -z "$reqs" ]; then usage; fi

for f in "$@"; do
    [ -f "$f" ] && [ -r "$f" ] || { echo "trace-events.sh: cannot read $f" >&2; exit 2; }
done

awk -v KINDS="$KINDS" -v REQ="$reqs" '
BEGIN { nk = split(KINDS, K, " ") }
function flush(   i, s) {
    s = file ":"
    for (i = 1; i <= nk; i++) s = s " " K[i] "=" c[K[i]]
    print s " other=" other
}
FNR == 1 { if (file != "") flush(); file = FILENAME; for (i = 1; i <= nk; i++) c[K[i]] = 0; other = 0 }
{ sub(/\r$/, "") }
/^CETRACE( |$)/ {
    ev = ""
    if (match($0, / ev=[^ ]*/)) ev = substr($0, RSTART + 4, RLENGTH - 4)
    known = 0
    for (i = 1; i <= nk; i++) if (ev == K[i]) { c[ev]++; seen[ev] = 1; known = 1 }
    if (!known) other++
}
END {
    if (file != "") flush()
    s = ""; m = ""
    for (i = 1; i <= nk; i++) {
        if (seen[K[i]]) s = s (s == "" ? "" : ",") K[i]
        else m = m (m == "" ? "" : ",") K[i]
    }
    print "SEEN " (s == "" ? "none" : s)
    print "MISSING " (m == "" ? "none" : m)
    n = split(REQ, R, " ")
    for (i = 1; i <= n; i++) if (!seen[R[i]]) bad = 1
    exit bad
}' "$@"
