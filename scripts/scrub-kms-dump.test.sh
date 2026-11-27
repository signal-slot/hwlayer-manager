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

python3 "$py" --bogus >/dev/null 2>&1
if [ $? -eq 2 ]; then ok "bad option exits 2"; else bad "bad option status"; fi
python3 "$py" "$work/missing" >/dev/null 2>&1
if [ $? -eq 2 ]; then ok "missing file exits 2"; else bad "missing file status"; fi

exit $fail
