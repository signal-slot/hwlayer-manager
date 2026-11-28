#!/bin/sh
# Check trace logs against the header rules of docs/trace-format.md
# (section 3.1 version, section 5.1 '# date:' and the not-a-hardware-result
# comment). Usage: sh scripts/check-trace-log.sh <log>...
# Output per log: 'ok' or 'fail <file>:<line>: <reason>'; warnings go to
# stderr. Exit 0 all pass, 1 any failure, 2 usage error.
# Only the shape is checked, as the doc does; no date is validated as real.

[ $# -ge 1 ] || { echo "usage: check-trace-log.sh <log>..." >&2; exit 2; }
for f in "$@"; do
    case $f in -*) echo "usage: check-trace-log.sh <log>..." >&2; exit 2 ;; esac
    [ -f "$f" ] && [ -r "$f" ] || { echo "check-trace-log.sh: cannot read $f" >&2; exit 2; }
done

rc=0
for f in "$@"; do
    out=$(awk -v F="$f" '
    function fail(n, why) { if (!bad) { printf "fail %s:%d: %s\n", F, n, why; bad = 1 } }
    { sub(/\r$/, "") }
    /^CETRACE( |$)/ {
        v = ""
        if (match($0, /^CETRACE v=[^ ]*/)) v = substr($0, 12, RLENGTH - 11)
        if (v != "1") fail(NR, "unsupported or missing version \"" v "\" (supported: v=1)")
        ncet++
        next
    }
    /^#/ {
        l = $0
        if (l ~ /^#[ \t]*date[ \t]*[:=][ \t]*[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9][ \t]*$/) {
            d = l; sub(/^#[ \t]*date[ \t]*[:=][ \t]*/, "", d); sub(/[ \t]*$/, "", d)
            if (date == "") { date = d; dline = NR }
            else if (d != date) fail(NR, "date " d " differs from " date " on line " dline)
            next
        }
        low = tolower(l)
        if (low ~ /not a hardware result/) { nothw = 1; next }
        if (!marked && (low ~ /(^|[^a-z])(host|vkms)([^a-z]|$)/ || low ~ /(^|[^a-z])dry[ -]?run([^a-z]|$)/)) { marked = 1; mline = NR }
        if (low ~ /hardware evidence|(^|[^a-z])board[ \t]*[:=]/) evid = 1
    }
    END {
        if (!ncet) fail(1, "no CETRACE line")
        if (date == "") fail(1, "no \"# date: YYYY-MM-DD\" comment line")
        if (marked && !nothw) fail(mline, "marked as a host, dry or vkms run but no \"not a hardware result\" comment")
        if (!bad) {
            if (!marked && !nothw && !evid) print "warn " F ": not marked as a host/dry/vkms run and no hardware evidence line" > "/dev/stderr"
            print "ok"
        }
        exit bad
    }' "$f")
    st=$?
    [ $st -eq 0 ] || rc=1
    printf '%s\n' "$out"
done
exit $rc
