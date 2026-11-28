#!/bin/sh
# Run an image on the Verdin AM62P MCU R5F through remoteproc and use the
# core's trace buffer (debugfs remoteproc/remoteprocN/trace0) as the console.
# What to stop or unbind first comes from STOPFILE and is never hard-coded.
#
# usage: am62p-r5f-run.sh [--timeout SECS] [--run-secs SECS] [--interval SECS]
#                         [--log FILE] IMAGE REMOTEPROC [STOPFILE]
#    or: am62p-r5f-run.sh restore STATEFILE
#   IMAGE       image to load; a regular file under /tmp, loaded through the
#               firmware_class path parameter (never copied, never installed)
#   REMOTEPROC  index (e.g. 1) or the contents of the remoteproc "name" file
#   STOPFILE    lines "stop UNIT" and "unbind DRIVER DEVICE"; "#" starts a
#               comment. The list to stop is passed here, not baked in.
#   --timeout   seconds to wait for "running" (default 10)
#   --run-secs  seconds to keep copying trace0 to the log (default 60,
#               0 = no time limit); ends early once a RESULT line appears
#   --interval  seconds between trace copies (default 1)
#   --log       log file under /tmp (default $STATE_DIR/log)
#
# Start flow: refuse IMAGE outside /tmp, a malformed STOPFILE, or a core that
# is not offline (this script never stops the core); record the state of
# every listed unit and binding in a state file; stop and unbind them; set
# the firmware_class path and the remoteproc firmware name; start the core;
# copy trace0 into the log every --interval seconds until --run-secs elapse
# or a line starting with RESULT appears; restore the firmware_class path,
# and the firmware name when the core is offline (the kernel writes that
# attribute only while offline); leave the core running, because a second
# stop fails on this board and a second image needs a reboot.
# restore STATEFILE: rebind what the state file recorded as bound and start
# what it recorded as active; remove the state file on success.
#
# Writes only under /tmp (state file, log and scratch), ${ROOT}/sys (bind
# and unbind files, the firmware name, the remoteproc state and the
# firmware_class path) and the units it was told to stop. It never touches
# the U-Boot environment, the eMMC boot areas or packages.
# Exit: 0 ok, 1 failure, 2 usage, 3 refused (core not offline), 4 core
# restore failed, 5 previous run not restored (run "restore" first).
#
# Test hooks (environment): ROOT (sysfs and debugfs prefix, default empty),
# RPROC_FW_ROOT (the only allowed directory of IMAGE, log and state,
# default /tmp), AM62P_STATE_DIR (default $RPROC_FW_ROOT/am62p-r5f-run),
# AM62P_TRACEFS (default $ROOT/sys/kernel/debug/remoteproc), RPROC_POLL and
# RPROC_POLLS_PER_SEC (start-wait poll, default 1 and 1), AM62P_TRACE_POLL
# and AM62P_TRACE_POLLS (trace copy interval and count; default --interval
# and --run-secs / --interval rounded up, count 0 = no limit), SYSTEMCTL
# (default systemctl), DMESG (default dmesg).

ROOT=${ROOT:-}
FW_ROOT=${RPROC_FW_ROOT:-/tmp}
POLL=${RPROC_POLL:-1}
PPS=${RPROC_POLLS_PER_SEC:-1}
DMESG=${DMESG:-dmesg}
SYSTEMCTL=${SYSTEMCTL:-systemctl}
STATE_DIR=${AM62P_STATE_DIR:-$FW_ROOT/am62p-r5f-run}
TRACEFS=${AM62P_TRACEFS:-$ROOT/sys/kernel/debug/remoteproc}
FWPARAM=$ROOT/sys/module/firmware_class/parameters/path

rd() { cat "$1" 2>/dev/null; }
wr() { printf '%s\n' "$2" > "$1"; }

