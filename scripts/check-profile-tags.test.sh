#!/bin/sh
# Self-test for check-profile-tags.sh.
# Run: sh scripts/check-profile-tags.test.sh

here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-profile-tags.sh
data=$here/testdata/check-profile-tags
fail=0

expect() { # name want_rc want_output_pattern(grep -c count, or -) cmd...
    name=$1; want=$2; pat=$3; shift 3
    out=$("$@" 2>&1); rc=$?
    if [ "$rc" -ne "$want" ]; then
        echo "FAIL $name: rc $rc, want $want"; echo "$out"; fail=1; return
    fi
    if [ "$pat" != - ] && ! printf '%s\n' "$out" | grep -q -- "$pat"; then
        echo "FAIL $name: output lacks '$pat'"; echo "$out"; fail=1; return
    fi
    echo "ok $name"
}

expect good 0 - sh "$script" "$data/good.rs"
expect notag 1 'bad-notag.rs:3: bullet has no evidence tag' sh "$script" "$data/bad-notag.rs"
expect nodate 1 'bad-nodate.rs:3: observed without a YYYY-MM-DD date' sh "$script" "$data/bad-nodate.rs"
expect multi 1 'bad-nodate.rs:3' sh "$script" "$data/good.rs" "$data/bad-nodate.rs"
expect badopt 2 usage sh "$script" -x
expect missing 2 'not a file' sh "$script" "$data/nonexistent.rs"

# the dated bullet and the tagged bullet must not be reported
out=$(sh "$script" "$data/bad-nodate.rs" "$data/bad-notag.rs")
n=$(printf '%s\n' "$out" | grep -c .)
if [ "$n" -ne 2 ]; then echo "FAIL exact-count: $n lines"; echo "$out"; fail=1; else echo "ok exact-count"; fi

exit "$fail"
