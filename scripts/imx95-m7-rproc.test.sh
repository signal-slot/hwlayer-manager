#!/bin/sh
# Host test for imx95-m7-rproc.sh against a fake sysfs tree. No board access.
# A background loop turns writes to the fake "state" file into transitions.

here=$(cd "$(dirname "$0")" && pwd)
script=$here/imx95-m7-rproc.sh
T=$(mktemp -d "${TMPDIR:-/tmp}/rproc-test.XXXXXX") || exit 1
simpid=
trap '[ -n "$simpid" ] && kill "$simpid" 2>/dev/null; rm -rf "$T"' EXIT
trap 'exit 1' INT TERM

ROOT=$T/root
RP=$ROOT/sys/class/remoteproc/remoteproc3
PARAM=$ROOT/sys/module/firmware_class/parameters/path
mkdir -p "$RP" "$ROOT/sys/module/firmware_class/parameters" "$T/fw" "$T/bin"
: > "$T/fw/m7.elf"
printf 'fake-m7\n' > "$RP/name"
printf 'dmesg 1\ndmesg 2\n' > "$T/dmesg.txt"
printf '#!/bin/sh\ncat "%s/dmesg.txt"\n' "$T" > "$T/bin/dmesg"
chmod +x "$T/bin/dmesg"

# mode: ok | fail (core crashes on start) | hang (never leaves "start")
sim() {
	while :; do
		s=$(cat "$RP/state")
		case $s in
		start)
			echo "fw=$(cat "$RP/firmware") path=$(cat "$PARAM")" > "$T/seen"
			case $(cat "$T/mode") in
			ok) echo running > "$RP/state"; echo "remoteproc3: m7 up" >> "$T/dmesg.txt" ;;
			fail) echo crashed > "$RP/state" ;;
			esac ;;
		stop) echo offline > "$RP/state" ;;
		esac
		sleep 0.05
	done
}
sim &
simpid=$!

fails=0
check() { # desc cmd...
	d=$1; shift
	if "$@"; then echo "ok   $d"; else echo "FAIL $d"; fails=$((fails + 1)); fi
}
has() { grep -q "$1" "$T/out"; }
reset() { # state mode
	printf '%s\n' "$1" > "$RP/state"
	printf '%s\n' "$2" > "$T/mode"
	printf 'orig.elf\n' > "$RP/firmware"
	printf '/lib/firmware\n' > "$PARAM"
	rm -f "$T/seen"
}
run() {
	ROOT=$ROOT RPROC_FW_ROOT=$T RPROC_POLL=0.05 RPROC_POLLS_PER_SEC=20 \
		DMESG=$T/bin/dmesg sh "$script" --timeout 1 --run-secs 0 "$@" > "$T/out" 2>&1
	rcode=$?
}
rc_is() { [ "$rcode" -eq "$1" ]; }
restored() {
	[ "$(cat "$RP/firmware")" = orig.elf ] &&
		[ "$(cat "$PARAM")" = /lib/firmware ] &&
		[ "$(cat "$RP/state")" = offline ] &&
		has 'restored: core=ok firmware=ok firmware_class.path=ok'
}
untouched() { # state
	[ "$(cat "$RP/state")" = "$1" ] && [ "$(cat "$RP/firmware")" = orig.elf ] &&
		[ "$(cat "$PARAM")" = /lib/firmware ] && [ ! -e "$T/seen" ]
}

# 1. normal load/stop, selected by index; log file captured
reset offline ok
printf 'hello from m7\n' > "$T/rpmsg.log"
run --log "$T/rpmsg.log" "$T/fw/m7.elf" 3
check "normal: exit 0" rc_is 0
check "normal: fw and path set at start" grep -q "fw=m7.elf path=$T/fw" "$T/seen"
check "normal: dmesg since start only" sh -c "grep -q 'dmesg: remoteproc3: m7 up' $T/out && ! grep -q 'dmesg: dmesg 1' $T/out"
check "normal: log captured" has 'log: hello from m7'
check "normal: restored" restored

# 2. selection by name
reset offline ok
run "$T/fw/m7.elf" fake-m7
check "by name: exit 0" rc_is 0
check "by name: restored" restored

# 3. failed start (core crashes)
reset offline fail
run "$T/fw/m7.elf" 3
check "failed start: exit 1" rc_is 1
check "failed start: restored" restored

# 4. start never completes (timeout)
reset offline hang
run "$T/fw/m7.elf" 3
check "timeout: exit 1" rc_is 1
check "timeout: restored" restored

