#!/bin/sh
# Self-test for sweep-to-fixture.py. Fixtures are synthetic.
set -u
cd "$(dirname "$0")" || exit 2
D=testdata/sweep-to-fixture
T=$(mktemp -d "${TMPDIR:-.}/s2f.XXXXXX") || exit 2
trap 'rm -rf "$T"' EXIT
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
run() { python3 -I sweep-to-fixture.py "$@"; }

run --source scripts/testdata/x.txt --date 2000-01-01 $D/scale.txt $D/sweep.txt >"$T/out.rs" 2>"$T/err"
[ $? -eq 0 ] && ok "exit 0" || bad "exit 0"
has() { grep -qF -- "$1" "$T/out.rs" && ok "$2" || bad "$2"; }
has "// source: scripts/testdata/x.txt, observed 2000-01-01" "provenance comment"
[ "$(head -1 "$T/out.rs")" = "// source: scripts/testdata/x.txt, observed 2000-01-01" ] && ok "comment first" || bad "comment first"
has "const SCALE_RESULTS: &[ScaleResult]" "scale const"
has "const COMBOS: &[Combo]" "combo const"
has "const MODIFIERS: &[ModifierResult]" "modifier const"
has "const REPLACES: &[Replace]" "replace const"
has "planes: &[31, 42, 43]," "plane list"
has "modifier: 0x0100000000000001," "modifier literal"
has "listed: false," "listed bool"
has 'test_only: "not reached",' "spaced value"
has 'max_upscale_h: "4.0000",' "ratio kept as text"
if grep -q "COMBO_MAX\|VERDICT" "$T/out.rs"; then bad "ignored lines"; else ok "ignored lines"; fi

# stdin input gives the same text
run --source scripts/testdata/x.txt --date 2000-01-01 <$D/sweep.txt >"$T/stdin.rs" && grep -qF "COMBOS" "$T/stdin.rs" && ok stdin || bad stdin

if command -v rustfmt >/dev/null 2>&1; then
    rustfmt --check --edition 2021 "$T/out.rs" >/dev/null 2>&1 && ok "rustfmt --check" || bad "rustfmt --check"
else
    echo "skip: rustfmt not found"
fi

# parse errors: exit 1, every bad line named
run --source s --date 2000-01-01 $D/bad.txt >/dev/null 2>"$T/err"
[ $? -eq 1 ] && ok "exit 1" || bad "exit 1"
[ "$(grep -c "bad.txt:" "$T/err")" -eq 3 ] && ok "names each bad line" || bad "names each bad line"
grep -qF "bad.txt:2:" "$T/err" && ok "line number" || bad "line number"

# usage errors: exit 2
run --date 2000-01-01 $D/sweep.txt >/dev/null 2>&1
[ $? -eq 2 ] && ok "missing --source" || bad "missing --source"
run --source s $D/sweep.txt >/dev/null 2>&1
[ $? -eq 2 ] && ok "missing --date" || bad "missing --date"
run --source s --date 01-01-2000 $D/sweep.txt >/dev/null 2>&1
[ $? -eq 2 ] && ok "bad date" || bad "bad date"
run --source s --date 2000-01-01 --bogus >/dev/null 2>&1
[ $? -eq 2 ] && ok "unknown option" || bad "unknown option"
run --source s --date 2000-01-01 /nonexistent >/dev/null 2>&1
[ $? -eq 2 ] && ok "unreadable file" || bad "unreadable file"
exit $fail
