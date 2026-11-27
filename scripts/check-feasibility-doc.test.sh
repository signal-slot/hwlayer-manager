#!/bin/sh
# Self-test for check-feasibility-doc.sh.
# Run: bash scripts/check-feasibility-doc.test.sh
# Uses synthetic files under scripts/testdata/check-feasibility-doc/.

here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-feasibility-doc.sh
d=$here/testdata/check-feasibility-doc
fail=0

check() { # NAME WANT_RC PATTERN ARGS...
    name=$1 want=$2 pat=$3
    shift 3
    got=$(sh "$script" "$@" 2>&1)
    rc=$?
    if [ "$rc" != "$want" ]; then
        echo "FAIL $name: exit $rc, want $want"; printf '%s\n' "$got"; fail=1
    elif [ -n "$pat" ] && ! printf '%s\n' "$got" | grep -Fq -- "$pat"; then
        echo "FAIL $name: output lacks '$pat'"; printf '%s\n' "$got"; fail=1
    else
        echo "ok   $name"
    fi
}
absent() { # NAME PATTERN ARGS...
    name=$1 pat=$2
    shift 2
    if sh "$script" "$@" 2>&1 | grep -Fq -- "$pat"; then
        echo "FAIL $name: unexpected '$pat'"; fail=1
    else
        echo "ok   $name"
    fi
}

check good 0 ok "$d/good.md"
check upper-case-tags 0 ok "$d/upper.md"
check missing-tag 1 'fail 0: no "inferred"' "$d/missing-inferred.md"
check no-date 1 'fail 0: no YYYY-MM-DD' "$d/nodate.md"
check unpinned-1 1 'fail 5:' "$d/unpinned.md"
check unpinned-2 1 'fail 6:' "$d/unpinned.md"
check missing-file 1 'fail 0: file not found' "$d/absent.md"
absent pinned-not-flagged 'fail 3:' "$d/good.md"
check bad-option 2 '' --bogus
check two-args 2 '' "$d/good.md" "$d/upper.md"

[ "$fail" = 0 ] && echo "all tests passed"
exit "$fail"
