#!/bin/sh
# Load an ELF onto a remoteproc core (i.MX95 M7), collect evidence, then stop
# the core and restore the firmware name and firmware_class path exactly.
#
# usage: imx95-m7-rproc.sh start IMAGE | stop     (see "subcommands" below)
#    or: imx95-m7-rproc.sh [--force-stop] [--timeout SECS] [--run-secs SECS]
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

# --- subcommands: start IMAGE.elf | stop ---------------------------------
#   start IMAGE  find the M7 remoteproc by name, record its state, firmware
#                name and firmware_class path in a state file, copy IMAGE to
#                a work directory, point firmware_class at it, set the
#                firmware name and start the core
#   stop         stop the core if start did, restore firmware name and path
#                from the state file, check the display service is active
#                and print "OK <check>" / "FAIL <check>" lines
# Environment: SYSFS_ROOT (default /sys), SYSTEMCTL (default systemctl),
# M7_NAME_GLOB (default '*m7* *M7* *imx-rproc*': space separated case
# patterns matched against each remoteproc "name"), M7_STATE_DIR (default
# imx95-m7-rproc under /tmp), RPROC_FW_ROOT (the only allowed prefix of the
# state directory and the image, default /tmp), M7_TIMEOUT (seconds to wait
# for a state change, default 10), RPROC_POLL, RPROC_POLLS_PER_SEC.
# Exit: 0 ok, 1 failure, 2 usage, 3 no M7 remoteproc, 4 core not offline,
# 5 a previous start was not stopped.
SYSFS_ROOT=${SYSFS_ROOT:-/sys}
SYSTEMCTL=${SYSTEMCTL:-systemctl}
M7_NAME_GLOB=${M7_NAME_GLOB:-*m7* *M7* *imx-rproc*}
STATE_DIR=${M7_STATE_DIR:-$FW_ROOT/imx95-m7-rproc}
SUB_FWPARAM=$SYSFS_ROOT/module/firmware_class/parameters/path

sub_inside_root() { # path: must be under $FW_ROOT, no ".."
	case $1 in
	*/../* | */..) return 1 ;;
	"$FW_ROOT"/*) return 0 ;;
	esac
	return 1
}

sub_find_rp() {
	for _d in "$SYSFS_ROOT"/class/remoteproc/*; do
		[ -f "$_d/name" ] || continue
		_n=$(cat "$_d/name" 2>/dev/null)
		for _g in $M7_NAME_GLOB; do
			# shellcheck disable=SC2254
			case $_n in $_g) echo "$_d"; return 0 ;; esac
		done
	done
	return 1
}

sub_wait() { # rp want
	_t=$(( ${M7_TIMEOUT:-10} * PPS ))
	while :; do
		[ "$(cat "$1/state" 2>/dev/null)" = "$2" ] && return 0
		[ "$_t" -le 0 ] && return 1
		_t=$((_t - 1))
		sleep "$POLL"
	done
}

sub_start() {
	[ $# -eq 1 ] || { echo "usage: $0 start IMAGE.elf" >&2; return 2; }
	img=$1
	sub_inside_root "$img" || { echo "FAIL image-path: must be under $FW_ROOT" >&2; return 2; }
	sub_inside_root "$STATE_DIR/x" || { echo "FAIL state-dir: must be under $FW_ROOT" >&2; return 2; }
	[ -f "$img" ] || { echo "FAIL image: not a file: $img" >&2; return 2; }
	[ -f "$SUB_FWPARAM" ] || { echo "FAIL firmware-class-path: parameter missing" >&2; return 1; }
	[ ! -e "$STATE_DIR/state" ] || { echo "FAIL state-file: previous start not stopped (run stop)" >&2; return 5; }
	rp=$(sub_find_rp) || { echo "FAIL m7-remoteproc: none matches '$M7_NAME_GLOB'" >&2; return 3; }
	[ -f "$rp/state" ] && [ -f "$rp/firmware" ] || { echo "FAIL m7-remoteproc: state/firmware files missing" >&2; return 1; }
	o_state=$(cat "$rp/state")
	if [ "$o_state" != offline ]; then
		echo "FAIL core-offline: state is '$o_state'; nothing changed" >&2
		return 4
	fi
	o_fw=$(cat "$rp/firmware")
	o_path=$(cat "$SUB_FWPARAM")
	mkdir -p "$STATE_DIR/fw" || return 1
	name=${img##*/}
	cp "$img" "$STATE_DIR/fw/$name" || return 1
	{
		echo "rproc=$rp"
		echo "orig_state=$o_state"
		echo "orig_fw=$o_fw"
		echo "orig_path=$o_path"
		echo "started=1"
	} > "$STATE_DIR/state"
	echo "recorded: rproc=${rp##*/} state=$o_state firmware=$o_fw firmware_class.path=$o_path"
	if printf '%s\n' "$STATE_DIR/fw" > "$SUB_FWPARAM" &&
		printf '%s\n' "$name" > "$rp/firmware" &&
		printf '%s\n' start > "$rp/state" &&
		sub_wait "$rp" running; then
		echo "start: running firmware=$name"
		echo "state=$(cat "$rp/state") firmware=$(cat "$rp/firmware") firmware_class.path=$(cat "$SUB_FWPARAM")"
		return 0
	fi
	echo "FAIL start: state=$(cat "$rp/state" 2>/dev/null); rolling back" >&2
	sub_stop
	return 1
}

sub_stop() {
	[ -f "$STATE_DIR/state" ] || { echo "FAIL state-file: nothing to stop (no start recorded)" >&2; return 1; }
	sub_inside_root "$STATE_DIR/x" || { echo "FAIL state-dir: must be under $FW_ROOT" >&2; return 2; }
	rp= o_fw= o_path= started=0
	while IFS== read -r _k _v; do
		case $_k in
		rproc) rp=$_v ;;
		orig_fw) o_fw=$_v ;;
		orig_path) o_path=$_v ;;
		started) started=$_v ;;
		esac
	done < "$STATE_DIR/state"
	[ -n "$rp" ] && [ -f "$rp/state" ] || { echo "FAIL state-file: bad contents" >&2; return 1; }
	bad=0
	if [ "$started" = 1 ] && [ "$(cat "$rp/state")" != offline ]; then
		printf '%s\n' stop > "$rp/state" 2>/dev/null
	fi
	if sub_wait "$rp" offline; then echo "OK core-offline"; else echo "FAIL core-offline"; bad=1; fi
	printf '%s\n' "$o_fw" > "$rp/firmware" 2>/dev/null
	if [ "$(cat "$rp/firmware")" = "$o_fw" ]; then echo "OK firmware-restored"; else echo "FAIL firmware-restored"; bad=1; fi
	printf '%s\n' "$o_path" > "$SUB_FWPARAM" 2>/dev/null
	if [ "$(cat "$SUB_FWPARAM")" = "$o_path" ]; then echo "OK firmware-class-path-restored"; else echo "FAIL firmware-class-path-restored"; bad=1; fi
	# SYSTEMCTL may be a command with arguments, so it is split on purpose
	# shellcheck disable=SC2086
	if [ "$($SYSTEMCTL is-active weston 2>/dev/null)" = active ]; then echo "OK weston-active"; else echo "FAIL weston-active"; bad=1; fi
	if [ "$bad" -eq 0 ]; then
		rm -rf "$STATE_DIR/fw" "$STATE_DIR/state"
		rmdir "$STATE_DIR" 2>/dev/null
	fi
	return "$bad"
}

case ${1:-} in
start) shift; sub_start "$@"; exit $? ;;
stop) shift; sub_stop "$@"; exit $? ;;
esac

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
