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

if [ "$fails" -eq 0 ]; then echo "all passed"; else echo "$fails failed"; exit 1; fi