# 5. refusal when already running: nothing changed
reset running ok
run "$T/fw/m7.elf" 3
check "running: refused (3)" rc_is 3
check "running: untouched" untouched running

# 6. --force-stop when running
reset running ok
run --force-stop "$T/fw/m7.elf" 3
check "force-stop: exit 0" rc_is 0
check "force-stop: recorded state" has 'before: state=running'
check "force-stop: restored" restored

# 7. input validation changes nothing
reset offline ok
run /etc/passwd 3
check "path outside root rejected" rc_is 2
run "$T/fw/../fw/m7.elf" 3
check ".. rejected" rc_is 2
run "$T/fw/nope.elf" 3
check "missing ELF rejected" rc_is 2
run "$T/fw/m7.elf" 9
check "unknown remoteproc rejected" rc_is 2
check "validation: nothing changed" untouched offline

# 8. start / stop subcommands (SYSFS_ROOT, fake systemctl)
SD=$T/state
printf '#!/bin/sh\ncat "%s/weston"\n' "$T" > "$T/bin/systemctl"
chmod +x "$T/bin/systemctl"
sub() { # args...
	SYSFS_ROOT=$ROOT/sys SYSTEMCTL=$T/bin/systemctl M7_STATE_DIR=$SD \
		RPROC_FW_ROOT=$T RPROC_POLL=0.05 RPROC_POLLS_PER_SEC=20 M7_TIMEOUT=1 \
		sh "$script" "$@" > "$T/out" 2>&1
	rcode=$?
}
sub_restored() {
	[ "$(cat "$RP/firmware")" = orig.elf ] && [ "$(cat "$PARAM")" = /lib/firmware ] &&
		[ "$(cat "$RP/state")" = offline ] && [ ! -e "$SD/state" ] && [ ! -e "$SD/fw" ]
}
seen_is() { [ "$(cat "$T/seen")" = "fw=m7.elf path=$SD/fw" ]; }
fresh() { rm -rf "$SD"; reset offline ok; echo active > "$T/weston"; }

fresh
sub start "$T/fw/m7.elf"
check "sub start: exit 0" rc_is 0
check "sub start: sees firmware and path" seen_is
check "sub start: image copied" test -f "$SD/fw/m7.elf"
check "sub start: running" test "$(cat "$RP/state")" = running
check "sub start: state file" grep -q '^orig_path=/lib/firmware$' "$SD/state"
sub stop
check "sub stop: exit 0" rc_is 0
check "sub stop: restored firmware, path, state" sub_restored
check "sub stop: verdict lines" has 'OK firmware-class-path-restored'
check "sub stop: firmware verdict" has 'OK firmware-restored'
check "sub stop: weston verdict" has 'OK weston-active'

fresh
sub start "$T/fw/m7.elf"
echo inactive > "$T/weston"
sub stop
check "sub stop: weston down is FAIL (1)" rc_is 1
check "sub stop: FAIL weston line" has 'FAIL weston-active'
check "sub stop: still restored" test "$(cat "$PARAM")" = /lib/firmware

fresh
printf 'm33-core\n' > "$RP/name"
sub start "$T/fw/m7.elf"
check "sub start: no M7 remoteproc (3)" rc_is 3
check "sub start: no M7 changed nothing" untouched offline
printf 'fake-m7\n' > "$RP/name"

fresh
printf 'running\n' > "$RP/state"
sub start "$T/fw/m7.elf"
check "sub start: already running (4)" rc_is 4
check "sub start: running untouched" untouched running

fresh
sub start "$T/fw/m7.elf"
sub start "$T/fw/m7.elf"
check "sub start: twice refused (5)" rc_is 5
sub stop

fresh
sub stop
check "sub stop: without start (1)" rc_is 1

fresh
sub start /etc/passwd
check "sub start: image outside root (2)" rc_is 2
sub start "$T/fw/../fw/m7.elf"
check "sub start: .. rejected (2)" rc_is 2
SD=/var/tmp/outside-state
sub start "$T/fw/m7.elf"
check "sub start: state dir outside root (2)" rc_is 2
check "sub start: nothing created outside" test ! -e "$SD"
SD=$T/state

fresh
printf 'fail\n' > "$T/mode"
sub start "$T/fw/m7.elf"
check "sub start: crash exits 1" rc_is 1
check "sub start: crash rolled back" sub_restored

if [ "$fails" -eq 0 ]; then echo "all passed"; else echo "$fails failed"; exit 1; fi