inside_root() { # path: must be under $FW_ROOT, no ".."
	case $1 in
	*/../* | */..) return 1 ;;
	"$FW_ROOT"/*) return 0 ;;
	esac
	return 1
}

# a single path-free token (unit, driver or device name)
token_ok() {
	case $1 in
	"" | */* | -* | *..*) return 1 ;;
	esac
	return 0
}

# a file or directory that is ours and not a symlink
own_ok() { # path
	[ ! -L "$1" ] && [ -O "$1" ]
}

find_rp() { # index or name -> remoteproc directory
	case $1 in
	*[!0-9]* | "")
		for _d in "$ROOT"/sys/class/remoteproc/*; do
			[ -f "$_d/name" ] || continue
			[ "$(cat "$_d/name" 2>/dev/null)" = "$1" ] && { echo "$_d"; return 0; }
		done
		return 1
		;;
	*)
		echo "$ROOT/sys/class/remoteproc/remoteproc$1"
		;;
	esac
}

find_driver() { # driver name -> driver directory under ${ROOT}/sys/bus
	for _d in "$ROOT"/sys/bus/*/drivers/"$1"; do
		[ -d "$_d" ] && { echo "$_d"; return 0; }
	done
	return 1
}

wait_state() { # rpdir want
	_n=$((timeout * PPS))
	while :; do
		[ "$(rd "$1/state")" = "$2" ] && return 0
		[ "$_n" -le 0 ] && return 1
		_n=$((_n - 1))
		sleep "$POLL"
	done
}

# sysfs removes or adds the device link synchronously; the loop keeps the
# check from racing an attribute that updates slightly later.
wait_file() { # path present|absent
	_n=$((2 + PPS))
	while :; do
		if [ "$2" = present ]; then
			[ -e "$1" ] && return 0
		else
			[ ! -e "$1" ] && return 0
		fi
		[ "$_n" -le 0 ] && return 1
		_n=$((_n - 1))
		sleep "$POLL"
	done
}

