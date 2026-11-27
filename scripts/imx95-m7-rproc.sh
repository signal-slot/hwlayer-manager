#!/bin/sh
# Load an ELF onto a remoteproc core (i.MX95 M7), collect evidence, then stop
# the core and restore the firmware name and firmware_class path exactly.
#
# usage: imx95-m7-rproc.sh [--force-stop] [--timeout SECS] [--run-secs SECS]
#                          [--log FILE] ELF REMOTEPROC
#   ELF         image to load; must be a regular file under /tmp
#   REMOTEPROC  index (e.g. 0) or the contents of the remoteproc "name" file
#   --force-stop  if the core is not offline, record its state and stop it
#                 first (otherwise the script refuses and changes nothing)
#   --timeout     seconds to wait for "running" (default 10)
#   --run-secs    seconds to let the core run before collecting (default 3)
#   --log FILE    optional rpmsg tty or log file to capture while running
#
# Writes only the remoteproc "firmware" and "state" files and the
# firmware_class "path" parameter, all under ${ROOT}/sys. Nothing else.
# Exit: 0 ok and restored, 1 failure (restore still attempted), 2 usage,
# 3 refused (core not offline), 4 restore failed.
#
# Test hooks (environment): ROOT (sysfs prefix, default empty),
# RPROC_FW_ROOT (allowed ELF directory, default /tmp), RPROC_POLL and
# RPROC_POLLS_PER_SEC (poll interval and polls per second, default 1 and 1),
# DMESG (dmesg command, default dmesg).

ROOT=${ROOT:-}
FW_ROOT=${RPROC_FW_ROOT:-/tmp}
POLL=${RPROC_POLL:-1}
PPS=${RPROC_POLLS_PER_SEC:-1}
DMESG=${DMESG:-dmesg}
FWPARAM=$ROOT/sys/module/firmware_class/parameters/path

force=0
timeout=10
runsecs=3
logfile=
while [ $# -gt 0 ]; do
	case $1 in
	--force-stop) force=1 ;;
	--timeout) shift; timeout=${1:-} ;;
	--run-secs) shift; runsecs=${1:-} ;;
	--log) shift; logfile=${1:-} ;;
	--) shift; break ;;
	-*) echo "unknown option: $1" >&2; exit 2 ;;
	*) break ;;
	esac
	shift
done
[ $# -eq 2 ] || { sed -n '2,/^$/s/^# \{0,1\}//p' "$0" >&2; exit 2; }
elf=$1
sel=$2
case $timeout$runsecs in *[!0-9]* | "") echo "numeric option expected" >&2; exit 2 ;; esac
[ -n "$timeout" ] && [ -n "$runsecs" ] || { echo "numeric option expected" >&2; exit 2; }

case $elf in
*/../* | */..) echo "ELF path must not contain .." >&2; exit 2 ;;
"$FW_ROOT"/*) ;;
*) echo "ELF must be under $FW_ROOT" >&2; exit 2 ;;
esac
[ -f "$elf" ] || { echo "ELF not found: $elf" >&2; exit 2; }
fwdir=${elf%/*}
fwname=${elf##*/}

