#!/bin/sh
# check-m3-session-dir.sh DIR - is an M3 AM62P board session directory complete?
# Run it while the board is still in the remoteproc route, before the Torizon
# restore, so a missing item can still be collected.
#
# Layout (the READMEs of the photo manifest, board info and state snapshot do
# not fix one directory; this script picks it and accepts the names below):
#   DIR/trace.txt            trace dump (also trace0.txt, trace.log, console.txt)
#                            must hold "SCENE 1".."SCENE 8" and a "RESULT" line;
#                            a leading marker such as "[trace] " is allowed
#   DIR/photos/sceneN.jpg    N = 1..8 (.jpeg or .png also accepted)
#   DIR/verdicts/sceneN.txt  N = 1..8 (.json also accepted), as am62p-photo-manifest
#   DIR/board-info.md        board info table (also board-info.txt, board-info)
#   DIR/state-before.txt     snapshot before the session (also snapshot-before.txt,
#                            before.txt, state-before)
#   DIR/state-after.txt      optional; noted only, never counted as missing,
#                            because it exists only after the restore
# Output: one "present: X" or "missing: X" line per item, then
# "COMPLETE" or "INCOMPLETE: <n> missing". Exit 0 complete, 1 incomplete,
# 2 usage error. Read-only. Empty files count as missing.
set -u
if [ $# -ne 1 ] || [ ! -d "$1" ]; then
    echo "usage: check-m3-session-dir.sh DIR" >&2
    exit 2
fi
d=$1
missing=0

report() { # ok label
    if [ "$1" = 1 ]; then
        echo "present: $2"
    else
        echo "missing: $2"
        missing=$((missing + 1))
    fi
}
first_file() { # prints the first nonempty file among the arguments
    for f in "$@"; do
        if [ -s "$f" ]; then
            echo "$f"
            return 0
        fi
    done
    return 1
}

tr=$(first_file "$d/trace.txt" "$d/trace0.txt" "$d/trace.log" "$d/console.txt") || tr=
if [ -z "$tr" ]; then
    report 0 "trace file (trace.txt)"
else
    report 1 "trace file ($(basename "$tr"))"
    n=1
    while [ $n -le 8 ]; do
        if grep -Eq "(^|[^A-Za-z0-9_])SCENE $n([^0-9]|$)" "$tr"; then
            report 1 "trace SCENE $n"
        else
            report 0 "trace SCENE $n"
        fi
        n=$((n + 1))
    done
    if grep -Eq '(^|[^A-Za-z0-9_])RESULT([^A-Za-z0-9_]|$)' "$tr"; then
        report 1 "trace RESULT line"
    else
        report 0 "trace RESULT line"
    fi
fi

n=1
while [ $n -le 8 ]; do
    p=$(first_file "$d/photos/scene$n.jpg" "$d/photos/scene$n.jpeg" "$d/photos/scene$n.png") || p=
    if [ -n "$p" ]; then report 1 "photograph scene $n"; else report 0 "photograph scene $n"; fi
    v=$(first_file "$d/verdicts/scene$n.txt" "$d/verdicts/scene$n.json") || v=
    if [ -n "$v" ]; then report 1 "verdict scene $n"; else report 0 "verdict scene $n"; fi
    n=$((n + 1))
done

b=$(first_file "$d/board-info.md" "$d/board-info.txt" "$d/board-info") || b=
if [ -n "$b" ]; then report 1 "board info"; else report 0 "board info (board-info.md)"; fi
s=$(first_file "$d/state-before.txt" "$d/snapshot-before.txt" "$d/before.txt" "$d/state-before") || s=
if [ -n "$s" ]; then report 1 "before snapshot"; else report 0 "before snapshot (state-before.txt)"; fi
if first_file "$d/state-after.txt" "$d/snapshot-after.txt" "$d/after.txt" "$d/state-after" >/dev/null; then
    echo "present: after snapshot (optional)"
else
    echo "note: after snapshot not present yet (optional, taken after the Torizon reboot)"
fi

if [ $missing -eq 0 ]; then
    echo COMPLETE
    exit 0
fi
echo "INCOMPLETE: $missing missing"
exit 1
