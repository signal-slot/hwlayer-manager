#!/bin/sh
# display-guard.sh: stop a display manager or compositor only for a board
# run, then show it active again.
#
# Usage: display-guard.sh [--wait N] detect
#        display-guard.sh [--wait N] stop STATEFILE
#        display-guard.sh [--wait N] restore STATEFILE
#
#   detect   print each active display unit, one per line. The candidates are
#            a fixed list of display managers, compositors and kiosk units
#            plus display-manager.service; a unit counts when
#            `systemctl is-active` says "active".
#   stop     `systemctl stop` each active unit and record it, one per line,
#            in STATEFILE (the caller names it, e.g. under /tmp on the
#            board). Prints "DISPLAY-GUARD stopped <unit>" per unit, or
#            "DISPLAY-GUARD none" (and writes an empty STATEFILE) when
#            nothing was active. Exit 1 if a stop command failed; every unit
#            is recorded before it is stopped, so restore still covers it.
#   restore  `systemctl start` every recorded unit, then wait up to N seconds
#            (--wait, default 15) for each to be active. Prints
#            "DISPLAY-GUARD restored <unit> active" or
#            "DISPLAY-GUARD restore-failed <unit> <state>" per unit; exit 0
#            only if all are active. An empty STATEFILE prints
#            "DISPLAY-GUARD none" and exits 0. A missing STATEFILE prints
#            "DISPLAY-GUARD no-state-file" and exits 2 (nothing can be
#            shown to be active again, so it is not reported as success).
#
# The helper only runs systemctl is-active, stop and start, on units that
# were already active. It never enables, disables, masks or edits units, and
# never touches DRM, udev or seat configuration.

CANDIDATES="display-manager.service gdm.service gdm3.service lightdm.service
sddm.service lxdm.service xdm.service greetd.service weston.service
cage.service cage@tty1.service cog.service sway.service plasma-kiosk.service
kiosk.service"

wait_s=15

die() { echo "display-guard: $*" >&2; exit 2; }

detect() {
	for u in $CANDIDATES; do
		if [ "$(systemctl is-active "$u" 2>/dev/null)" = active ]; then
			echo "$u"
		fi
	done
	return 0
}

while [ $# -gt 0 ]; do
	case $1 in
	--wait)
		[ $# -ge 2 ] || die "--wait needs a number"
		wait_s=$2
		shift 2 ;;
	--wait=*)
		wait_s=${1#--wait=}
		shift ;;
	*) break ;;
	esac
done
case $wait_s in
'' | *[!0-9]*) die "--wait must be a whole number of seconds" ;;
esac

cmd=${1:-}
case $cmd in
detect)
	detect
	;;
stop)
	state=${2:-}
	[ -n "$state" ] || die "stop needs a STATEFILE"
	units=$(detect)
	: > "$state" || die "cannot write $state"
	if [ -z "$units" ]; then
		echo "DISPLAY-GUARD none"
		exit 0
	fi
	rc=0
	for u in $units; do
		# Record first: a half-failed stop must still be restored.
		echo "$u" >> "$state"
		if systemctl stop "$u"; then
			echo "DISPLAY-GUARD stopped $u"
		else
			echo "DISPLAY-GUARD stop-failed $u"
			rc=1
		fi
	done
	exit $rc
	;;
restore)
	state=${2:-}
	[ -n "$state" ] || die "restore needs a STATEFILE"
	if [ ! -f "$state" ]; then
		echo "DISPLAY-GUARD no-state-file"
		exit 2
	fi
	units=$(cat "$state")
	if [ -z "$units" ]; then
		echo "DISPLAY-GUARD none"
		exit 0
	fi
	for u in $units; do
		systemctl start "$u" || :
	done
	rc=0
	for u in $units; do
		i=0
		while :; do
			st=$(systemctl is-active "$u" 2>/dev/null)
			[ "$st" = active ] && break
			[ "$i" -ge "$wait_s" ] && break
			sleep 1
			i=$((i + 1))
		done
		if [ "$st" = active ]; then
			echo "DISPLAY-GUARD restored $u active"
		else
			echo "DISPLAY-GUARD restore-failed $u ${st:-unknown}"
			rc=1
		fi
	done
	exit $rc
	;;
*)
	die "usage: display-guard.sh [--wait N] detect|stop STATEFILE|restore STATEFILE"
	;;
esac
