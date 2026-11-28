#!/bin/sh
# Host test for scripts/imx95-observe-summary.py: synthetic fixture, expected
# output, and the exit codes (0 ok, 1 unparsable, 2 usage).
here=$(cd "$(dirname "$0")" && pwd)
py="$here/imx95-observe-summary.py"
fx="$here/testdata/imx95-observe-summary"
tmp=$(mktemp -d) || exit 1
trap 'rm -rf "$tmp"' EXIT
fail=0
check() { echo "FAIL: $*" >&2; fail=1; }

python3 -I "$py" "$fx/input.txt" >"$tmp/out" || check "fixture exit code"
diff -u "$fx/expected.md" "$tmp/out" || check "output differs from expected.md"
python3 -I "$py" <"$fx/input.txt" | cmp -s - "$fx/expected.md" || check "stdin mode differs"

echo "garbage" | python3 -I "$py" >/dev/null 2>&1
[ $? -eq 1 ] || check "unparsable input should exit 1"
python3 -I "$py" "$tmp/missing" >/dev/null 2>&1
[ $? -eq 1 ] || check "missing file should exit 1"
python3 -I "$py" --bogus >/dev/null 2>&1
[ $? -eq 2 ] || check "bad option should exit 2"
python3 -I "$py" a b >/dev/null 2>&1
[ $? -eq 2 ] || check "two arguments should exit 2"

[ $fail -eq 0 ] && echo "imx95-observe-summary.test.sh: ok"
exit $fail
