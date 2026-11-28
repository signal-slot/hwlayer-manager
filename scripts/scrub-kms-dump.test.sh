#!/bin/sh
# Self-test for scrub-kms-dump.py. Run: scripts/scrub-kms-dump.test.sh
# Fixtures are made up (documentation ranges) and carry the "secrets-ok" marker.
here=$(cd "$(dirname "$0")" && pwd)
py=$here/scrub-kms-dump.py
data=$here/testdata/scrub-kms-dump
work=$(mktemp -d "${TMPDIR:-.}/scrub-test.XXXXXX") || exit 2
trap 'rm -rf "$work"' EXIT HUP INT TERM
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1" >&2; fail=1; }

# The grep patterns of check-no-board-secrets.sh (ipv4, tty, mac, user@host,
# secret), plus the made-up values of the fixture.
leaks() {
    grep -E '(^|[^0-9.])([0-9]{1,3}\.){3}[0-9]{1,3}($|[^0-9])' "$1" | grep -vE '(0\.0\.0\.0|127\.0\.0\.1)'
    grep -E '/dev/(tty[A-Za-z0-9_]+|serial/by-id)' "$1" | grep -vE '/dev/ttyRPMSG'
    grep -E '(^|[^0-9A-Fa-f:-])(([0-9A-Fa-f]{2}:){5}|([0-9A-Fa-f]{2}-){5})[0-9A-Fa-f]{2}($|[^0-9A-Fa-f:-])' "$1"
    grep -E '[A-Za-z0-9_][A-Za-z0-9._-]*@[A-Za-z][A-Za-z0-9-]*' "$1" | grep -viE 'git@github\.com|noreply|user@host'
    grep -iE '(password|passwd|token|secret|api_key) *= *["'"'"']?[A-Za-z0-9+/_.-]' "$1"
    grep -iE 'testboard1([^0-9]|$)|ABC1234567|0123456789|hunter2|[0-9a-f]{32}' "$1"
}

if [ -n "$(leaks "$data/input.txt")" ]; then ok "input trips the patterns (test is meaningful)"; else bad "input does not trip the patterns"; fi

python3 "$py" --hostname testboard1 "$data/input.txt" >"$work/out" 2>"$work/err"
rc=$?
if [ "$rc" -eq 0 ]; then ok "exit 0"; else bad "exit $rc"; fi
if [ -z "$(leaks "$work/out")" ]; then ok "scrubbed output passes the patterns"; else bad "leak: $(leaks "$work/out")"; fi
if cmp -s "$work/out" "$data/expected.txt"; then ok "matches expected.txt"; else bad "differs from expected.txt"; fi
if grep -q 'replacements' "$work/err"; then ok "summary on stderr"; else bad "no summary"; fi

if python3 "$py" --hostname testboard1 <"$data/input.txt" 2>/dev/null | cmp -s - "$work/out"; then ok "stdin equals file"; else bad "stdin differs"; fi
if python3 "$py" --hostname testboard1 "$work/out" 2>/dev/null | cmp -s - "$work/out"; then ok "idempotent"; else bad "not idempotent"; fi

# Text without identifiers stays byte-identical (CRLF and a missing final newline too).
printf 'plane 33: zpos range 0..=7 = 0\r\nmodifier 0x0800000000000001: XR24\nno newline' >"$work/plain"
if python3 "$py" "$work/plain" 2>/dev/null | cmp -s - "$work/plain"; then ok "clean text byte-identical"; else bad "clean text changed"; fi

# Toradex default host names '<family>-<soc>-<8 digits>' go without --hostname.
# Only made-up digits here.
printf 'root@%s:~# id\n' verdin-imx95-12345678 >"$work/tdx"
cat >>"$work/tdx" <<'EOF'
Nov 28 10:00:00 verdin-imx95-12345678 systemd[1]: Started Weston.
Linux colibri-imx8x-87654321 6.6.0 #1 SMP PREEMPT aarch64 GNU/Linux
VERDIN-IMX95-12345678 and apalis-imx8-00000001.
verdin-imx95
resource verdin-imx95
imx95-12345678
EOF
python3 "$py" "$work/tdx" >"$work/tdx.out" 2>/dev/null
if ! grep -qiE '(verdin|colibri|apalis)-[a-z0-9]+-[0-9]{8}' "$work/tdx.out" &&
    [ "$(grep -c '<host>' "$work/tdx.out")" -ge 3 ]; then ok "Toradex default host names scrubbed"; else bad "Toradex host leak"; fi
if grep -qx 'verdin-imx95' "$work/tdx.out" && grep -qx 'resource verdin-imx95' "$work/tdx.out" &&
    grep -qx 'imx95-12345678' "$work/tdx.out"; then ok "names without family+serial kept"; else bad "over-scrubbed Toradex lookalikes"; fi
if grep -q 'Linux <host> 6.6.0' "$work/tdx.out"; then ok "uname -a scrubbed"; else bad "uname line"; fi

python3 "$py" --bogus >/dev/null 2>&1
if [ $? -eq 2 ]; then ok "bad option exits 2"; else bad "bad option status"; fi
python3 "$py" "$work/missing" >/dev/null 2>&1
if [ $? -eq 2 ]; then ok "missing file exits 2"; else bad "missing file status"; fi

exit $fail
