#!/bin/sh
# Self-test for scenario-report.py.
# Run: sh scripts/scenario-report.test.sh

here=$(cd "$(dirname "$0")" && pwd)
rep=$here/scenario-report.py
chk=$here/check-board-results.sh
T=$(mktemp -d) || exit 2
trap 'rm -rf "$T"' EXIT
fail=0
ok() { echo "ok   $1"; }
bad() { echo "FAIL $1"; fail=1; }
# t DESC CMD...: pass when CMD succeeds.
t() { d=$1; shift; if "$@"; then ok "$d"; else bad "$d"; fi; }

# Redaction samples are assembled from fragments so that the secrets
# scanner does not see literal addresses, logins or tty paths here.
ip1=192.0.2.$((3 + 4))
ip2=198.51.100.$((1 + 1))
at=@
login=root${at}board$((3 + 4))
tty=/dev/tty"USB0"
host=nanopc.local
# Mixed log: PASS, FAIL, NOT-RUN, BLOCKED; A6 is missing. The reasons carry
# an address, a host name, a login and a tty that must not survive.
cat > "$T/run.log" <<LOG
host time: 2026-11-28T10:00:00Z
A1 startup PASS
A2 steady-state FAIL
2026-11-28T10:05:00Z A3 stop-restart PASS
A4 lease-loss NOT-RUN
A5 modeset BLOCKED
SCENARIO steady-state FAIL
exit=0
failed: dropped=2 exceeds 0 (seen on $ip1 and $host)
SCENARIO modeset BLOCKED
BLOCKED: harness has no modeset scenario | 3 modes; ssh $login $tty
LOG
cat > "$T/pictures.log" <<'LOG'
A1 hold-start 2026-11-28T10:00:25Z /srv/kvm.example.org/A1-hold-start-1.png PASS
A3 hold-end 2026-11-28T10:05:30Z - capture failed (status 1)
LOG
o=$T/out.md
python3 -I "$rep" --board test-board --disclaimer --pictures "$T/pictures.log" \
	"$T/run.log" > "$o" 2> "$T/err"
[ $? -eq 3 ] && ok "mixed log: rc 3 (time missing)" || bad "mixed log rc"

t A1 grep -q '^| A1 | PASS | 2026-11-28T10:00:00Z | observed |' "$o"
t A2 grep -q '^| A2 | FAIL | 2026-11-28T10:00:00Z | observed |.*dropped=2' "$o"
t A3 grep -q '^| A3 | PASS | 2026-11-28T10:05:00Z |.*capture failed' "$o"
t "A4 not run is SKIPPED" grep -q '^| A4 | SKIPPED | .*not run' "$o"
t A5 grep -q '^| A5 | BLOCKED | .*no modeset scenario / 3 modes' "$o"
t "missing row: BLOCKED, empty time, not in log" \
	grep -q '^| A6 | BLOCKED |  | observed | not in log; host time missing' "$o"
t "picture verdict copied" grep -q '^| A1 .*picture hold-start PASS' "$o"
t "missing time flagged on stderr" grep -q 'no host time for A6' "$T/err"
t "six rows" test "$(grep -c '^| A[1-6] |' "$o")" -eq 6
t "every row observed" test "$(grep -c '| observed |' "$o")" -eq 6

# Address stripping.
if grep -Eq '192\.0\.2\.7|nanopc\.local|board7|root[@]|tty[U]SB|kvm\.example' "$o"; then
	bad "address, host, login or tty leaked"; cat "$o"
else ok "address, host, login, tty stripped"; fi
t "redaction marker present" grep -q '<redacted>' "$o"

# The only checker finding on the mixed output is A6's empty time.
sh "$chk" "$o" > "$T/chk" 2>&1
if [ "$(grep -c . "$T/chk")" -eq 1 ] && grep -q 'row A6: host time' "$T/chk"
then ok "checker flags only the empty time"
else bad "checker output"; cat "$T/chk"; fi

# Full log with times: output passes the checker.
cat > "$T/full.log" <<'LOG'
2026-11-28T11:00:00Z A1 startup PASS
2026-11-28T11:01:00Z A2 steady-state PASS
2026-11-28T11:02:00Z A3 stop-restart FAIL
2026-11-28T11:03:00Z A4 lease-loss PASS
2026-11-28T11:04:00Z A5 modeset BLOCKED
2026-11-28T11:05:00Z A6 recovery BLOCKED
LOG
python3 -I "$rep" --board nanopc-t6 --disclaimer "$T/full.log" > "$T/full.md"
[ $? -eq 0 ] && ok "full log: rc 0" || bad "full log: rc 0"
t "full output passes check-board-results" sh "$chk" "$T/full.md"

# Two logs merge: A6 missing from the first comes from the second.
head -n 5 "$T/full.log" > "$T/p1.log"; tail -n 1 "$T/full.log" > "$T/p2.log"
python3 -I "$rep" --board nanopc-t6 --disclaimer "$T/p1.log" "$T/p2.log" \
	> "$T/merged.md"
t "two logs merge and pass the checker" sh "$chk" "$T/merged.md"

# Usage errors.
python3 -I "$rep" "$T/full.log" >/dev/null 2>&1
[ $? -eq 2 ] && ok "no --board" || bad "no --board"
python3 -I "$rep" --board "$ip2" "$T/full.log" >/dev/null 2>&1
[ $? -eq 2 ] && ok "address as board name refused" || bad "address as board"
python3 -I "$rep" --board x "$T/none.log" >/dev/null 2>&1
[ $? -eq 2 ] && ok "missing file" || bad "missing file"

exit "$fail"
