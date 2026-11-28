#!/bin/sh
# Self-test for check-m3-acceptance.sh (always run with --quick; C1 is SKIP).
# Run: sh scripts/check-m3-acceptance.test.sh
# Builds fake trees under $TMPDIR (never /tmp): a good one, then one broken
# copy per criterion. C6 uses stub scripts inside the fake tree.

here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-m3-acceptance.sh
base=${TMPDIR:-}
if [ -z "$base" ]; then
    echo "TMPDIR must be set (not /tmp)" >&2
    exit 2
fi
work=$(mktemp -d "$base/m3-accept-test.XXXXXX") || exit 2
trap 'rm -rf "$work"' EXIT HUP INT TERM
fails=0

# mk DIR: a fake tree that passes C2-C6
mk() {
    d=$1
    mkdir -p "$d/docs/observed" "$d/docs/photos" "$d/scripts"
    : >"$d/docs/photos/s1.jpg"
    : >"$d/docs/extra.md"
    {
        echo "# M3 report"
        i=1
        while [ "$i" -le 8 ]; do
            echo "SCENE $i x"
            i=$((i + 1))
        done
        echo "RESULT PASS"
        echo "![s1](photos/s1.jpg) [extra](extra.md#a) [web](https://example.invalid/x)"
        echo "## Restore"
        echo "Torizon was brought back and confirmed up over ssh."
    } >"$d/docs/m3-am62p-report.md"
    echo "# observed" >"$d/docs/observed/am62p.md"
    echo "AM62P observed (2026-11-28)" >"$d/docs/am62p-imx95-matrix.md"
    echo "m3-am62p-report is done" >"$d/docs/sow-traceability.md"
    echo "see m3-am62p-report" >"$d/docs/open-issues.md"
    for s in check-no-board-secrets.sh check-no-farm-ids.sh check-doc-links.sh; do
        echo "exit 0" >"$d/scripts/$s"
    done
}

# expect NAME DIR WANT_EXIT PATTERN...
expect() {
    name=$1
    dir=$2
    want=$3
    shift 3
    out=$(sh "$script" --quick --root "$dir" 2>&1)
    got=$?
    ok=1
    [ "$got" -eq "$want" ] || ok=0
    for pat in "$@"; do
        printf '%s\n' "$out" | grep -q -- "$pat" || ok=0
    done
    if [ "$ok" = 1 ]; then
        echo "ok   $name"
    else
        echo "FAIL $name: want exit $want and [$*], got $got:"
        printf '%s\n' "$out"
        fails=$((fails + 1))
    fi
}

# variant NAME MUTATION-COMMAND: copy of good tree, mutate, expect FAIL of Cn
good=$work/good
mk "$good"
expect good "$good" 0 '^C1 SKIP' '^C2 PASS' '^C3 PASS' '^C4 PASS' '^C5 PASS' '^C6 PASS' '^M3-ACCEPTANCE PASS$'

v=$work/v
bad() { # NAME CN mutation...
    name=$1
    cn=$2
    shift 2
    rm -rf "$v"
    cp -R "$good" "$v"
    (cd "$v" && "$@")
    expect "$name" "$v" 1 "^$cn FAIL" "^M3-ACCEPTANCE FAIL:.*$cn"
}

bad c2-no-report C2 rm docs/m3-am62p-report.md
bad c2-few-scenes C2 sed -i '/^SCENE 8/d' docs/m3-am62p-report.md
bad c2-no-result C2 sed -i '/RESULT /d' docs/m3-am62p-report.md
bad c2-broken-link C2 sed -i 's|photos/s1.jpg|photos/gone.jpg|' docs/m3-am62p-report.md
bad c3-no-section C3 sed -i '/^## Restore/d' docs/m3-am62p-report.md
bad c3-no-ssh C3 sed -i 's/ssh/serial/' docs/m3-am62p-report.md
bad c4-no-observed C4 rm docs/observed/am62p.md
bad c4-no-matrix-col C4 sh -c 'echo nothing >docs/am62p-imx95-matrix.md'
bad c5-old-text C5 sh -c 'echo "No document records an AM62P board observation" >>docs/sow-traceability.md'
bad c5-no-open-issues-ref C5 sh -c 'echo none >docs/open-issues.md'
bad c5-no-trace-ref C5 sh -c 'echo none >docs/sow-traceability.md'
bad c6-secrets C6 sh -c 'echo "exit 1" >scripts/check-no-board-secrets.sh'
bad c6-farm-ids C6 sh -c 'echo "exit 1" >scripts/check-no-farm-ids.sh'
bad c6-doc-links C6 sh -c 'echo "exit 1" >scripts/check-doc-links.sh'

# empty tree: every criterion except C1 (SKIP) fails; usage errors exit 2
mkdir "$work/empty"
expect empty "$work/empty" 1 '^C1 SKIP' '^C2 FAIL' '^C3 FAIL' '^C4 FAIL' '^C5 FAIL' '^M3-ACCEPTANCE FAIL: C2 C3 C4 C5'
sh "$script" --bogus >/dev/null 2>&1
if [ $? -eq 2 ]; then echo "ok   usage-exit-2"; else echo "FAIL usage-exit-2"; fails=$((fails + 1)); fi

if [ "$fails" -eq 0 ]; then
    echo "all passed"
    exit 0
fi
echo "$fails failed"
exit 1
