#!/bin/sh
# Host test for am62p-r5f-run.sh against a fake sysfs/debugfs tree and a fake
# systemctl. No board access. A background loop turns writes to the fake
# remoteproc "state" file into transitions and fills the fake trace buffer.

here=$(cd "$(dirname "$0")" && pwd)
script=$here/am62p-r5f-run.sh
T=$(mktemp -d "${TMPDIR:-/tmp}/am62p-r5f-test.XXXXXX") || exit 1
simpid=
trap '[ -n "$simpid" ] && kill "$simpid" 2>/dev/null; rm -rf "$T"' EXIT
trap 'exit 1' INT TERM

ROOT=$T/root
RP=$ROOT/sys/class/remoteproc/remoteproc1
PARAM=$ROOT/sys/module/firmware_class/parameters/path
DRV=$ROOT/sys/bus/platform/drivers/display-drv
TR=$ROOT/sys/kernel/debug/remoteproc/remoteproc1/trace0
SD=$T/state
logf=$SD/log
img=$T/fw/r5f.elf
mkdir -p "$RP" "$ROOT/sys/module/firmware_class/parameters" "$DRV" \
	"$ROOT/sys/kernel/debug/remoteproc/remoteproc1" "$T/fw" "$T/bin" "$T/units"
: > "$img"
printf 'mcu-r5f0\n' > "$RP/name"
printf 'dmesg 1\ndmesg 2\n' > "$T/dmesg.txt"
printf '#!/bin/sh\ncat "%s/dmesg.txt"\n' "$T" > "$T/bin/dmesg"
cat > "$T/bin/systemctl" <<EOF
#!/bin/sh
f=$T/units/\$2
case \$2 in
missing)
	case \$1 in
	stop | start) exit 1 ;;
	*) echo unknown; exit 0 ;;
	esac
	;;
esac
case \$1 in
is-active) [ -f "\$f" ] && cat "\$f" || echo inactive ;;
stop) echo inactive > "\$f" ;;
start) echo active > "\$f" ;;
esac
EOF
: > "$DRV/unbind"
: > "$DRV/bind"
chmod +x "$T/bin/dmesg" "$T/bin/systemctl"

# mode: ok (SCENE and RESULT lines) | quiet (no RESULT) | crash (core crashes)
# The loop also dispatches writes to the fake bind/unbind attributes: a write
# to one of them (a device name) binds or unbinds, and the attribute empties,
# as a sysfs attribute would.
sim() {
	while :; do
		for _f in "$DRV/unbind" "$DRV/bind"; do
			_l=$(head -n 1 "$_f" 2>/dev/null | tr -d ' \t\r')
			[ -n "$_l" ] || continue
			: > "$_f"
			case $_f in
			*unbind) rm -f "$DRV/$_l" ;;
			*) : > "$DRV/$_l" ;;
			esac
		done
		s=$(cat "$RP/state" 2>/dev/null)
		case $s in
		start)
			echo "fw=$(cat "$RP/firmware") path=$(cat "$PARAM")" > "$T/seen"
			case $(cat "$T/mode") in
			ok)
				echo running > "$RP/state"
				echo "SCENE 1 two-plane" >> "$TR"
				echo "RESULT scene=1 rc=0" >> "$TR"
				echo "remoteproc1: r5f up" >> "$T/dmesg.txt"
				;;
			quiet)
				echo running > "$RP/state"
				echo "SCENE 1 two-plane" >> "$TR"
				;;
			crash)
				echo crashed > "$RP/state"
				echo "remoteproc1: crash detected" >> "$T/dmesg.txt"
				;;
			esac
			;;
		esac
		sleep 0.02
	done
}

fails=0
check() { # desc cmd...
	d=$1
	shift
	if "$@"; then echo "ok   $d"; else echo "FAIL $d"; fails=$((fails + 1)); fi
}
has() { grep -q "$1" "$T/out"; }
hasnt() { ! grep -q "$1" "$T/out"; }
reset_all() { # mode
	printf 'offline\n' > "$RP/state"
	printf 'orig-fw\n' > "$RP/firmware"
	printf '/lib/firmware\n' > "$PARAM"
	printf 'active\n' > "$T/units/weston"
	printf 'inactive\n' > "$T/units/other-unit"
	: > "$TR"
	printf '%s\n' "$1" > "$T/mode"
	printf 'dmesg 1\ndmesg 2\n' > "$T/dmesg.txt"
	: > "$DRV/display-dev"
	: > "$DRV/unbind"
	: > "$DRV/bind"
	rm -rf "$SD"
	rm -f "$T/seen"
}
printf 'offline\n' > "$RP/state"
printf 'orig-fw\n' > "$RP/firmware"
printf '/lib/firmware\n' > "$PARAM"
printf 'active\n' > "$T/units/weston"
printf 'inactive\n' > "$T/units/other-unit"
: > "$TR"
printf 'ok\n' > "$T/mode"
: > "$DRV/display-dev"
sim &
simpid=$!

