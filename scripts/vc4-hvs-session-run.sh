#!/bin/bash
# vc4-hvs-session-run.sh: one-pass board session runner for the Raspberry Pi
# HVS boards (RPi5 HVS6, reTerminal CM4 HVS5). See
# scripts/vc4-hvs-session-run-README.md.
#
# Usage: BOARD_SSH='<ssh target and options>' \
#        vc4-hvs-session-run.sh [--dry-run] --bin-dir DIR --out DIR \
#            --step 'NAME:BINARY ARGS...' [--step ...]
#
# The board login comes from BOARD_SSH (optional BOARD_SUDO, e.g. "sudo -n",
# prefixes the guard and the steps). It is never written to the outputs.

set -u

here=$(cd "$(dirname "$0")" && pwd)
guard_src=$here/display-guard.sh
SSH_BIN=${SSH_BIN:-ssh}
KNOWN_BINS="probe props scale_probe pending_probe mode_test downscale_probe"

dry=0
bin_dir=
out_dir=
steps=()

die() { echo "vc4-hvs-session-run: $*" >&2; exit 2; }

while [ $# -gt 0 ]; do
	case $1 in
	--dry-run) dry=1; shift ;;
	--bin-dir) [ $# -ge 2 ] || die "--bin-dir needs a path"; bin_dir=$2; shift 2 ;;
	--out) [ $# -ge 2 ] || die "--out needs a path"; out_dir=$2; shift 2 ;;
	--step) [ $# -ge 2 ] || die "--step needs NAME:CMD"; steps+=("$2"); shift 2 ;;
	-h | --help) sed -n '2,12p' "$0"; exit 0 ;;
	*) die "unknown argument: $1" ;;
	esac
done

