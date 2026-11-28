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

# --- error paths (synthetic fixtures: err-*.txt) ---
# run FILE ARGS...: sets rc, out (stdout) and err (stderr) from the script.
run() { python3 -I "$py" "$@" >"$tmp/o" 2>"$tmp/e"; rc=$?; }
has() { grep -qF -- "$2" "$1" || check "$3: missing line '$2'"; }

# 1. missing snapshot directory (path does not exist), and a directory given as file
run "$tmp/no-such-dir/snapshot.txt"
[ $rc -eq 1 ] || check "missing directory: exit $rc, want 1"
has "$tmp/e" "cannot read input" "missing directory"
[ -s "$tmp/o" ] && check "missing directory: stdout should be empty"
run "$fx"
[ $rc -eq 1 ] || check "directory as input: exit $rc, want 1"
has "$tmp/e" "cannot read input" "directory as input"

# 2. truncated / malformed snapshots
run "$fx/err-truncated.txt"   # cut mid-record, device tree section missing
[ $rc -eq 1 ] || check "truncated (no device tree): exit $rc, want 1"
has "$tmp/e" "not an imx95-observe.sh snapshot" "truncated"
run "$fx/err-bad-header.txt"
[ $rc -eq 1 ] || check "bad header: exit $rc, want 1"
has "$tmp/e" "not an imx95-observe.sh snapshot" "bad header"
run /dev/null
[ $rc -eq 1 ] || check "empty input: exit $rc, want 1"
run "$fx/err-truncated-record.txt"   # last record cut: tolerated, missing keys shown as '-'
[ $rc -eq 0 ] || check "truncated record: exit $rc, want 0"
has "$tmp/o" "| remoteproc0 | imx-rproc | - | - |" "truncated record"

# 3. no remoteproc entries
run "$fx/err-no-remoteproc.txt"
[ $rc -eq 0 ] || check "no remoteproc entries: exit $rc, want 0"
has "$tmp/o" "## remoteproc" "no remoteproc"
awk '/^## remoteproc/{f=1;next} f&&NF{print;exit}' "$tmp/o" | grep -qx none \
    || check "no remoteproc: first line under '## remoteproc' should be 'none'"
has "$tmp/o" "| /soc/display-controller@0 | example,dpu | okay |" "no remoteproc"

# 4. unreadable or absent debugfs sections: reported, not a crash
for f in err-unreadable err-no-debugfs; do
    run "$fx/$f.txt"
    [ $rc -eq 0 ] || check "$f: exit $rc, want 0"
    [ "$(grep -cx 'not readable' "$tmp/o")" -eq 2 ] || check "$f: want two 'not readable' lines (clk, genpd)"
    has "$tmp/o" "| remoteproc0 | imx-rproc | offline | example-m7.elf |" "$f"
    grep -q Traceback "$tmp/e" && check "$f: traceback on stderr"
done

[ $fail -eq 0 ] && echo "imx95-observe-summary.test.sh: ok"
exit $fail
