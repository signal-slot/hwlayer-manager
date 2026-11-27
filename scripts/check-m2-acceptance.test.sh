#!/bin/bash
# Self-test for check-m2-acceptance.sh.
# Run: bash scripts/check-m2-acceptance.test.sh
# Builds throwaway git repos under $TMPDIR and points the script at them with
# M2_ACCEPT_ROOT. Exercises PASS and FAIL for c1, c2, c5 and c7, and the SKIP
# for a missing base ref.

here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-m2-acceptance.sh
tmp=$(mktemp -d "${TMPDIR:-/tmp}/m2acc.XXXXXX") || exit 2
trap 'rm -rf "$tmp"' EXIT
fail=0

# mkrepo DIR: a repo whose first commit satisfies c1-c5 and has the
# platform-capabilities sections; tag "base" marks it.
mkrepo() {
    d=$1
    mkdir -p "$d/docs" "$d/tests/board/imx95_m7/src"
    git init -q "$d"
    (
        cd "$d" || exit 1
        git config user.email t@example.invalid
        git config user.name t
        cat >docs/imx95-zephyr-feasibility.md <<'DOC'
# Feasibility
Observed 2026-11-25: remoteproc lists one core.
Read from source: upstream tree.
Inferred: the rest.
DOC
        cat >docs/platform-capabilities.md <<'DOC'
# Platform capabilities

## i.MX95, Linux

linux row

## i.MX95, Zephyr

old zephyr row

## TI AM62P

am62p row
DOC
        : >tests/board/imx95_m7/CMakeLists.txt
        : >tests/board/imx95_m7/prj.conf
        echo 'int main(void){return 0;}' >tests/board/imx95_m7/src/main.c
        echo 'See SoW.md and docs/imx95-zephyr-feasibility.md.' >README.md
        echo 'Use #![forbid(unsafe_code)]; keep composition_engine.h in step.' >CONTRIBUTING.md
        git add -A
        git commit -q -m base
        git tag base
        # c2 pointer, inside the Zephyr section only
        sed -i 's/^old zephyr row$/see docs\/imx95-zephyr-feasibility.md/' docs/platform-capabilities.md
        git commit -q -am "pointer"
    )
}

run() { # DIR [ARGS...]: output of the script on stdout
    d=$1
    shift
    M2_ACCEPT_ROOT=$d bash "$script" "$@" 2>&1
}

expect() { # NAME OUTPUT LINE-PREFIX
    if printf '%s\n' "$2" | grep -q -- "^$3"; then
        echo "ok   $1"
    else
        echo "FAIL $1: no line starting '$3'"
        printf '%s\n' "$2"
        fail=1
    fi
}

r=$tmp/good
mkrepo "$r"
out=$(run "$r" base)
for c in c1 c2 c3 c4 c5 c7; do expect "good $c" "$out" "PASS $c"; done
expect "good c6 skipped" "$out" "SKIP c6"
expect "good c8 skipped" "$out" "SKIP c8: board-side, see the board item reports"
M2_ACCEPT_ROOT=$r bash "$script" base >/dev/null 2>&1 && echo "ok   good exit 0" || { echo "FAIL good exit"; fail=1; }

# missing / HEAD base -> SKIP c2 and c7
out=$(run "$r" no-such-ref)
expect "missing base c2" "$out" "SKIP c2: base ref 'no-such-ref' does not resolve"
expect "missing base c7" "$out" "SKIP c7: base ref 'no-such-ref' does not resolve"
out=$(run "$r" HEAD)
expect "base==HEAD c7" "$out" "SKIP c7: base ref 'HEAD' equals HEAD"

# c1 fail: drop the 'inferred' tag
r=$tmp/c1
mkrepo "$r"
(cd "$r" && sed -i '/Inferred/d' docs/imx95-zephyr-feasibility.md && git commit -q -am x)
out=$(run "$r" base)
expect "c1 fail" "$out" "FAIL c1: doc lacks 'inferred'"
run "$r" base >/dev/null && { echo "FAIL c1 exit"; fail=1; } || echo "ok   c1 exit 1"

# c2 fail: a change outside the Zephyr section
r=$tmp/c2
mkrepo "$r"
(cd "$r" && sed -i 's/^am62p row$/am62p row changed/' docs/platform-capabilities.md && git commit -q -am x)
out=$(run "$r" base)
expect "c2 fail outside" "$out" "FAIL c2: changes outside"
# c2 fail: no pointer
r=$tmp/c2b
mkrepo "$r"
(cd "$r" && sed -i 's/imx95-zephyr-feasibility/other/' docs/platform-capabilities.md && git commit -q -am x)
out=$(run "$r" base)
expect "c2 fail no pointer" "$out" "FAIL c2: .* does not mention"

# c5 fail: README lacks SoW.md
r=$tmp/c5
mkrepo "$r"
(cd "$r" && echo 'imx95-zephyr-feasibility.md only' >README.md && git commit -q -am x)
out=$(run "$r" base)
expect "c5 fail" "$out" "FAIL c5: missing README:SoW.md"

# c7 fail: an address-like string added under a scoped path
r=$tmp/c7
mkrepo "$r"
(cd "$r" && echo 'board at 10.20.30.40' >>README.md && git commit -q -am x)
out=$(run "$r" base)
expect "c7 fail" "$out" "FAIL c7:"
# c7 fail: tty path in tests/board
r=$tmp/c7b
mkrepo "$r"
(cd "$r" && echo '/dev/ttyUSB0' >tests/board/imx95_m7/src/x.txt && git add -A && git commit -q -m x)
out=$(run "$r" base)
expect "c7 fail tty" "$out" "FAIL c7:"

exit $fail
