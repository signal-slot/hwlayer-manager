#!/bin/sh
# Test for check-wp5-claims.sh against scripts/testdata/check-wp5-claims/.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
t=$here/testdata/check-wp5-claims
s=$here/check-wp5-claims.sh
fail=0
ok() { echo "ok: $1"; }
ng() { echo "FAIL: $1" >&2; fail=1; }

if sh "$s" "$t/pass-paragraph-tag.md" "$t/pass-plain.md" >/dev/null; then ok pass; else ng pass; fi
if out=$(sh "$s" "$t/fail-untagged.md" 2>&1); then
    ng "fail case exited 0"
else
    n=$(printf '%s\n' "$out" | grep -c 'untagged claim' || true)
    if [ "$n" = 3 ]; then ok "fail case flags 3 lines"; else ng "expected 3 findings, got $n"; fi
fi
rc=0
sh "$s" "$t/missing.md" >/dev/null 2>&1 || rc=$?
if [ "$rc" = 2 ]; then ok "missing file exits 2"; else ng "missing file exit $rc"; fi
exit $fail
