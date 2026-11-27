#!/bin/sh
# Test scripts/check-commit-subjects.sh on a throwaway git repository.
# Usage: sh scripts/check-commit-subjects.test.sh
set -u
here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-commit-subjects.sh
tmp=$(mktemp -d "${TMPDIR:-/tmp}/commit-subjects-test.XXXXXX") || exit 1
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
fails=0
n=0

expect() { # name status(0 = pass)
    if [ "$2" = 0 ]; then echo "pass: $1"; else echo "FAIL: $1"; fails=$((fails + 1)); fi
}

cd "$tmp" || exit 1
git init -q . || exit 1
git config user.email t@example.invalid
git config user.name test
git config commit.gpgsign false
echo 0 >seed
git add seed
git commit -q -m "seed"
git branch -M main
git tag base

c() { # c subject body file -- commit a change to file
    n=$((n + 1))
    mkdir -p "$(dirname "$3")"
    echo "$n" >>"$3"
    git add "$3"
    if [ -n "$2" ]; then git commit -q -m "$1" -m "$2"; else git commit -q -m "$1"; fi
}

run() { out=$(sh "$script" "$@" 2>&1); rc=$?; }

c "engine: add plane assignment" "Why this approach." crates/a.rs
c "Plain sentence with a period." "" notes.txt
c "docs: no body needed outside crates" "" docs/x.md
run base
expect "good commits exit 0" "$rc"
expect "three ok lines" "$([ "$(printf '%s\n' "$out" | grep -c '^ok ')" = 3 ] && echo 0 || echo 1)"

bad() { # name subject body file expected-fragment
    git tag -f pre >/dev/null
    c "$2" "$3" "$4"
    run pre
    expect "$1" "$([ $rc = 1 ] && printf '%s\n' "$out" | grep -q "^fail .*$5" && echo 0 || echo 1)"
}
bad "prefixed trailing period" "drm: fix thing." "b" n1 "period"
long="engine: 0123456789012345678901234567890123456789012345678901234567890123456789"
bad "subject over 72" "$long" "b" n2 "72"
bad "WIP" "engine: WIP scanout" "b" n3 "WIP"
bad "fixup!" "fixup! engine: x" "b" n4 "fixup!"
bad "crates diff without body" "ffi: change" "" crates/b.rs "no body"

git tag -f pre >/dev/null
echo e >e
git add e
git commit -q --allow-empty-message -m "" 2>/dev/null
run pre
expect "empty subject" "$([ $rc = 1 ] && printf '%s\n' "$out" | grep -q 'empty subject' && echo 0 || echo 1)"

run no-such-ref
expect "unresolvable base exits 2" "$([ $rc = 2 ] && echo 0 || echo 1)"

if [ "$fails" = 0 ]; then echo "all passed"; exit 0; fi
echo "$fails failed"; exit 1