[ -n "$bin_dir" ] && [ -d "$bin_dir" ] || die "--bin-dir must be a directory"
[ -n "$out_dir" ] || die "--out is required"
[ ${#steps[@]} -gt 0 ] || die "at least one --step is required"
[ -f "$guard_src" ] || die "scripts/display-guard.sh not found"
if [ $dry -eq 0 ]; then
	[ -n "${BOARD_SSH:-}" ] || die "BOARD_SSH is not set"
fi

# Binaries present in bin-dir.
present=()
for b in $KNOWN_BINS; do
	[ -f "$bin_dir/$b" ] && present+=("$b")
done
[ ${#present[@]} -gt 0 ] || die "none of: $KNOWN_BINS found in $bin_dir"

# Validate steps: NAME:BINARY ARGS, restricted character set, known binary.
name_re='^[A-Za-z0-9_-]+$'
cmd_re='^[A-Za-z0-9_][A-Za-z0-9_ ./=,:+-]*$'
step_names=()
step_cmds=()
for s in "${steps[@]}"; do
	name=${s%%:*}
	cmd=${s#*:}
	[ "$name" != "$s" ] || die "step '$s' is not NAME:CMD"
	[[ $name =~ $name_re ]] || die "bad step name '$name'"
	[[ $cmd =~ $cmd_re ]] || die "step '$name': command has characters outside the allowed set"
	bin=${cmd%% *}
	ok=0
	for p in "${present[@]}"; do [ "$p" = "$bin" ] && ok=1; done
	[ $ok -eq 1 ] || die "step '$name': '$bin' is not one of the supplied binaries (${present[*]})"
	step_names+=("$name")
	step_cmds+=("$cmd")
done

sudo_p=${BOARD_SUDO:-}

# run_ssh CMD: run CMD on the board (or print it in dry-run mode).
run_ssh() {
	if [ $dry -eq 1 ]; then
		echo "ssh <board> $1"
		return 0
	fi
	# shellcheck disable=SC2086
	$SSH_BIN $BOARD_SSH "$1"
}

# scrub: replace the ssh target words in stdin by <board>.
scrub() {
	local args=() w e
	for w in ${BOARD_SSH:-}; do
		case $w in -*) continue ;; esac
		for e in "$w" "${w#*@}"; do
			[ -n "$e" ] || continue
			e=$(printf '%s' "$e" | sed 's/[][\.*^$|/&]/\\&/g')
			args+=(-e "s|$e|<board>|g")
		done
	done
	if [ ${#args[@]} -eq 0 ]; then cat; else sed "${args[@]}"; fi
}

utc() { date -u '+%Y-%m-%d %H:%MZ'; }

if [ $dry -eq 1 ]; then
	echo "DRY-RUN: no ssh is run; outputs would go to $out_dir"
	dir=/tmp/hvs-session.XXXXXX
	echo "ssh <board> mktemp -d $dir"
	echo "tar -C $bin_dir -cf - ${present[*]} display-guard.sh | ssh <board> tar -C $dir -xf -"
	echo "ssh <board> uname -r"
	echo "ssh <board> $sudo_p sh $dir/display-guard.sh stop $dir/dg.state"
	for i in "${!step_names[@]}"; do
		printf 'ssh <board> cd %s && %s ./%s   # stdout > %s/%02d-%s.stdout, stderr > .stderr, exit code > .exit\n' \
			"$dir" "$sudo_p" "${step_cmds[$i]}" "$out_dir" "$((i + 1))" "${step_names[$i]}"
	done
	echo "ssh <board> $sudo_p sh $dir/display-guard.sh restore $dir/dg.state"
	echo "ssh <board> sh $dir/display-guard.sh detect"
	echo "ssh <board> rm -rf $dir"
	echo "ssh <board> ls -d $dir   # must be gone"
	exit 0
fi

mkdir -p "$out_dir" || die "cannot create $out_dir"
summary=$out_dir/summary.txt
: > "$summary"
say() { echo "$*" | scrub | tee -a "$summary"; }

dir=$(run_ssh 'mktemp -d /tmp/hvs-session.XXXXXX' 2>/dev/null | tr -d '\r')
case $dir in
/tmp/hvs-session.[A-Za-z0-9]*) ;;
*) say "FAIL: could not create the board work directory"; exit 3 ;;
esac
case $dir in *[!A-Za-z0-9_./-]*) say "FAIL: odd work directory name"; exit 3 ;; esac

guard_stopped=0
finished=0
rc_guard=0
rc_clean=0
detect_out=
restore_out=

finish() {
	[ $finished -eq 0 ] || return 0
	finished=1
	if [ $guard_stopped -eq 1 ]; then
		restore_out=$(run_ssh "$sudo_p sh $dir/display-guard.sh restore $dir/dg.state" 2>&1 | scrub)
		rc_guard=${PIPESTATUS[0]}
		detect_out=$(run_ssh "sh $dir/display-guard.sh detect" 2>&1 | scrub)
	fi
	run_ssh "rm -rf $dir" > /dev/null 2>&1
	if run_ssh "ls -d $dir" > /dev/null 2>&1; then
		rc_clean=1
	fi
}
trap 'finish; exit 130' INT TERM HUP
trap finish EXIT

# Copy binaries and the guard.
tar -C "$bin_dir" -cf - "${present[@]}" -C "$here" display-guard.sh |
	{ run_ssh "tar -C $dir -xf -" 2>&1 | scrub >> "$summary"; }
[ "${PIPESTATUS[0]}" -eq 0 ] || { say "FAIL: copy to the board failed"; exit 3; }

kernel=$(run_ssh 'uname -r' 2>/dev/null | tr -d '\r')
start=$(utc)
say "SESSION start=$start (host UTC) kernel=$kernel"
say "BINARIES ${present[*]}"

guard_stopped=1   # restore also covers a half-failed stop
stop_out=$(run_ssh "$sudo_p sh $dir/display-guard.sh stop $dir/dg.state" 2>&1 | scrub)
rc_stop=${PIPESTATUS[0]}
say "$stop_out"
[ "$rc_stop" -eq 0 ] || say "WARN: display-guard stop exit $rc_stop"

any_fail=0
for i in "${!step_names[@]}"; do
	n=$(printf '%02d-%s' "$((i + 1))" "${step_names[$i]}")
	{
		echo "DATE $(utc) (host UTC)"
		echo "KERNEL $kernel"
		echo "STEP ${step_names[$i]}"
		echo "CMD ${step_cmds[$i]}"
	} > "$out_dir/$n.header"
	run_ssh "cd $dir && $sudo_p ./${step_cmds[$i]}" \
		> >(scrub > "$out_dir/$n.stdout") 2> >(scrub > "$out_dir/$n.stderr")
	rc=$?
	wait
	echo "$rc" > "$out_dir/$n.exit"
	[ "$rc" -eq 0 ] || any_fail=1
	say "STEP $n exit=$rc"
done

finish
say "$restore_out"
say "GUARD-DETECT-AFTER: ${detect_out:-none}"
if [ $rc_guard -eq 0 ]; then
	say "DISPLAY-MANAGER: active again (guard restore exit 0)"
else
	say "DISPLAY-MANAGER: NOT confirmed active (guard restore exit $rc_guard)"
fi
if [ $rc_clean -eq 0 ]; then
	say "CLEANUP: board work directory removed"
else
	say "CLEANUP: FAILED, board work directory may remain"
fi
[ $rc_guard -eq 0 ] && [ $rc_clean -eq 0 ] || exit 3
exit $any_fail
