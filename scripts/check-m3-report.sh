#!/bin/sh
# Structural check of the M3 (Verdin AM62P) board report.
# Usage: check-m3-report.sh [REPORT]   (default docs/m3-am62p-report.md)
#
# Checks:
#   - a YYYY-MM-DD date;
#   - 8 console lines "SCENE N ..." (N = 1..8, at line start, after optional
#     markdown quoting/indent) and a "RESULT PASS|FAIL" line;
#   - every image or file linked as [..](path) exists (relative to the report
#     or the repository root; http(s) and #anchors are skipped);
#   - per scene a verdict line mentioning "scene N" and "camera-check"
#     followed by PASS, FAIL, SKIP or BLOCKED;
#   - board, BSP, kernel and panel each as a heading or a "Label:" line;
#   - a heading with "restore" or "Torizon" whose section has "Torizon" and
#     a YYYY-MM-DD date (restore and Torizon confirmation);
#   - no IPv4/IPv6 address, MAC address, /dev/tty path or user@host string
#     (same patterns as check-no-board-secrets.sh, applied to the whole file).
# A missing report is not an error: "no report yet", exit 0.
# Exit: 0 ok, 1 problems found.
set -u

root=$(cd "$(dirname "$0")/.." && pwd)
report=${1:-$root/docs/m3-am62p-report.md}

if [ ! -f "$report" ]; then
    echo "check-m3-report: no report yet ($report)"
    exit 0
fi
dir=$(dirname "$report")
status=0
fail() {
    echo "check-m3-report: $report: $*" >&2
    status=1
}
date_re='(^|[^0-9])[0-9]{4}-[0-9]{2}-[0-9]{2}([^0-9]|$)'

grep -Eq "$date_re" "$report" || fail "no date in YYYY-MM-DD form"

n=1
while [ "$n" -le 8 ]; do
    grep -Eq "^[ >\`|*-]*SCENE $n( |\$)" "$report" ||
        fail "missing console line 'SCENE $n ...'"
    grep -Eiq "scene[ _-]*$n([^0-9].*)?camera-check.*(PASS|FAIL|SKIP|BLOCKED)" "$report" ||
        fail "scene $n has no camera-check verdict line (scene $n ... camera-check ... PASS|FAIL|SKIP|BLOCKED)"
    n=$((n + 1))
done
grep -Eq 'RESULT (PASS|FAIL)' "$report" || fail "missing 'RESULT PASS|FAIL' line"

# linked images and files
links=$(grep -Eo '\]\([^)[:space:]]+' "$report" | sed 's/^](//')
while IFS= read -r l; do
    [ -n "$l" ] || continue
    p=${l%%#*}
    case $p in
    '' | http://* | https://* | mailto:*) continue ;;
    esac
    [ -e "$dir/$p" ] || [ -e "$root/$p" ] || fail "linked file does not exist: $p"
done <<EOF2
$links
EOF2

# board facts
for label in board bsp kernel panel; do
    grep -Eiq "^[ >#*\`|-]*$label([^A-Za-z]|\$)" "$report" ||
        fail "no '$label' section or '$label:' line"
done

# restore and Torizon confirmation
sec=$(awk '
    /^#/ { on = (tolower($0) ~ /restor|torizon/); if (on) print; next }
    on { print }' "$report")
if [ -z "$sec" ]; then
    fail "no restore / Torizon confirmation section"
else
    printf '%s\n' "$sec" | grep -qi 'torizon' || fail "restore section does not mention Torizon"
    printf '%s\n' "$sec" | grep -Eq "$date_re" || fail "restore / Torizon section has no date"
fi

# board secrets (patterns of check-no-board-secrets.sh)
hit() { # kind regex
    out=$(grep -En -e "$2" "$report" | grep -v 'secrets-ok') || return 0
    [ -n "$out" ] || return 0
    printf '%s\n' "$out" | sed "s/^/$1: line /" >&2
    fail "$1 found"
}
hit ipv4 '(^|[^0-9.])([0-9]{1,3}\.){3}[0-9]{1,3}($|[^0-9])'
hit ipv6 '(^|[^0-9A-Za-z:])([0-9A-Fa-f]{1,4}:){3,7}[0-9A-Fa-f]{1,4}($|[^0-9A-Za-z:])|(^|[^0-9A-Za-z:])([0-9A-Fa-f]{1,4}:){1,6}:[0-9A-Fa-f]{1,4}'
hit mac '(^|[^0-9A-Fa-f:-])(([0-9A-Fa-f]{2}:){5}|([0-9A-Fa-f]{2}-){5})[0-9A-Fa-f]{2}($|[^0-9A-Fa-f:-])'
hit tty '/dev/(tty[A-Za-z0-9_]+|serial/by-id)'
# user@host, minus git@github.com, noreply and placeholders
uh=$(grep -En '[A-Za-z0-9_][A-Za-z0-9._-]*@[A-Za-z][A-Za-z0-9-]*' "$report" |
    grep -viE 'git@github\.com|noreply|@example\.|user@host|secrets-ok') || uh=
if [ -n "$uh" ]; then
    printf '%s\n' "$uh" | sed 's/^/user@host: line /' >&2
    fail "user@host string found"
fi

[ "$status" = 0 ] && echo "check-m3-report: ok ($report)"
exit "$status"
