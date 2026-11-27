#!/bin/sh
# Self-test for profile-check-expect.py.
# Run: bash scripts/profile-check-expect.test.sh

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/profile-check-expect.py
data=$here/testdata/profile-check-expect
fail=0

check() { # NAME WANT_RC PATTERN ARGS...
    name=$1 want=$2 pat=$3
    shift 3
    got=$(python3 -I "$tool" "$@" 2>&1)
    rc=$?
    if [ "$rc" != "$want" ]; then
        echo "FAIL $name: exit $rc, want $want"; printf '%s\n' "$got"; fail=1
    elif [ -n "$pat" ] && ! printf '%s\n' "$got" | grep -Fq -- "$pat"; then
        echo "FAIL $name: output lacks '$pat'"; printf '%s\n' "$got"; fail=1
    else
        echo "ok   $name"
    fi
}

check all-agree 0 "VERDICT: PASS" "$data/all-agree.txt"
check all-agree-counts 0 "profile=rk3588-vop2 agree=3 disagree=0" "$data/all-agree.txt"
check one-disagree 1 "VERDICT: FAIL" "$data/one-disagree.txt"
check disagree-shown 1 "note=tiled, rot 90" "$data/one-disagree.txt"
check no-lines 1 "no AGREE/DISAGREE lines" "$data/no-lines.txt"
check mixed-log 0 "profile=vc4-hvs5 agree=2 disagree=0" "$data/mixed.log"
check mixed-pass 0 "VERDICT: PASS" "$data/mixed.log"
check mixed-markdown 0 "| vc4-hvs5 | 2 | 0 |" --markdown "$data/mixed.log"
check missing-profile 1 "no line for profile vc4-hvs6" --expect-profile vc4-hvs6 "$data/missing-profile.txt"
check profile-present 0 "VERDICT: PASS" --expect-profile vc4-hvs5 "$data/missing-profile.txt"
check allow-disagree 0 "VERDICT: PASS" --allow-disagree 1 "$data/one-disagree.txt"
check allow-too-few 1 "2 DISAGREE (allowed 1)" --allow-disagree 1 "$data/two-disagree.txt"
check markdown 0 "| rk3588-vop2 | 3 | 0 |" --markdown "$data/all-agree.txt"
check markdown-disagree 1 "| vc4-hvs6 | sand128 | accept | errno=22 |" --markdown "$data/one-disagree.txt"
check stdin 0 "VERDICT: PASS" - < "$data/all-agree.txt"
check missing-file 2 "No such file" "$data/absent.txt"
check bad-option 2 "" --bogus

[ "$fail" = 0 ] && echo "all passed"
exit "$fail"