# --- restore STATEFILE -----------------------------------------------------
# Rebind what was recorded as bound, then start what was recorded as active.
# Reads the state file; touches nothing else.
restore_state() {
	[ $# -eq 1 ] || { echo "usage: $0 restore STATEFILE" >&2; return 2; }
	sf=$1
	case $sf in
	*/../* | */..) echo "state file path must not contain .." >&2; return 2 ;;
	esac
	inside_root "$sf" || { echo "state file must be under $FW_ROOT" >&2; return 2; }
	[ -f "$sf" ] || { echo "state file not found: $sf" >&2; return 2; }
	if ! own_ok "$sf" || ! own_ok "${sf%/*}"; then
		echo "state file or its directory is a symlink or not owned by this user: $sf" >&2
		return 2
	fi
	bad=0 nb=0 nu=0
	while read -r _k _a _b _c; do
		[ "$_k" = bind ] || continue
		if [ "$_c" != bound ]; then
			echo "skip rebind: $_b (was ${_c:-unbound})"
			continue
		fi
		case $_a in
		*..*) echo "FAIL rebind: $_b (bad driver path)" >&2; bad=1; continue ;;
		"$ROOT"/sys/bus/*/drivers/*) ;;
		*) echo "FAIL rebind: $_b (driver path outside ${ROOT}/sys/bus)" >&2; bad=1; continue ;;
		esac
		if ! token_ok "$_b"; then
			echo "FAIL rebind: bad device name in state file" >&2
			bad=1
			continue
		fi
		if [ ! -d "$_a" ]; then
			echo "FAIL rebind: $_b (driver directory missing)" >&2
			bad=1
			continue
		fi
		if [ -e "$_a/$_b" ]; then
			echo "OK rebind: $_b (still bound)"
			nb=$((nb + 1))
			continue
		fi
		printf '%s\n' "$_b" > "$_a/bind" 2>/dev/null
		if wait_file "$_a/$_b" present; then
			echo "OK rebind: $_b"
			nb=$((nb + 1))
		else
			echo "FAIL rebind: $_b" >&2
			bad=1
		fi
	done < "$sf"
	while read -r _k _a _b; do
		[ "$_k" = unit ] || continue
		if [ "$_b" != active ]; then
			echo "skip start: $_a (was ${_b:-unknown})"
			continue
		fi
		if ! token_ok "$_a"; then
			echo "FAIL start: bad unit name in state file" >&2
			bad=1
			continue
		fi
		# SYSTEMCTL may be a command with arguments, so it is split on purpose
		# shellcheck disable=SC2086
		$SYSTEMCTL start "$_a" 2>/dev/null
		# shellcheck disable=SC2086
		if [ "$($SYSTEMCTL is-active "$_a" 2>/dev/null)" = active ]; then
			echo "OK start: $_a"
			nu=$((nu + 1))
		else
			echo "FAIL start: $_a" >&2
			bad=1
		fi
	done < "$sf"
	echo "restored: bindings=$nb units=$nu state=$sf"
	if [ "$bad" -eq 0 ]; then
		rm -f "$sf"
		rmdir "$(dirname "$sf")" 2>/dev/null
	fi
	return "$bad"
}

case ${1:-} in
restore) shift; restore_state "$@"; exit $? ;;
esac

usage() { sed -n '2,/^$/s/^# \{0,1\}//p' "$0" >&2; }

timeout=10
runsecs=60
interval=1
logfile=
while [ $# -gt 0 ]; do
	case $1 in
	--timeout) shift; timeout=${1:-} ;;
	--run-secs) shift; runsecs=${1:-} ;;
	--interval) shift; interval=${1:-} ;;
	--log) shift; logfile=${1:-} ;;
	--) shift; break ;;
	-*) echo "unknown option: $1" >&2; exit 2 ;;
	*) break ;;
	esac
	shift
done
if [ $# -lt 2 ] || [ $# -gt 3 ]; then
	usage
	exit 2
fi
elf=$1
sel=$2
stopfile=${3:-}
for v in "$timeout" "$runsecs" "$interval"; do
	case $v in "" | *[!0-9]*) echo "numeric option expected" >&2; exit 2 ;; esac
done

case $elf in
*/../* | */..) echo "ELF path must not contain .." >&2; exit 2 ;;
esac
inside_root "$elf" || { echo "ELF must be under $FW_ROOT" >&2; exit 2; }
[ -f "$elf" ] || { echo "ELF not found: $elf" >&2; exit 2; }
fwdir=${elf%/*}
fwname=${elf##*/}

log=${logfile:-$STATE_DIR/log}
case $log in
*/../* | */..) echo "log path must not contain .." >&2; exit 2 ;;
esac
inside_root "$log" || { echo "log must be under $FW_ROOT" >&2; exit 2; }
inside_root "$STATE_DIR/x" || { echo "state dir must be under $FW_ROOT" >&2; exit 2; }

if [ -n "$stopfile" ]; then
	[ -f "$stopfile" ] || { echo "stop file not found: $stopfile" >&2; exit 2; }
	while read -r _a _b _c _d; do
		case $_a in '' | '#'*) continue ;; esac
		if [ "$_a" = stop ] && token_ok "$_b" && [ -z "$_c" ]; then
			:
		elif [ "$_a" = unbind ] && token_ok "$_b" && token_ok "$_c" && [ -z "$_d" ]; then
			find_driver "$_b" >/dev/null || {
				echo "stop file: driver not found: $_b" >&2; exit 2; }
		else
			echo "stop file: bad line: $_a $_b $_c $_d" >&2
			exit 2
		fi
	done < "$stopfile"
fi

rp=$(find_rp "$sel") || { echo "remoteproc '$sel' not found" >&2; exit 2; }
if ! [ -f "$rp/state" ] || ! [ -f "$rp/firmware" ]; then
	echo "remoteproc '$sel' not found" >&2
	exit 2