rp=
case $sel in
"" | *[!0-9]*)
	for d in "$ROOT"/sys/class/remoteproc/*; do
		[ -f "$d/name" ] || continue
		if [ "$(cat "$d/name")" = "$sel" ]; then rp=$d; break; fi
	done
	;;
*) rp=$ROOT/sys/class/remoteproc/remoteproc$sel ;;
esac
[ -n "$rp" ] && [ -f "$rp/state" ] && [ -f "$rp/firmware" ] || {
	echo "remoteproc '$sel' not found" >&2; exit 2; }
[ -f "$FWPARAM" ] || { echo "firmware_class path parameter missing" >&2; exit 2; }

rd() { cat "$1" 2>/dev/null; }
wr() { printf '%s\n' "$2" > "$1"; }

orig_state=$(rd "$rp/state")
orig_fw=$(rd "$rp/firmware")
orig_path=$(rd "$FWPARAM")
echo "before: state=$orig_state firmware=$orig_fw firmware_class.path=$orig_path"

if [ "$orig_state" != offline ]; then
	if [ "$force" -ne 1 ]; then
		echo "refused: state is '$orig_state', not offline (use --force-stop)" >&2
		exit 3
	fi
	echo "force-stop: recorded state=$orig_state, stopping"
fi

work=$(mktemp -d "${TMPDIR:-/tmp}/imx95-m7-rproc.XXXXXX") || exit 1
touched=0
catpid=
rc=1
done_cleanup=0

wait_state() { # want
	_n=$((timeout * PPS))
	while :; do
		[ "$(rd "$rp/state")" = "$1" ] && return 0
		[ "$_n" -le 0 ] && return 1
		_n=$((_n - 1))
		sleep "$POLL"
	done
}

cleanup() {
	[ "$done_cleanup" -eq 1 ] && return
	done_cleanup=1
	trap '' INT TERM HUP
	[ -n "$catpid" ] && kill "$catpid" 2>/dev/null
	stopped=ok
	if [ "$(rd "$rp/state")" != offline ]; then
		wr "$rp/state" stop 2>/dev/null
		wait_state offline || stopped=FAILED
	fi
	fwok=ok
	pathok=ok
	if [ "$touched" -eq 1 ]; then
		wr "$rp/firmware" "$orig_fw" 2>/dev/null
		wr "$FWPARAM" "$orig_path" 2>/dev/null
		[ "$(rd "$rp/firmware")" = "$orig_fw" ] || fwok=FAILED
		[ "$(rd "$FWPARAM")" = "$orig_path" ] || pathok=FAILED
	fi
	note=
	[ "$orig_state" != offline ] && note=" (core was $orig_state before; left offline)"
	echo "restored: core=$stopped firmware=$fwok firmware_class.path=$pathok$note"
	echo "restored: state=$(rd "$rp/state") firmware=$(rd "$rp/firmware") firmware_class.path=$(rd "$FWPARAM")"
	rm -rf "$work"
	if [ "$stopped$fwok$pathok" != okokok ] && [ "$rc" -eq 0 ]; then
		rc=4
	fi
	exit "$rc"
}
trap cleanup EXIT
trap 'rc=130; exit 130' INT
trap 'rc=143; exit 143' TERM
trap 'rc=129; exit 129' HUP

touched=1
if [ "$orig_state" != offline ]; then
	wr "$rp/state" stop || exit 1
	wait_state offline || { echo "core did not stop" >&2; exit 1; }
fi
wr "$FWPARAM" "$fwdir" || { echo "cannot set firmware_class.path" >&2; exit 1; }
wr "$rp/firmware" "$fwname" || { echo "cannot set firmware name" >&2; exit 1; }

before=$($DMESG 2>/dev/null | wc -l)
if [ -n "$logfile" ]; then
	cat "$logfile" > "$work/log" 2>/dev/null &
	catpid=$!
fi
echo "start: firmware=$fwname dir=$fwdir"
if ! wr "$rp/state" start; then
	echo "start: write failed" >&2
	exit 1
fi
if ! wait_state running; then
	echo "start: not running after ${timeout}s (state=$(rd "$rp/state"))" >&2
	$DMESG 2>/dev/null | tail -n +"$((before + 1))" | sed 's/^/dmesg: /'
	exit 1
fi
echo "running"
sleep "$runsecs"
echo "state-after-run: $(rd "$rp/state")"
$DMESG 2>/dev/null | tail -n +"$((before + 1))" | sed 's/^/dmesg: /'
if [ -n "$logfile" ]; then
	[ -n "$catpid" ] && kill "$catpid" 2>/dev/null
	catpid=
	sed 's/^/log: /' "$work/log" 2>/dev/null
fi
rc=0
exit 0