run() { # args... (main path, short polls, capture ends on RESULT or 200 copies)
	ROOT=$ROOT RPROC_FW_ROOT=$T AM62P_STATE_DIR=$SD RPROC_POLL=0.05 \
		RPROC_POLLS_PER_SEC=20 AM62P_TRACE_POLL=0.05 AM62P_TRACE_POLLS=200 \
		SYSTEMCTL=$T/bin/systemctl DMESG=$T/bin/dmesg \
		sh "$script" --timeout 1 --run-secs 60 --interval 1 "$@" > "$T/out" 2>&1
	rcode=$?
}
sub() { # args... (restore subcommand: no shared option prefix)
	ROOT=$ROOT RPROC_FW_ROOT=$T SYSTEMCTL=$T/bin/systemctl DMESG=$T/bin/dmesg \
		sh "$script" "$@" > "$T/out" 2>&1
	rcode=$?
}
stopfile=$T/stop.list
printf 'stop weston\nunbind display-drv display-dev\nstop other-unit\n' > "$stopfile"
rc_is() { [ "$rcode" -eq "$1" ]; }
untouched() {
	[ "$(cat "$RP/state")" = offline ] && [ "$(cat "$RP/firmware")" = orig-fw ] &&
		[ "$(cat "$PARAM")" = /lib/firmware ] && [ -e "$DRV/display-dev" ] &&
		[ "$(cat "$T/units/weston")" = active ] && [ ! -e "$SD/state" ] &&
		[ ! -e "$T/seen" ]
}

# 1. normal run with a stop file: stop, unbind, start, capture until RESULT
reset_all ok
run "$img" 1 "$stopfile"
check "normal: exit 0" rc_is 0
check "normal: firmware and path set at start" grep -q "fw=r5f.elf path=$T/fw" "$T/seen"
check "normal: RESULT ends capture" has 'result: RESULT line seen'
check "normal: SCENE and RESULT in log" sh -c "grep -q '^RESULT' '$logf' && grep -q '^SCENE' '$logf'"
check "normal: trace echoed live" has 'trace: RESULT scene=1 rc=0'
check "normal: dmesg since start only" sh -c "grep -q 'dmesg: remoteproc1: r5f up' $T/out && ! grep -q 'dmesg: dmesg 1' $T/out"
check "normal: firmware_class.path restored" sh -c "[ \"\$(cat '$PARAM')\" = /lib/firmware ]"
check "normal: path restored, name kept (core runs)" has 'restored: firmware=kept firmware_class.path=restored'
check "normal: firmware name still the image" sh -c "[ \"\$(cat '$RP/firmware')\" = r5f.elf ]"
check "normal: core left running (never stopped)" sh -c "[ \"\$(cat '$RP/state')\" = running ]"
check "normal: run never starts or rebinds" hasnt 'OK start:'
check "normal: run never rebinds" hasnt 'OK rebind:'
check "normal: weston stopped" sh -c "[ \"\$(cat '$T/units/weston')\" = inactive ]"
check "normal: device unbound" sh -c "[ ! -e '$DRV/display-dev' ]"
check "normal: state file records weston" grep -q '^unit weston active$' "$SD/state"
check "normal: state file records other-unit" grep -q '^unit other-unit inactive$' "$SD/state"
check "normal: state file records binding" grep -q "^bind $DRV display-dev bound\$" "$SD/state"
check "normal: state file records originals" sh -c "grep -q '^orig_path=/lib/firmware\$' '$SD/state' && grep -q '^orig_fw=orig-fw\$' '$SD/state'"
check "normal: state path printed" has "state: $SD/state"
check "normal: log path printed" has "log: $logf"

# 2. restore STATEFILE: rebind and restart what was stopped
sub restore "$SD/state"
check "restore: exit 0" rc_is 0
check "restore: rebinds device" has 'OK rebind: display-dev'
check "restore: device bound again" sh -c "[ -e '$DRV/display-dev' ]"
check "restore: starts active unit" has 'OK start: weston'
check "restore: weston active again" sh -c "[ \"\$(cat '$T/units/weston')\" = active ]"
check "restore: skips inactive unit" has 'skip start: other-unit (was inactive)'
check "restore: does not start the inactive unit" hasnt 'OK start: other-unit'
check "restore: state file removed" sh -c "[ ! -e '$SD/state' ]"

# 3. selection by remoteproc name, no stop file
reset_all ok
run "$img" mcu-r5f0
check "by name: exit 0" rc_is 0
check "by name: core running" sh -c "[ \"\$(cat '$RP/state')\" = running ]"

# 4. trace capture runs out of copies without RESULT
reset_all quiet
ROOT=$ROOT RPROC_FW_ROOT=$T AM62P_STATE_DIR=$SD RPROC_POLL=0.05 \
	RPROC_POLLS_PER_SEC=20 AM62P_TRACE_POLL=0.05 AM62P_TRACE_POLLS=3 \
	SYSTEMCTL=$T/bin/systemctl DMESG=$T/bin/dmesg \
	sh "$script" --timeout 1 --run-secs 60 --interval 1 "$img" 1 > "$T/out" 2>&1