fi
[ -f "$FWPARAM" ] || { echo "firmware_class path parameter missing" >&2; exit 2; }
trace0=$TRACEFS/${rp##*/}/trace0
[ -f "$trace0" ] || { echo "trace buffer missing: $trace0" >&2; exit 2; }

orig_state=$(rd "$rp/state")
orig_fw=$(rd "$rp/firmware")
orig_path=$(rd "$FWPARAM")
echo "before: state=$orig_state firmware=$orig_fw firmware_class.path=$orig_path"

if [ "$orig_state" != offline ]; then
	echo "refused: core state is '$orig_state', not offline; this script never stops the core" >&2
	exit 3
fi
[ ! -e "$STATE_DIR/state" ] || {
	echo "refused: previous run not restored; run: $0 restore $STATE_DIR/state" >&2
	exit 5
}
mkdir -p "$STATE_DIR" || exit 1
if [ ! -d "$STATE_DIR" ] || ! own_ok "$STATE_DIR"; then
	echo "refused: $STATE_DIR is not a real directory owned by this user" >&2
	exit 1
fi
chmod 700 "$STATE_DIR" || exit 1
if [ -L "$STATE_DIR/state" ]; then
	echo "refused: $STATE_DIR/state is a symlink" >&2
	exit 1
fi
if [ -L "$log" ] || { [ -e "$log" ] && { [ ! -f "$log" ] || ! own_ok "$log"; }; }; then
	echo "refused: log $log is a symlink or not our regular file" >&2
	exit 1
fi

# record every listed unit and binding before anything is changed
{
	echo "elf=$elf"
	echo "fwname=$fwname"
	echo "orig_fw=$orig_fw"
	echo "orig_path=$orig_path"
	echo "orig_state=$orig_state"
	echo "rproc=$rp"
	echo "log=$log"
	if [ -n "$stopfile" ]; then
		while read -r _a _b _c; do
			case $_a in '' | '#'*) continue ;; esac
			if [ "$_a" = stop ]; then
				# shellcheck disable=SC2086
				st=$($SYSTEMCTL is-active "$_b" 2>/dev/null)
				echo "unit $_b ${st:-inactive}"
			else
				drv=$(find_driver "$_b")
				if [ -e "$drv/$_c" ]; then bst=bound; else bst=unbound; fi
				echo "bind $drv $_c $bst"
			fi
		done < "$stopfile"
	fi
} > "$STATE_DIR/state" || exit 1
echo "recorded: state=$STATE_DIR/state"

apply_fail=0
if [ -n "$stopfile" ]; then
	while read -r _a _b _c; do
		case $_a in '' | '#'*) continue ;; esac
		if [ "$_a" = stop ]; then
			# shellcheck disable=SC2086
			if $SYSTEMCTL stop "$_b" 2>/dev/null; then
				echo "stop: $_b"
			else
				echo "FAIL stop: $_b" >&2
				apply_fail=1
			fi
		else
			drv=$(find_driver "$_b")
			if [ ! -e "$drv/$_c" ]; then
				echo "skip unbind: $_c (not bound)"
				continue
			fi
			printf '%s\n' "$_c" > "$drv/unbind" 2>/dev/null
			if wait_file "$drv/$_c" absent; then
				echo "unbind: $_c from $_b"
			else
				echo "FAIL unbind: $_c still bound to $_b" >&2
				apply_fail=1
			fi
		fi
	done < "$stopfile"
fi
if [ "$apply_fail" -ne 0 ]; then
	echo "stop file: failed to stop or unbind; core untouched; run: $0 restore $STATE_DIR/state" >&2
	exit 1
fi

work=$(mktemp -d "${TMPDIR:-/tmp}/am62p-r5f-run.XXXXXX") || exit 1
touched=0
rc=1
done_cleanup=0

