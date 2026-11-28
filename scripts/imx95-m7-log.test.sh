#!/usr/bin/env bash
# Host test for imx95-m7-log.sh: runs it on a fake sysfs and /dev tree under
# $TMPDIR. Run: bash scripts/imx95-m7-log.test.sh
here=$(cd "$(dirname "$0")" && pwd)
script=$here/imx95-m7-log.sh
root=$(mktemp -d "${TMPDIR:-/tmp}/m7-log-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok()  { echo "ok   $1"; }
bad() { echo "FAIL $1"; fail=1; }
check() { [ "$2" = "$3" ] && ok "$1" || bad "$1 (got $2, want $3)"; }

mkdir -p "$root/dev" "$root/sys/kernel/debug/remoteproc/remoteproc0" "$root/empty" "$root/out"
printf 'CETRACE v=1 ev=commit seq=1 t=10\nhello from m7\n' >"$root/dev/ttyRPMSG0"
printf 'trace line one\n' >"$root/sys/kernel/debug/remoteproc/remoteproc0/trace0"

# run SECS OUT [DEVROOT]: LOG_ROOT points at the temp dir so the test never
# writes under /tmp itself.
run() {
	SYSFS_ROOT=$root/sys DEV_ROOT=${3:-$root/dev} LOG_ROOT=$root/out \
		sh "$script" "$1" "$2" >"$root/stdout" 2>"$root/stderr"
}

# log captured
run 1 "$root/out/log.txt"; check "capture exit" $? 0
if head -n1 "$root/out/log.txt" | grep -Eq '^# date: [0-9]{4}-[0-9]{2}-[0-9]{2}$'; then ok "date header first"; else bad "date header first"; fi
if grep -Fq 'hello from m7' "$root/out/log.txt"; then ok "tty content"; else bad "tty content"; fi
if grep -Fq 'trace line one' "$root/out/log.txt"; then ok "trace content"; else bad "trace content"; fi

# no rpmsg tty but a trace file: trace-only capture, exit 0
run 1 "$root/out/tr.txt" "$root/empty"; check "trace-only exit" $? 0
if grep -Fq 'trace line one' "$root/out/tr.txt"; then ok "trace-only content"; else bad "trace-only content"; fi
if grep -Fq 'no ttyRPMSG' "$root/out/tr.txt"; then ok "trace-only note"; else bad "trace-only note"; fi

# neither tty nor trace: exit 1, clear message, no file
mkdir -p "$root/nosys"
SYSFS_ROOT=$root/nosys DEV_ROOT=$root/empty LOG_ROOT=$root/out sh "$script" 1 "$root/out/none.txt" >/dev/null 2>"$root/stderr"
check "neither exit" $? 1
if grep -Fq 'neither ttyRPMSG' "$root/stderr"; then ok "neither message"; else bad "neither message"; fi
if [ -e "$root/out/none.txt" ]; then bad "no file on failure"; else ok "no file on failure"; fi

# a symlinked OUTFILE is refused and its target left alone
printf 'keep\n' >"$root/target"; ln -s "$root/target" "$root/out/link.txt"
run 1 "$root/out/link.txt"; check "symlink exit" $? 2
if [ "$(cat "$root/target")" = keep ]; then ok "symlink target intact"; else bad "symlink target intact"; fi

# fixtures from scripts/testdata/imx95-m7-log/
fx=$here/testdata/imx95-m7-log
mkdir -p "$root/fxdev" "$root/fxsys/kernel/debug/remoteproc/remoteproc0"
cp "$fx/ttyRPMSG0" "$root/fxdev/"; cp "$fx/trace0" "$root/fxsys/kernel/debug/remoteproc/remoteproc0/"
SYSFS_ROOT=$root/fxsys DEV_ROOT=$root/fxdev LOG_ROOT=$root/out sh "$script" 1 "$root/out/fx.txt" >/dev/null 2>&1
if grep -Fq 'fixture tty line' "$root/out/fx.txt" && grep -Fq 'fixture trace line' "$root/out/fx.txt"; then ok "fixture capture"; else bad "fixture capture"; fi

# output path outside /tmp (default LOG_ROOT) is rejected
SYSFS_ROOT=$root/sys DEV_ROOT=$root/dev sh "$script" 1 /var/imx95-m7-log-test.txt >/dev/null 2>"$root/stderr"
check "outside /tmp exit" $? 2
if [ -e /var/imx95-m7-log-test.txt ]; then bad "outside file absent"; else ok "outside file absent"; fi
SYSFS_ROOT=$root/sys DEV_ROOT=$root/dev sh "$script" 1 /tmp/../etc/x >/dev/null 2>&1
check "dotdot exit" $? 2
SYSFS_ROOT=$root/sys DEV_ROOT=$root/dev sh "$script" abc /tmp/x >/dev/null 2>&1
check "bad duration exit" $? 2

# the script holds no fixed tty path or address
if grep -Eq '/dev/tty|[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+' "$script"; then bad "no fixed path/address"; else ok "no fixed path/address"; fi

exit $fail
