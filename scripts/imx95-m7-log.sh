#!/bin/sh
# Capture the i.MX95 M7 log on the board: the rpmsg tty for a given time, then
# the remoteproc trace buffer if there is one. Meant to run through ssh:
#   ssh BOARD sh -s -- SECS OUTFILE <imx95-m7-log.sh
#
# usage: imx95-m7-log.sh SECS OUTFILE
#   SECS     seconds to read the rpmsg tty (positive integer)
#   OUTFILE  file to create; must be under /tmp (no ".." parts)
#
# The file starts with "# date: YYYY-MM-DD" (docs/trace-format.md). The tty
# bytes follow; the trace buffer, if present, is appended after a
# "# remoteproc trace: <name>" comment line. The only thing written is
# OUTFILE (mode 600). No module is loaded, no tty setting or sysfs file is
# changed. The rpmsg tty is found by globbing ttyRPMSG* under DEV_ROOT.
#
# Environment (test hooks): SYSFS_ROOT (default /sys), DEV_ROOT (default
# /dev), LOG_ROOT (allowed output prefix, default /tmp).
# Without an rpmsg tty the trace buffer alone is captured (SECS is then a wait
# before one read). A tty that appears after the start (the M7 announces its
# channel late) is picked up within SECS. A symlinked OUTFILE, or one whose
# directory resolves outside LOG_ROOT, is refused. On HUP/INT/TERM the partial
# log is kept with the trace buffer appended, and the exit status is 2.
# Exit: 0 ok, 1 neither tty nor trace found, 2 usage, refused path or signal.

SYSFS_ROOT=${SYSFS_ROOT:-/sys}
DEV_ROOT=${DEV_ROOT:-/dev}
LOG_ROOT=${LOG_ROOT:-/tmp}

[ $# -eq 2 ] || { echo "usage: $0 SECS OUTFILE" >&2; exit 2; }
secs=$1
out=$2
case $secs in
'' | *[!0-9]* | 0) echo "FAIL duration: need a positive integer: $secs" >&2; exit 2 ;;
esac
case $out in
*/../* | */..) echo "FAIL out-path: no '..' allowed: $out" >&2; exit 2 ;;
"$LOG_ROOT"/*) ;;
*) echo "FAIL out-path: must be under $LOG_ROOT: $out" >&2; exit 2 ;;
esac
if [ -e "$out" ] && [ ! -f "$out" ]; then
	echo "FAIL out-path: not a regular file: $out" >&2
	exit 2
fi
if [ -L "$out" ]; then
	echo "FAIL out-path: refusing a symlink: $out" >&2
	exit 2
fi
# A symlinked directory under LOG_ROOT must not lead the write elsewhere.
_dir=$(dirname "$out")
_phys=$(cd -P "$_dir" 2>/dev/null && pwd -P) || { echo "FAIL out-path: no such directory: $_dir" >&2; exit 2; }
_root=$(cd -P "$LOG_ROOT" 2>/dev/null && pwd -P) || _root=$LOG_ROOT
case $_phys in
"$_root" | "$_root"/*) ;;
*) echo "FAIL out-path: directory resolves outside $LOG_ROOT: $_dir" >&2; exit 2 ;;
esac

find_tty() {
	tty=
	for _t in "$DEV_ROOT"/ttyRPMSG*; do
		[ -e "$_t" ] || continue
		tty=$_t
		break
	done
}
append_trace() {
	for _tr in "$SYSFS_ROOT"/kernel/debug/remoteproc/*/trace*; do
		[ -f "$_tr" ] || continue
		echo "# remoteproc trace: $(basename "$(dirname "$_tr")")/$(basename "$_tr")" >>"$out"
		cat "$_tr" >>"$out" 2>/dev/null
	done
}

find_tty
have_trace=
for _tr in "$SYSFS_ROOT"/kernel/debug/remoteproc/*/trace*; do
	[ -f "$_tr" ] && have_trace=1
done
if [ -z "$tty" ] && [ -z "$have_trace" ]; then
	echo "FAIL log: neither ttyRPMSG* under $DEV_ROOT nor a remoteproc trace file under $SYSFS_ROOT (is the M7 running?)" >&2
	exit 1
fi

# The log may carry anything the M7 prints: keep it owner-only.
umask 077
date +"# date: %Y-%m-%d" >"$out" || { echo "FAIL out-path: cannot write $out" >&2; exit 2; }

pid=
trap '[ -n "$pid" ] && kill "$pid" 2>/dev/null; echo "# interrupted" >>"$out"; append_trace; echo "FAIL log: interrupted, partial $out" >&2; exit 2' HUP INT TERM
# Poll for the tty during the whole window instead of looking once.
_n=0
while [ "$_n" -lt "$secs" ]; do
	if [ -z "$pid" ]; then
		find_tty
		if [ -n "$tty" ]; then
			cat "$tty" >>"$out" 2>/dev/null &
			pid=$!
		fi
	elif ! kill -0 "$pid" 2>/dev/null; then
		break
	fi
	sleep 1
	_n=$((_n + 1))
done
if [ -n "$pid" ]; then
	kill "$pid" 2>/dev/null
	wait "$pid" 2>/dev/null
	pid=
else
	echo "# rpmsg: no ttyRPMSG* found, trace buffer only" >>"$out"
fi

append_trace
echo "OK log $out"
