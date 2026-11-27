#!/bin/sh
# Self-test for check-fixture-provenance.sh.
# Run: sh scripts/check-fixture-provenance.test.sh

here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-fixture-provenance.sh
data=$here/testdata/check-fixture-provenance
fail=0

expect() { # name want_rc want_output_pattern(or -) cmd...
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

expect good 0 clean sh "$script" "$data/good.rs" "$data/good-nodata.rs" "$data/good-other.rs"
expect nosource 1 'bad-nosource.rs:1: provenance' sh "$script" "$data/bad-nosource.rs"
expect nodate 1 'bad-nodate.rs:1: provenance' sh "$script" "$data/bad-nodate.rs"
expect split 1 'bad-split.rs:1: provenance' sh "$script" "$data/bad-split.rs"
expect include 1 'bad-include.rs:2: runtime-read' sh "$script" "$data/bad-include.rs"
expect mixed 1 'bad-nodate.rs' sh "$script" "$data/good.rs" "$data/bad-nodate.rs"
expect badopt 2 usage sh "$script" -x
expect missing 2 'not a file' sh "$script" "$data/nonexistent.rs"

out=$(sh "$script" "$data/good.rs" "$data/bad-nodate.rs" "$data/bad-include.rs")
n=$(printf '%s\n' "$out" | grep -c .)
if [ "$n" -ne 2 ]; then echo "FAIL exact-count: $n lines"; echo "$out"; fail=1; else echo "ok exact-count"; fi

exit "$fail"
