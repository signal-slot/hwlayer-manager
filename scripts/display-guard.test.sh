#!/bin/sh
# Host test for display-guard.sh against a mocked systemctl on PATH. The mock
# keeps one file per unit in $MOCK_DIR holding its state and logs every call
# to $MOCK_DIR/log. Units named in MOCK_NOSTART never come back on start.
# No systemd, no board.

here=$(cd "$(dirname "$0")" && pwd)
script=$here/display-guard.sh
T=$(mktemp -d "${TMPDIR:-/tmp}/display-guard-test.XXXXXX") || exit 1
trap 'rm -rf "$T"' EXIT
trap 'exit 1' INT TERM

mkdir "$T/bin" "$T/state"
cat > "$T/bin/systemctl" <<'MOCK'
#!/bin/sh
echo "$*" >> "$MOCK_DIR/log"
verb=$1
unit=$2
case $verb in
is-active)
	if [ -f "$MOCK_DIR/$unit" ]; then cat "$MOCK_DIR/$unit"; else echo inactive; fi
	[ "$(cat "$MOCK_DIR/$unit" 2>/dev/null)" = active ]
	;;
stop) echo inactive > "$MOCK_DIR/$unit" ;;
start)
	case " $MOCK_NOSTART " in
	*" $unit "*) echo failed > "$MOCK_DIR/$unit" ;;
	*) echo active > "$MOCK_DIR/$unit" ;;
	esac
	;;
*) echo "$verb" >> "$MOCK_DIR/bad"; exit 1 ;;
esac
MOCK
chmod +x "$T/bin/systemctl"
PATH=$T/bin:$PATH
MOCK_DIR=$T/state
export MOCK_DIR

fails=0
# check DESCRIPTION CONDITION...: run the condition, count a failure.
check() {
	d=$1
	shift
	if "$@"; then echo "ok   $d"; else echo "FAIL $d"; fails=$((fails + 1)); fi
}
reset() {
	rm -f "$MOCK_DIR"/* "$T/sf"
	MOCK_NOSTART=
	export MOCK_NOSTART
}
has() { printf '%s\n' "$1" | grep -qx "$2"; }
lacks() { ! has "$1" "$2"; }
eq() { [ "$1" = "$2" ]; }
is_empty() { [ ! -s "$1" ]; }
nonzero() { [ "$1" -ne 0 ]; }
state_is() { [ "$(cat "$MOCK_DIR/$1")" = "$2" ]; }

# 1. none active
reset
out=$(sh "$script" detect)
check "detect: none active prints nothing" eq "$out" ""
out=$(sh "$script" stop "$T/sf")
rc=$?
check "none: DISPLAY-GUARD none" eq "$out" "DISPLAY-GUARD none"
check "none: exit 0" eq $rc 0
check "none: empty state file written" is_empty "$T/sf"
out=$(sh "$script" restore "$T/sf")
rc=$?
check "none: restore of empty state is none" eq "$out" "DISPLAY-GUARD none"
check "none: restore exit 0" eq $rc 0

# 2. two active, one inactive: stop and restore
reset
echo active > "$MOCK_DIR/gdm.service"
echo active > "$MOCK_DIR/weston.service"
echo inactive > "$MOCK_DIR/sddm.service"
out=$(sh "$script" detect)
check "detect: lists gdm" has "$out" gdm.service
check "detect: lists weston" has "$out" weston.service
check "detect: omits inactive sddm" lacks "$out" sddm.service
out=$(sh "$script" stop "$T/sf")
rc=$?
check "stop: exit 0" eq $rc 0
check "stop: gdm inactive" state_is gdm.service inactive
check "stop: weston inactive" state_is weston.service inactive
check "stop: state file records both" eq "$(sort "$T/sf" | tr '\n' ' ')" "gdm.service weston.service "
check "stop: inactive unit never stopped" lacks "$(cat "$MOCK_DIR/log")" "stop sddm.service"
out=$(sh "$script" restore "$T/sf")
rc=$?
check "restore: exit 0" eq $rc 0
check "restore: gdm line" has "$out" "DISPLAY-GUARD restored gdm.service active"
check "restore: weston line" has "$out" "DISPLAY-GUARD restored weston.service active"
check "restore: gdm active again" state_is gdm.service active
check "no enable/disable/mask or other verb used" test ! -f "$MOCK_DIR/bad"

# 3. a restore that never comes back
reset
echo active > "$MOCK_DIR/lightdm.service"
echo active > "$MOCK_DIR/weston.service"
sh "$script" stop "$T/sf" > /dev/null
MOCK_NOSTART=lightdm.service
export MOCK_NOSTART
out=$(sh "$script" --wait 2 restore "$T/sf")
rc=$?
check "restore-failed: nonzero exit" nonzero $rc
check "restore-failed: line carries state" has "$out" "DISPLAY-GUARD restore-failed lightdm.service failed"
check "restore-failed: other unit still restored" has "$out" "DISPLAY-GUARD restored weston.service active"

# 4. missing and empty state file, bad usage
reset
out=$(sh "$script" restore "$T/nonexistent")
rc=$?
check "missing state file: no-state-file" eq "$out" "DISPLAY-GUARD no-state-file"
check "missing state file: exit 2" eq $rc 2
: > "$T/sf"
out=$(sh "$script" restore "$T/sf")
rc=$?
check "empty state file: none" eq "$out" "DISPLAY-GUARD none"
check "empty state file: exit 0" eq $rc 0
sh "$script" restore > /dev/null 2>&1
check "restore without STATEFILE fails" nonzero $?
sh "$script" --wait x detect > /dev/null 2>&1
check "non-numeric --wait fails" nonzero $?

if [ $fails -eq 0 ]; then
	echo "display-guard tests passed"
else
	echo "$fails failed"
	exit 1
fi
