#!/bin/sh
# Self-test for check-m1-report.sh. Run: sh scripts/check-m1-report.test.sh
# Fixtures are synthetic and live in scripts/testdata/check-m1-report/.
here=$(cd "$(dirname "$0")" && pwd)
s=$here/check-m1-report.sh
d=$here/testdata/check-m1-report
fail=0
t() { # name expected-rc expected-output-regex file (empty regex: no output)
    name=$1; erc=$2; re=$3; f=$4
    out=$(sh "$s" "$f" 2>&1); rc=$?
    if [ "$rc" != "$erc" ]; then
        echo "FAIL $name: rc=$rc out=$out"; fail=1
    elif [ -n "$re" ]; then
        if ! printf '%s\n' "$out" | grep -Eq -- "$re"; then
            echo "FAIL $name: out=$out"; fail=1
        fi
    elif [ -n "$out" ]; then
        echo "FAIL $name: unexpected output: $out"; fail=1
    fi
}
t good 0 '' "$d/good.md"
t blocked-needs-no-picture 0 '' "$d/blocked-ok.md"
t missing-cond4 1 "verdict line 'Condition 4" "$d/missing-cond4.md"
t empty-field 1 "field 'latency' missing" "$d/empty-field.md"
t no-date 1 'no date in YYYY-MM-DD' "$d/nodate.md"
t bad-overall 1 "missing 'Overall M1 verdict" "$d/bad-overall.md"
t cond3-not-passed 1 "condition 3 is 'failed'" "$d/overall-cond3-failed.md"
t no-picture 1 'cites no picture path that exists' "$d/no-picture.md"
t no-planes 1 'names no plane IDs' "$d/no-planes.md"
t missing-report 2 'report not found' "$d/absent.md"
# the checker's built-in self-test still passes
if ! sh "$s" --self-test >/dev/null 2>&1; then
    echo "FAIL --self-test"; fail=1
fi
if [ $fail = 0 ]; then echo "check-m1-report.test.sh: all passed"; fi
exit $fail