rcode=$?
check "quiet: exit 0" rc_is 0
check "quiet: note about missing RESULT" has 'note: no RESULT line'
check "quiet: SCENE captured" grep -q '^SCENE' "$logf"
check "quiet: restored" has 'restored: firmware=kept firmware_class.path=restored'

# 5. core crashes on start
reset_all crash
run "$img" 1
check "crash: exit 1" rc_is 1
check "crash: not running reported" has 'start: not running'
check "crash: dmesg shown" has 'dmesg: remoteproc1: crash detected'
check "crash: path restored" sh -c "[ \"\$(cat '$PARAM')\" = /lib/firmware ]"
check "crash: firmware kept (crashed)" has 'restored: firmware=kept firmware_class.path=restored'
check "crash: state file kept for restore" test -f "$SD/state"

# 6. refusals and bad usage change nothing
reset_all ok
run /etc/passwd 1 "$stopfile"
check "image outside /tmp refused (2)" rc_is 2
run "$T/fw/../fw/r5f.elf" 1
check ".. in image path refused (2)" rc_is 2
run "$T/fw/nope.elf" 1
check "missing ELF refused (2)" rc_is 2
run "$img" 9
check "unknown remoteproc refused (2)" rc_is 2
run --log /etc/evil.log "$img" 1
check "log outside /tmp refused (2)" rc_is 2
printf 'frobnicate x\n' > "$T/bad1.list"
run "$img" 1 "$T/bad1.list"
check "bad stop file line refused (2)" rc_is 2
printf 'unbind nosuchdrv x\n' > "$T/bad2.list"
run "$img" 1 "$T/bad2.list"
check "unknown driver refused (2)" rc_is 2
printf 'running\n' > "$RP/state"
run "$img" 1 "$stopfile"
check "core not offline refused (3)" rc_is 3
printf 'offline\n' > "$RP/state"
check "refusals: nothing changed" untouched
printf 'offline\n' > "$RP/state"
mkdir -p "$SD"
printf 'stale=1\n' > "$SD/state"
run "$img" 1 "$stopfile"
check "previous run not restored refused (5)" rc_is 5
check "stale state file kept" sh -c "[ \"\$(cat '$SD/state')\" = 'stale=1' ]"
rm -rf "$SD"

# 7. a failing stop keeps the core untouched and the state file for restore
reset_all ok
printf 'stop weston\nstop missing\n' > "$T/partial.list"
run "$img" 1 "$T/partial.list"
check "failing stop: exit 1" rc_is 1
check "failing stop: weston was stopped" sh -c "[ \"\$(cat '$T/units/weston')\" = inactive ]"
check "failing stop: core untouched" sh -c "[ \"\$(cat '$RP/state')\" = offline ] && [ \"\$(cat '$PARAM')\" = /lib/firmware ] && [ ! -e '$T/seen' ]"
check "failing stop: state file kept" grep -q '^unit weston active$' "$SD/state"
sub restore "$SD/state"
check "failing stop: exit 0" rc_is 0
check "failing stop: restore restarts weston" sh -c "[ \"\$(cat '$T/units/weston')\" = active ]"

# 8. restore argument validation
reset_all ok
sub restore "$SD/nothing"
check "restore: missing file (2)" rc_is 2
sub restore /etc/passwd
check "restore: outside /tmp (2)" rc_is 2
sub restore /etc/../state
check "restore: .. in path (2)" rc_is 2
sub restore
check "restore: no argument (2)" rc_is 2

# 9. a planted state file or state directory is not trusted
reset_all ok
mkdir -p "$SD" "$T/victim"
printf 'bind %s evil bound\nunit ../evil active\n' "$T/victim" > "$SD/state"
sub restore "$SD/state"
check "restore: foreign driver path refused (1)" rc_is 1
check "restore: nothing written outside sys" sh -c "[ ! -e '$T/victim/bind' ]"
rm -rf "$SD"
mkdir -p "$SD" "$T/elsewhere"
ln -s "$T/elsewhere/s" "$SD/state"
: > "$T/elsewhere/s"
sub restore "$SD/state"
check "restore: symlinked state file refused (2)" rc_is 2
rm -rf "$SD"
mkdir -p "$T/realdir"
ln -s "$T/realdir" "$SD"
run "$img" 1 "$stopfile"
check "start: symlinked state dir refused (1)" rc_is 1
check "start: nothing written through symlinked dir" sh -c "[ ! -e '$T/realdir/state' ]"
rm -f "$SD"
mkdir -p "$SD"
: > "$T/elsewhere/log"
ln -s "$T/elsewhere/log" "$SD/log"
printf 'keep\n' > "$T/elsewhere/log"
run "$img" 1
check "start: symlinked log refused (1)" rc_is 1
check "start: symlinked log target intact" sh -c "[ \"\$(cat '$T/elsewhere/log')\" = keep ]"
check "start: refused log leaves core untouched" sh -c "[ \"\$(cat '$RP/state')\" = offline ]"
rm -rf "$SD"

if [ "$fails" -eq 0 ]; then echo "all passed"; else echo "$fails failed"; exit 1; fi