# invoked indirectly through: trap cleanup EXIT
# shellcheck disable=SC2317
cleanup() {
	[ "$done_cleanup" -eq 1 ] && return
	done_cleanup=1
	trap '' INT TERM HUP
	fwok=skip
	pathok=skip
	if [ "$touched" -eq 1 ]; then
		core=$(rd "$rp/state")
		if [ "$core" = offline ]; then
			wr "$rp/firmware" "$orig_fw" 2>/dev/null
			if [ "$(rd "$rp/firmware")" = "$orig_fw" ]; then
				fwok=restored
			else
				fwok=FAILED
			fi
		else
			fwok=kept
			echo "firmware: name not restored (core state '$core'; the kernel writes it only while offline; core not stopped on purpose)"
		fi
		wr "$FWPARAM" "$orig_path" 2>/dev/null
		if [ "$(rd "$FWPARAM")" = "$orig_path" ]; then
			pathok=restored
		else
			pathok=FAILED
		fi
	fi
	echo "restored: firmware=$fwok firmware_class.path=$pathok"
	echo "after: state=$(rd "$rp/state") firmware=$(rd "$rp/firmware") firmware_class.path=$(rd "$FWPARAM")"
	echo "log: $log"
	echo "state: $STATE_DIR/state (restore units and bindings with: $0 restore $STATE_DIR/state)"
	rm -rf "$work"
	if { [ "$fwok" = FAILED ] || [ "$pathok" = FAILED ]; } && [ "$rc" -eq 0 ]; then
		rc=4
	fi
	exit "$rc"
}
trap cleanup EXIT
trap 'rc=130; exit 130' INT
trap 'rc=143; exit 143' TERM
trap 'rc=129; exit 129' HUP

touched=1
wr "$FWPARAM" "$fwdir" || { echo "cannot set firmware_class.path" >&2; exit 1; }
wr "$rp/firmware" "$fwname" || { echo "cannot set firmware name" >&2; exit 1; }

before=$($DMESG 2>/dev/null | wc -l)
echo "start: firmware=$fwname dir=$fwdir"
if ! wr "$rp/state" start; then
	echo "start: write failed" >&2
	exit 1
fi
if ! wait_state "$rp" running; then
	echo "start: not running after ${timeout}s (state=$(rd "$rp/state"))" >&2
	$DMESG 2>/dev/null | tail -n +"$((before + 1))" | sed 's/^/dmesg: /'
	exit 1
fi
echo "running"

polls_want=$(( (runsecs + interval - 1) / interval ))
polls=${AM62P_TRACE_POLLS:-$polls_want}
tpoll=${AM62P_TRACE_POLL:-$interval}
: > "$log" || { echo "cannot write log: $log" >&2; exit 1; }
if [ "$runsecs" -gt 0 ]; then
	limit="${runsecs}s"
else
	limit="no time limit"
fi
echo "trace: $trace0 -> $log every ${tpoll}s, until $limit or a RESULT line"
seen=0
n=0
result=0
while :; do
	rd "$trace0" > "$work/raw"
	total=$(wc -l < "$work/raw" | tr -d ' ')
	if [ "$total" -gt "$seen" ]; then
		tail -n +"$((seen + 1))" "$work/raw" | tee -a "$log" | sed 's/^/trace: /'
	elif [ "$total" -lt "$seen" ]; then
		# trace buffer wrapped or reset: keep a full copy
		tee -a "$log" < "$work/raw" | sed 's/^/trace: /'
	fi
	seen=$total
	if grep -q '^RESULT' "$log" 2>/dev/null; then
		result=1
		break
	fi
	if [ "$polls" -gt 0 ] && [ "$n" -ge "$polls" ]; then break; fi
	n=$((n + 1))
	sleep "$tpoll"
done
echo "state-after-run: $(rd "$rp/state")"
$DMESG 2>/dev/null | tail -n +"$((before + 1))" | sed 's/^/dmesg: /'
if [ "$result" -eq 1 ]; then
	echo "result: RESULT line seen after $((n + 1)) trace copies"
else
	echo "note: no RESULT line in the log after $((n + 1)) trace copies"
fi
rc=0
exit 0
