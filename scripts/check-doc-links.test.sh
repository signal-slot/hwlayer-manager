#!/bin/sh
# Self-test for check-doc-links.sh.
# Run: sh scripts/check-doc-links.test.sh
# Builds a throwaway git repository under $TMPDIR; never touches the real one.

here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-doc-links.sh
root=$(mktemp -d "${TMPDIR:-.}/dl-test.XXXXXX") || exit 2
case $root in /*) ;; *) root=$(cd "$root" && pwd) ;; esac
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
repo=$root/r

mkdir -p "$repo/docs" "$repo/sub dir"
cd "$repo" || exit 2
git init -q . || exit 2
echo x > code.rs
echo x > "sub dir/a b.md"
cat > docs/target.md <<'T'
# Title

## Hello, World! (v2)

## Repeat

## Repeat
T
cat > README.md <<'T'
# Readme
[ok](docs/target.md) [ok2](docs/target.md#hello-world-v2?x=1) [code](code.rs)
[ext](https://example.com/nope) [mail](mailto:a@b.c) [top](#readme)
[space](<sub dir/a b.md>) [pct](sub%20dir/a%20b.md "title")
[ref][r1]
[r1]: docs/target.md
`[inline code](nothere.md)`
```
[fenced](nothere.md)
```
T
cat > docs/bad.md <<'T'
line one
[gone](missing.md) and [dir ok](../docs)
[ref][r2]

[r2]: ../nowhere/x.md
[badanchor](target.md#nope) [dup](target.md#repeat-1) [self](#zzz)
T
git add . >/dev/null 2>&1

check() { # NAME WANT_RC PATTERN [ARGS]
    name=$1 want=$2 pat=$3
    shift 3
    got=$(sh "$script" "$@" 2>&1)
    rc=$?
    if [ "$rc" != "$want" ]; then
        echo "FAIL $name: exit $rc, want $want"; fail=1
    elif [ -n "$pat" ] && ! printf '%s\n' "$got" | grep -Fq -- "$pat"; then
        echo "FAIL $name: output lacks '$pat'"; printf '%s\n' "$got"; fail=1
    else
        echo "ok   $name"
    fi
}
absent() { # NAME PATTERN [ARGS]
    name=$1 pat=$2
    shift 2
    if sh "$script" "$@" 2>&1 | grep -Fq -- "$pat"; then
        echo "FAIL $name: unexpected '$pat'"; fail=1
    else
        echo "ok   $name"
    fi
}

check missing-inline 1 'docs/bad.md:2: missing target missing.md'
check missing-ref 1 'docs/bad.md:5: missing target ../nowhere/x.md'
absent good-readme-clean 'README.md:'
absent code-and-fence-skipped 'nothere.md'
absent bad-anchor-ignored-by-default 'missing anchor'
check anchor-flag 1 'docs/bad.md:6: missing anchor target.md#nope' --anchors
check anchor-self 1 'docs/bad.md:6: missing anchor #zzz' --anchors
absent anchor-dup-ok 'repeat-1' --anchors
absent anchor-punct-ok 'hello-world-v2' --anchors
absent anchor-readme-ok 'README.md:' --anchors
check bad-option 2 'usage' --bogus

git rm -q -f docs/bad.md
check all-good 0 ''
check all-good-anchors 0 '' --anchors

# Works from a subdirectory too.
echo '[x](nope.md)' > sub\ dir/c.md
git add . >/dev/null 2>&1
cd docs || exit 2
check subdir-cwd 1 'sub dir/c.md:1: missing target nope.md'

[ "$fail" = 0 ] && echo "all tests passed"
exit "$fail"
