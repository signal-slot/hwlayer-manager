#!/bin/sh
# Self-test of check-am62p-matrix.sh. Fixtures: scripts/testdata/am62p-matrix/.
# Mutations of good.md must fail with their message. The clock is fixed.
set -u
here=$(cd "$(dirname "$0")" && pwd)
chk=$here/check-am62p-matrix.sh
td=$here/testdata/am62p-matrix
tmp=$(mktemp -d "${TMPDIR:-.}/am62pmx.XXXXXX") || exit 2
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
TODAY=2026-12-15
export TODAY
rc=0
bad() {
    echo "FAIL: $*" >&2
    rc=1
}

# run_ ARGS...: sets out, returns the exit status
run_() {
    out=$(sh "$chk" "$@" 2>&1)
}

run_ --matrix "$td/good.md" || bad "good fixture rejected: $out"
run_ --matrix "$td/good.md" --require-observed --observed "$td/observed.md" ||
    bad "good fixture with --require-observed rejected: $out"
run_ --matrix "$td/good.md" --require-observed --observed "$tmp/none.md" &&
    bad "missing observed file accepted"
case $out in *"does not exist"*) ;; *) bad "missing observed file message: $out" ;; esac
run_ --matrix "$td/nocolumn.md" && bad "matrix without AM62P column accepted"
case $out in *"no table with an AM62P column"*) ;; *) bad "nocolumn message: $out" ;; esac
run_ --matrix "$tmp/none.md" && bad "unreadable matrix accepted"
run_ --bogus && bad "bad option accepted"

# case_ NAME SED-EXPR EXPECTED-MESSAGE [extra args]
case_() {
    name=$1 expr=$2 msg=$3
    shift 3
    sed -e "$expr" "$td/good.md" >"$tmp/$name.md"
    run_ --matrix "$tmp/$name.md" "$@" && bad "$name accepted"
    case $out in
    *"$msg"*) ;;
    *) bad "$name: message lacks '$msg': $out" ;;
    esac
}
case_ untagged 's/^| Formats | .*| many |/| Formats | RGB565 | many |/' 'no recognised tag'
case_ empty 's/^| Scaling | not recorded/| Scaling | /' 'empty AM62P cell'
case_ badfmt 's/2026-12-01/1 Dec 2026/' 'malformed or future date'
case_ badmonth 's/2026-12-01/2026-13-01/' 'malformed or future date'
case_ badday 's/2026-12-01/2026-02-30/' 'malformed or future date'
case_ future 's/2026-12-01/2026-12-16/' 'malformed or future date'
case_ nodate 's/observed (2026-12-01)/observed/' 'no (YYYY-MM-DD) date'
case_ r5fcol 's/^| Latch | follows the GO bit (`TP`)/| Latch | follows the GO bit/' 'no recognised tag'

# a leap day is real; "not observed" is no observation
sed -e 's/2026-12-01/2024-02-29/' "$td/good.md" >"$tmp/leap.md"
run_ --matrix "$tmp/leap.md" || bad "leap day rejected: $out"
sed -e 's/observed (2026-12-01)/not observed/' "$td/good.md" >"$tmp/notobs.md"
run_ --matrix "$tmp/notobs.md" || bad "'not observed' rejected: $out"
run_ --matrix "$tmp/notobs.md" --require-observed --observed "$td/observed.md" &&
    bad "--require-observed accepted a matrix with no observed cell"
case $out in *"no AM62P cell is observed"*) ;; *) bad "no-observed message: $out" ;; esac

# observed file without the matrix's date
printf 'Run on 2026-11-30.\n' >"$tmp/otherdate.md"
run_ --matrix "$td/good.md" --require-observed --observed "$tmp/otherdate.md" &&
    bad "observed file with another date accepted"
case $out in *"does not hold the observed date 2026-12-01"*) ;; *) bad "date mismatch message: $out" ;; esac

# today's tree: passes without the flag, fails with it
run_ || bad "current tree rejected: $out"
run_ --require-observed && bad "current tree accepted with --require-observed"

if [ $rc -eq 0 ]; then
    echo "check-am62p-matrix.test: ok"
fi
exit $rc
