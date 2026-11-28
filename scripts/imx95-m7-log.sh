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
# OUTFILE. No module is loaded, no tty setting or sysfs file is changed.
# The rpmsg tty is found by globbing ttyRPMSG* under DEV_ROOT.
#
# Environment (test hooks): SYSFS_ROOT (default /sys), DEV_ROOT (default
# /dev), LOG_ROOT (allowed output prefix, default /tmp).
# Exit: 0 ok, 1 no rpmsg tty found, 2 usage or refused path.

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

tty=
for _t in "$DEV_ROOT"/ttyRPMSG*; do
	[ -e "$_t" ] || continue
	tty=$_t
	break
done
if [ -z "$tty" ]; then
	echo "FAIL rpmsg: no ttyRPMSG* under $DEV_ROOT (is the M7 running with an rpmsg tty?)" >&2
	exit 1
fi

date +"# date: %Y-%m-%d" >"$out" || { echo "FAIL out-path: cannot write $out" >&2; exit 2; }

cat "$tty" >>"$out" 2>/dev/null &
pid=$!
_n=0
while [ "$_n" -lt "$secs" ] && kill -0 "$pid" 2>/dev/null; do
	sleep 1
	_n=$((_n + 1))
done
kill "$pid" 2>/dev/null
wait "$pid" 2>/dev/null

for _tr in "$SYSFS_ROOT"/kernel/debug/remoteproc/*/trace*; do
	[ -f "$_tr" ] || continue
	echo "# remoteproc trace: $(basename "$(dirname "$_tr")")/$(basename "$_tr")" >>"$out"
	cat "$_tr" >>"$out" 2>/dev/null
done
echo "OK log $out"
