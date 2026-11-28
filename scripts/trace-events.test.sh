#!/bin/sh
# Self-test for trace-events.sh. Run: sh scripts/trace-events.test.sh
# Fixtures are generated into a temporary directory under $TMPDIR.
here=$(cd "$(dirname "$0")" && pwd)
s=$here/trace-events.sh
base=${TMPDIR:-}
[ -n "$base" ] && [ -d "$base" ] || { echo "trace-events.test.sh: set TMPDIR to a directory" >&2; exit 2; }
d=$(mktemp -d "$base/trace-events.XXXXXX") || exit 2
trap 'rm -rf "$d"' EXIT INT TERM
fail=0

ev() { printf 'CETRACE v=1 ev=%s seq=1 t=1 at=1\n' "$1"; }
{ echo '# date: 2026-11-28'; for k in hdr validate commit latched released dropped check; do ev $k; done; } >"$d/all.trace"
{ for k in validate commit commit latched; do ev $k; done; } >"$d/some.trace"
printf '# only comments\nnot a trace line\n' >"$d/none.trace"
for k in validate commit latched released dropped; do ev $k; done | sed 's/$/\r/' >"$d/crlf.trace"

t() { # name expected-rc expected-output-regex args...
    name=$1; erc=$2; re=$3; shift 3
    out=$(sh "$s" "$@" 2>&1); rc=$?
    if [ "$rc" != "$erc" ] || ! printf '%s\n' "$out" | grep -Eq "$re"; then
        echo "FAIL $name: rc=$rc out=$out"; fail=1
    fi
}
t all 0 "all.trace: validate=1 commit=1 latched=1 released=1 dropped=1 other=2$" "$d/all.trace"
t all-seen 0 '^SEEN validate,commit,latched,released,dropped$' "$d/all.trace"
t all-none 0 '^MISSING none$' "$d/all.trace"
t some-counts 0 "some.trace: validate=1 commit=2 latched=1 released=0 dropped=0 other=0$" "$d/some.trace"
t some-missing 0 '^MISSING released,dropped$' "$d/some.trace"
t req-ok 0 '^MISSING' --require validate,commit "$d/some.trace"
t req-missing 1 '^MISSING released,dropped$' --require released "$d/some.trace"
t req-union 0 '^MISSING none$' --require dropped "$d/some.trace" "$d/all.trace"
t none 0 "none.trace: validate=0 commit=0 latched=0 released=0 dropped=0 other=0$" "$d/none.trace"
t none-seen 0 '^SEEN none$' "$d/none.trace"
t crlf 0 "crlf.trace: validate=1 commit=1 latched=1 released=1 dropped=1 other=0$" "$d/crlf.trace"
t crlf-none 0 '^MISSING none$' "$d/crlf.trace"
t usage 2 'usage'
t usage-opt 2 'usage' --bogus "$d/all.trace"
t usage-req 2 'usage' --require
t usage-kind 2 'unknown kind' --require nosuch "$d/all.trace"
t unreadable 2 'cannot read' "$d/missing.trace"
[ $fail = 0 ] && echo "trace-events.test.sh: all passed"
exit $fail
