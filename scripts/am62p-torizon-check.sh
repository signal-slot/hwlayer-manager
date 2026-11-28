#!/bin/sh
# Read-only check over ssh that a Verdin AM62P is running Torizon.
#
# usage: am62p-torizon-check.sh [DEST]
#   DEST  ssh destination; default: the environment variable AM62P_SSH_DEST.
#         It comes from the lease and is never printed.
# Environment: SSH  ssh command (default ssh; may carry options, split on
#                   spaces). BatchMode and a connect timeout are added.
#
# One ssh connection runs read-only commands only (date -u, os-release,
# uname -r, uptime, ostree admin status, systemctl is-system-running, the
# remoteproc name/state/firmware files). Output is "KEY value" lines, then
# exactly one verdict line:
#   TORIZON UP                 os-release names Torizon and systemd is
#                              running or degraded
#   TORIZON NOT-UP <reason>
# IPv4 addresses and serial-number lines are removed from the output; the
# remote command prints no host name, and the destination is never printed.
# Exit: 0 only on TORIZON UP, 1 not up, 2 usage.

dest=${1:-${AM62P_SSH_DEST:-}}
if [ -z "$dest" ] || [ $# -gt 1 ]; then
	echo "usage: am62p-torizon-check.sh [DEST]  (or set AM62P_SSH_DEST)" >&2
	echo "TORIZON NOT-UP no destination given"
	exit 2
fi
SSH=${SSH:-ssh}

# Runs on the board. Reads only; writes nothing.
remote='
osv() { sed -n "s/^$1=//p" /etc/os-release 2>/dev/null | head -n 1 | tr -d "\"" ; }
echo "DATE $(date -u +%Y-%m-%dT%H:%M:%SZ)"
for k in ID NAME VERSION_ID PRETTY_NAME VARIANT; do echo "OS_$k $(osv $k)"; done
echo "KERNEL $(uname -r)"
echo "UPTIME $(uptime)"
if command -v ostree >/dev/null 2>&1; then
	ostree admin status 2>&1 | sed "s/^/OSTREE /"
fi
echo "SYSTEMD $(systemctl is-system-running 2>/dev/null)"
for d in /sys/class/remoteproc/remoteproc*; do
	[ -d "$d" ] || continue
	echo "REMOTEPROC ${d##*/} name $(cat "$d/name" 2>/dev/null) state $(cat "$d/state" 2>/dev/null) firmware $(cat "$d/firmware" 2>/dev/null)"
done
echo "END"
'

nu() { echo "TORIZON NOT-UP $1"; exit 1; }

# shellcheck disable=SC2086
raw=$($SSH -o BatchMode=yes -o ConnectTimeout=10 "$dest" "$remote" 2>/dev/null) ||
	nu "ssh failed"
case $raw in
*"
END") ;;
*) nu "incomplete answer" ;;
esac

# scrub: IPv4 addresses and serial lines
out=$(printf '%s\n' "$raw" | sed '/^END$/d' |
	sed -e 's/[0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*/<ip>/g' |
	grep -vi 'serial')
printf '%s\n' "$out"

os=$(printf '%s\n' "$out" | grep -E '^OS_(ID|NAME|PRETTY_NAME) ' | tr 'A-Z' 'a-z')
case $os in
*torizon*) ;;
*) nu "os-release does not name Torizon" ;;
esac
st=$(printf '%s\n' "$out" | sed -n 's/^SYSTEMD //p' | head -n 1)
case $st in
running | degraded) ;;
*) nu "system state '${st:-unknown}'" ;;
esac
echo "TORIZON UP"
exit 0
