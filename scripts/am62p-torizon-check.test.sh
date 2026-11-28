#!/bin/sh
# Host test for am62p-torizon-check.sh with a fake ssh. No board access.

here=$(cd "$(dirname "$0")" && pwd)
script=$here/am62p-torizon-check.sh
T=$(mktemp -d "${TMPDIR:-/tmp}/torizon-check-test.XXXXXX") || exit 1
trap 'rm -rf "$T"' EXIT
trap 'exit 1' INT TERM

# fake ssh: records the last argument (remote command), prints $FAKE_OUT,
# exits $FAKE_RC
cat > "$T/ssh" <<'EOS'
#!/bin/sh
for a; do last=$a; done
printf '%s\n' "$last" > "$FAKE_CMD"
[ -f "$FAKE_OUT" ] && cat "$FAKE_OUT"
exit "${FAKE_RC:-0}"
EOS
chmod +x "$T/ssh"

DEST=testdest-for-leak-check
ipa=10.20.30.$((40 + 1))
export FAKE_CMD=$T/cmd FAKE_OUT=$T/fake

mkfake() { # os_id os_name systemd
	cat > "$FAKE_OUT" <<EOF
DATE 2026-11-28T10:00:00Z
OS_ID $1
OS_NAME $2
OS_VERSION_ID 7.2.0
OS_PRETTY_NAME $2 7.2.0
OS_VARIANT Docker
KERNEL 6.6.0-test
UPTIME  10:00:00 up 1 min,  1 user,  load average: 0.10, 0.05, 0.01
OSTREE * torizon abcdef.0 (host $ipa)
OSTREE serial number 12345
SYSTEMD $3
REMOTEPROC remoteproc0 name r5f state offline firmware am62-mcu-r5f0_0-fw
END
EOF
}

fails=0
check() { # desc cmd...
	d=$1; shift
	if "$@"; then echo "ok   $d"; else echo "FAIL $d"; fails=$((fails + 1)); fi
}
run() {
	SSH=$T/ssh AM62P_SSH_DEST= sh "$script" "$@" > "$T/out" 2>"$T/err"
	rc=$?
}
rc_is() { [ "$rc" -eq "$1" ]; }
has() { grep -q "$1" "$T/out"; }
one_verdict() { [ "$(grep -c '^TORIZON ' "$T/out")" -eq 1 ]; }
absent() { ! grep -qi -e "$1" "$T/out" "$T/err"; }
cmd_valid() { sh -n "$FAKE_CMD"; }

mkfake torizon "Torizon OS" running
run "$DEST"
check "up: exit 0" rc_is 0
check "up: verdict" has '^TORIZON UP$'
check "up: one verdict" one_verdict
check "up: kernel line" has '^KERNEL 6.6.0-test$'
check "up: remoteproc line" has '^REMOTEPROC remoteproc0 name r5f state offline firmware '
check "up: ip scrubbed" absent "$ipa"
check "up: serial scrubbed" absent 'serial'
check "up: destination not printed" absent "$DEST"
check "remote command is valid sh" cmd_valid

mkfake torizon "Torizon OS" degraded
run "$DEST"
check "degraded counts as up" rc_is 0

mkfake debian Debian running
run "$DEST"
check "non-torizon: exit 1" rc_is 1
check "non-torizon: NOT-UP" has '^TORIZON NOT-UP '
check "non-torizon: one verdict" one_verdict

mkfake torizon "Torizon OS" starting
run "$DEST"
check "starting: exit 1" rc_is 1
check "starting: NOT-UP" has '^TORIZON NOT-UP '
check "starting: one verdict" one_verdict

FAKE_RC=255 run "$DEST"
check "ssh fails: exit 1" rc_is 1
check "ssh fails: NOT-UP" has '^TORIZON NOT-UP ssh failed$'
check "ssh fails: one verdict" one_verdict
check "ssh fails: destination not printed" absent "$DEST"

run
check "no destination: exit 2" rc_is 2
check "no destination: NOT-UP" has '^TORIZON NOT-UP '

mkfake torizon "Torizon OS" running
SSH=$T/ssh AM62P_SSH_DEST=$DEST sh "$script" > "$T/out" 2>&1
check "destination from environment" has '^TORIZON UP$'

if [ "$fails" -eq 0 ]; then echo "all passed"; else echo "$fails failed"; exit 1; fi
