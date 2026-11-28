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

expect good 0 clean sh "$script" "$data/good.rs" "$data/good-nodata.rs" "$data/good-other.rs" "$data/good-caps.rs" "$data/good-synthetic.rs"
expect nosource 1 'bad-nosource.rs:1: provenance' sh "$script" "$data/bad-nosource.rs"
expect nodate 1 'bad-nodate.rs:1: provenance' sh "$script" "$data/bad-nodate.rs"
expect split 1 'bad-split.rs:1: provenance' sh "$script" "$data/bad-split.rs"
expect include 1 'bad-include.rs:2: runtime-read' sh "$script" "$data/bad-include.rs"
expect mixed 1 'bad-nodate.rs' sh "$script" "$data/good.rs" "$data/bad-nodate.rs"
# The retired notes directory is no longer a provenance source and its files
# need not exist; the path is built here so no file in the tree names it.
old=docs/obs; old=${old}erved
tmp=$(mktemp -d "${TMPDIR:-/tmp}/cfpt.XXXXXX") || exit 2
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
printf '// NanoPC-T6 data from %s/rk3588-topology.md, observed 2026-11-28.\nconst P: &[u32] = &[33];\n' "$old" >"$tmp/legacy-source.rs"
printf '// bcm2711 fixture, observed 2026-11-28.\nconst T: &str = include_str!(\n    "../../../%s/bcm2711-cm4.md");\n' "$old" >"$tmp/legacy-include.rs"
expect legacy-source 1 'legacy-source.rs:1: provenance' sh "$script" "$tmp/legacy-source.rs"
expect legacy-include 1 'legacy-include.rs:2: runtime-read' sh "$script" "$tmp/legacy-include.rs"
expect badopt 2 usage sh "$script" -x
expect missing 2 'not a file' sh "$script" "$data/nonexistent.rs"

out=$(sh "$script" "$data/good.rs" "$data/bad-nodate.rs" "$data/bad-include.rs")
n=$(printf '%s\n' "$out" | grep -c .)
if [ "$n" -ne 2 ]; then echo "FAIL exact-count: $n lines"; echo "$out"; fail=1; else echo "ok exact-count"; fi

exit "$fail"
