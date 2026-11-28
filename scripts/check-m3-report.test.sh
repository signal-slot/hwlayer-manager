#!/bin/sh
# Self-test of check-m3-report.sh: the good fixture passes, every failing case
# (a mutation of the good fixture) fails with its message.
# Fixtures: scripts/testdata/m3-report/.
set -u
here=$(cd "$(dirname "$0")" && pwd)
chk=$here/check-m3-report.sh
td=$here/testdata/m3-report
tmp=$(mktemp -d "${TMPDIR:-.}/m3rt.XXXXXX") || exit 2
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
cp "$td/scene1.png" "$tmp/"
rc=0
bad() {
    echo "FAIL: $*" >&2
    rc=1
}

sh "$chk" "$td/good.md" >/dev/null 2>&1 || bad "good fixture rejected"
out=$(sh "$chk" "$tmp/nope.md" 2>&1) || bad "missing report must exit 0"
case $out in
*"no report yet"*) ;;
*) bad "missing report message" ;;
esac

# case_ NAME SED-EXPR EXPECTED-MESSAGE
case_() {
    sed -e "$2" "$td/good.md" >"$tmp/$1.md"
    out=$(sh "$chk" "$tmp/$1.md" 2>&1) && bad "$1 accepted"
    case $out in
    *"$3"*) ;;
    *) bad "$1: message lacks '$3': $out" ;;
    esac
}
case_ nodate 's/2026-12-01/some day/g' 'no date'
case_ noscene 's/^SCENE 5 /SCENE X /' "missing console line 'SCENE 5"
case_ noresult 's/^RESULT PASS/DONE/' "RESULT"
case_ nolink 's/scene1.png/gone.png/' 'does not exist: gone.png'
case_ noverdict 's/^- Scene 3: camera-check PASS/- Scene 3: looked fine/' 'scene 3 has no camera-check'
case_ nopanel 's/^- Panel:.*//' "no 'panel'"
case_ nokernel 's/^- Kernel:.*//' "no 'kernel'"
case_ nobsp 's/^- BSP:.*//' "no 'bsp'"
case_ noboard 's/^- Board:.*//; s/^## Board, /## /' "no 'board'"
case_ notorizon 's/^## Restore and Torizon confirmation/## Cleanup/' 'no restore'
case_ restoredate 's/confirmed 2026-12-01/confirmed/' 'restore / Torizon section has no date'
case_ ipv4 's/^- Board: .*/- Board: Verdin AM62P at 10.20.30.40/' 'ipv4'
case_ ipv6 's/^- Board: .*/- Board: fe80:0:0:0:1:2:3:4/' 'ipv6'
case_ mac 's/^- Board: .*/- Board: aa:bb:cc:dd:ee:ff/' 'mac'
case_ tty 's/^- Board: .*/- Board: console on \/dev\/ttyUSB0/' 'tty'
case_ userhost 's/^- Board: .*/- Board: root@verdinhost/' 'user@host'

[ "$rc" = 0 ] && echo "check-m3-report.test: ok"
exit "$rc"
