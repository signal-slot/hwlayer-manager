#!/bin/sh
# Self-test for check-trace-log.sh. Run: sh scripts/check-trace-log.test.sh
# Fixtures are synthetic and live in scripts/testdata/check-trace-log/.
here=$(cd "$(dirname "$0")" && pwd)
s=$here/check-trace-log.sh
d=$here/testdata/check-trace-log
fail=0
t() { # name expected-rc expected-stdout-regex args...
    name=$1; erc=$2; re=$3; shift 3
    out=$(sh "$s" "$@" 2>/dev/null); rc=$?
    if [ "$rc" != "$erc" ] || ! printf '%s\n' "$out" | grep -Eq "$re"; then
        echo "FAIL $name: rc=$rc out=$out"; fail=1
    fi
}
t host-ok 0 '^ok$' "$d/host-ok.log"
t board-ok 0 '^ok$' "$d/board-ok.log"
t unmarked 0 '^ok$' "$d/unmarked-warn.log"
t nomark 1 'host-nomark.log:2: .*not a hardware result' "$d/host-nomark.log"
t nodate 1 'nodate.log:1: .*date' "$d/nodate.log"
t conflict 1 'date-conflict.log:3: .*differs' "$d/date-conflict.log"
t version 1 'bad-version.log:2: .*version' "$d/bad-version.log"
t shape 1 'bad-date-shape.log:1: .*date' "$d/bad-date-shape.log"
t empty 1 'empty.log:1: .*CETRACE' "$d/empty.log"
t mixed 1 '^ok$' "$d/host-ok.log" "$d/nodate.log"
for f in "$here"/testdata/trace-to-metrics/*ok*.log; do t "m:$f" 0 '^ok$' "$f"; done
sh "$s" >/dev/null 2>&1; [ $? = 2 ] || { echo "FAIL usage"; fail=1; }
sh "$s" "$d/missing.log" >/dev/null 2>&1; [ $? = 2 ] || { echo "FAIL missing"; fail=1; }
sh "$s" "$d/unmarked-warn.log" 2>&1 >/dev/null | grep -q '^warn' || { echo "FAIL warn"; fail=1; }
[ $fail = 0 ] && echo "check-trace-log.test.sh: all passed"
exit $fail
