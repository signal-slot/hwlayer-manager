#!/bin/sh
# Run rows A1-A6 of docs/board-scenarios.md on a Linux board with no
# compositor, by driving the drm-harness binary and judging its records.
#
# usage: board-scenarios.sh [options] HARNESS
#   HARNESS         path of the drm-harness binary (built for the board)
#   --out DIR       result directory (default /tmp/board-scenarios); the only
#                   place the script writes to
#   --rows LIST     comma-separated rows to run, default A1,A2,A3,A4,A5,A6;
#                   the others get NOT-RUN
#   --hold SECS     A2 steady-state hold (default 10 seconds); with --fps
#                   (default 60, assumed, not read from the sink) it gives
#                   `rotate --frames HOLD*FPS --buffers 3`
#   --frames N      A2 frame count, overrides --hold/--fps
#   --seconds N     how long each picture is held in A1, A3, A4, A5, A6
#                   (default 3); a hold that a picture is taken 20 s or more
#                   into needs --seconds of 25 or more
#   --loss NAME     A6: how the harness loses the display, master or disable
#                   (default: the harness's, master)
#   --allow-drops N dropped frames A2 may report (default 0)
#   --stop-wait N   seconds to wait for a signalled run to exit (default 10)
#   --device PATH, --connector NAME, --profile NAME
#                   passed to the harness unchanged when given
#   --hook CMD      command called as `CMD ROW PHASE [LABEL]` around each
#                   hold, so the caller can take a camera or KVM picture; its
#                   failure is only noted. hold-start is called once the
#                   hold is under way (the first `commit` record of the run
#                   is seen), while the hold goes on, so the hook may sleep
#                   and then take its picture; hold-end follows the end of
#                   the hold. PHASEs, by row (LABEL in brackets):
#                     A1  hold-start/hold-end [master | kernel-lease]
#                     A2  hold-start/hold-end
#                     A3  hold-start/hold-end [TERM | KILL]: the run before
#                         the signal, held --seconds; the signal follows
#                         hold-end at once
#                         post-stop-start/post-stop-end [TERM | KILL]: the
#                         process has exited; the screen is held --seconds
#                         restart-start/restart-end [TERM | KILL]: the
#                         restarted run, held --seconds
#                     A4  hold-start/hold-end: the revoke run
#                     A5  hold-start/hold-end: the modeset run; hold-start
#                         comes with its first commit on the original mode,
#                         about 1 s before the switch, so a picture taken
#                         20 s in shows the second mode if --seconds >= 25
#                     A6  hold-start/hold-end twice: first the hold before
#                         the loss (from the first commit; hold-end once the
#                         loss is made), then the new run's hold after the
#                         recovery (from its first commit to the exit)
#
# Rows (each writes OUT/ROW/result.txt: first line
# `SCENARIO <name> PASS|FAIL|BLOCKED|NOT-RUN`, then the evidence lines, and
# OUT/ROW/*.log with the raw harness records):
#   A1 startup       two-planes --source master, then --source kernel-lease:
#                    exit 0, VERDICT PASS, every `commit` record has a
#                    latency_us (latched), at least one commit.
#   A2 steady state  rotate: VERDICT PASS, presented == frames, dropped <=
#                    --allow-drops, releases == 2*(presented-1)+2*dropped
#                    (the harness's own count, > 0), no order violations.
#   A3 stop/restart  two-planes held long, SIGTERM once the first commit is
#                    seen; separately SIGKILL; each must exit within
#                    --stop-wait, then A1's master check must pass again.
#   A4 lease loss    revoke --source kernel-lease: VERDICT PASS,
#                    revoke_result with DeviceLost on the first attempt,
#                    device_lost_events=1, nothing busy, no outputs/planes
#                    after, and no commit/frame/step_result record after
#                    before_revoke.
#   A5 modeset       modeset --source master --seconds N (N is 1 unless
#                    --seconds is given): the harness commits on the current
#                    mode, switches to the first other mode the connector
#                    offers that passes TEST_ONLY, re-plans, commits for N
#                    seconds and restores the mode. PASS: exit 0, VERDICT
#                    PASS, at least one `commit step=modeset-after` record
#                    and each of them latched (latency_us), and
#                    modeset_result restored= the original mode. VERDICT
#                    BLOCKED (exit 3; the connector offers one mode):
#                    BLOCKED with the harness's reason. Anything else: FAIL.
#   A6 display-recovery
#                    recovery --source master --seconds N [--loss NAME]: the
#                    harness commits for N seconds, loses the display (master
#                    taken by a second descriptor that turns the CRTC off, or
#                    the CRTC turned off), checks that the engine reports it
#                    and holds no buffer, then a new run commits for N
#                    seconds. PASS: exit 0, VERDICT PASS, recovery_result with
#                    reported=true, still_busy=-, relatched >= 1 and
#                    crtc_active_end=true, commits before the loss and after
#                    the recovery all latched, no commit of the first run
#                    after the loss record, and a resources record.
#
# Never starts or stops a display manager, never touches DRM, udev or seat
# configuration: stop the compositor yourself before, restore it after. The
# A5 mode switch and the A6 CRTC off are commits of the harness on the
# device it was given, undone before it exits. Only --source master and
# kernel-lease are used.
#
# Prints one `ROW name RESULT` summary line per row. Exit: 0 every selected
# row PASS, 1 a row FAILed, 2 usage, 3 no FAIL but a row is BLOCKED or
# NOT-RUN among the selected ones.

out=/tmp/board-scenarios
rows=A1,A2,A3,A4,A5,A6
hold=10
fps=60
frames=
seconds=3
allow_drops=0
stop_wait=10
hook=
common=
seconds_given=
loss=
while [ $# -gt 0 ]; do
	case $1 in
	--out) shift; out=${1:-} ;;
	--rows) shift; rows=${1:-} ;;
	--hold) shift; hold=${1:-} ;;
	--fps) shift; fps=${1:-} ;;
	--frames) shift; frames=${1:-} ;;
	--seconds) shift; seconds=${1:-}; seconds_given=1 ;;
	--allow-drops) shift; allow_drops=${1:-} ;;
	--stop-wait) shift; stop_wait=${1:-} ;;
	--hook) shift; hook=${1:-} ;;
	--loss) shift; loss=${1:-} ;;
	--device) shift; common="$common --device ${1:-}" ;;
	--connector) shift; common="$common --connector ${1:-}" ;;
	--profile) shift; common="$common --profile ${1:-}" ;;
	--) shift; break ;;
	-*) echo "unknown option: $1" >&2; exit 2 ;;
	*) break ;;
	esac
	shift
done
usage() { sed -n '2,/^$/s/^# \{0,1\}//p' "$0" >&2; exit 2; }
[ $# -eq 1 ] || usage
harness=$1
[ -x "$harness" ] || { echo "harness not executable: $harness" >&2; exit 2; }
for n in "$hold" "$fps" "$seconds" "$allow_drops" "$stop_wait" $frames; do
	case $n in ''|*[!0-9]*) echo "not a number: $n" >&2; exit 2 ;; esac
done
for r in $(echo "$rows" | tr ',' ' '); do
	case $r in A1|A2|A3|A4|A5|A6) ;; *) echo "unknown row: $r" >&2; exit 2 ;; esac
done
case $loss in ''|master|disable) ;; *) echo "unknown loss: $loss" >&2; exit 2 ;; esac
[ -n "$frames" ] || frames=$((hold * fps))
mkdir -p "$out" || exit 2

# fld FILE KIND KEY: value of KEY on the last KIND record (quotes dropped).
fld() {
	awk -v k="$2" -v key="$3" '
	$1 == k { for (i = 2; i <= NF; i++) if (index($i, key "=") == 1) {
		v = substr($i, length(key) + 2); gsub(/"/, "", v); r = v } }
	END { print r }' "$1"
}
count() { awk -v k="$2" '$1 == k { n++ } END { print n + 0 }' "$1"; }
is_num() { case $1 in ''|*[!0-9]*) return 1 ;; esac; }

# Evidence and failures of the row being judged.
ev=
bad=
note() { ev="$ev$*
"; }
fail() { bad="$bad$*
"; }

# runhook ROW PHASE [LABEL]
runhook() {
	[ -z "$hook" ] || $hook "$1" "$2" ${3:+"$3"} || note "hook $1 $2 ${3:-} failed"
}

# run_held ROW LABEL START END LOG CMD...: run CMD in the background, call
# the hook START once its first commit is seen (or it has ended), wait for
# it, then call the hook END. Sets rc to its exit status.
run_held() {
	hrow=$1 hlab=$2 hs=$3 he=$4 hlog=$5
	shift 5
	"$@" > "$hlog" 2>&1 &
	hpid=$!
	t=0
	while [ "$(count "$hlog" commit)" -eq 0 ] && kill -0 "$hpid" 2>/dev/null \
		&& ! grep -q '^VERDICT ' "$hlog" \
		&& [ "$t" -lt $((stop_wait * 10)) ]; do
		sleep 0.1; t=$((t + 1))
	done
	runhook "$hrow" "$hs" "$hlab"
	wait "$hpid"
	rc=$?
	runhook "$hrow" "$he" "$hlab"
}

# hold_for ROW START END LABEL: hold --seconds with the hook START running
# meanwhile, then call END.
hold_for() {
	sleep "$seconds" &
	hsp=$!
	runhook "$1" "$2" "$4"
	wait "$hsp"
	runhook "$1" "$3" "$4"
}

# check_verdict LOG NAME RC: exit status and VERDICT line.
check_verdict() {
	note "exit=$3"
	[ "$3" -eq 0 ] || fail "$2: exit status $3"
	v=$(grep "^VERDICT $2 " "$1" | tail -n 1)
	if [ -z "$v" ]; then fail "$2: no VERDICT line"; else
		note "$v"
		case $v in "VERDICT $2 PASS"*) ;; *) fail "$2: $v" ;; esac
	fi
}

# check_commits LOG: at least one commit, each latched (latency_us set).
check_commits() {
	c=$(count "$1" commit)
	l=$(awk '$1 == "commit" { for (i = 2; i <= NF; i++)
		if ($i ~ /^latency_us=[0-9]+$/) n++ } END { print n + 0 }' "$1")
	note "commits=$c latched=$l"
	[ "$c" -ge 1 ] || fail "no commit record"
	[ "$l" -eq "$c" ] || fail "latched=$l differs from commits=$c"
}

# run_two_planes LOG SOURCE [ROW LABEL START END]: the A1 check on one
# source; the hook defaults are A1, the source, hold-start, hold-end.
run_two_planes() {
	# shellcheck disable=SC2086
	run_held "${3:-A1}" "${4:-$2}" "${5:-hold-start}" "${6:-hold-end}" "$1" \
		"$harness" $common --source "$2" --seconds "$seconds" two-planes
	note "-- two-planes --source $2"
	check_verdict "$1" two-planes "$rc"
	check_commits "$1"
}

finish() { # ROW NAME: write result.txt from ev/bad
	d=$out/$1
	mkdir -p "$d"
	if [ -n "$bad" ]; then st=FAIL; else st=${forced:-PASS}; fi
	{ echo "SCENARIO $2 $st"; printf '%s' "$ev"
	  printf '%s' "$bad" | sed 's/^/failed: /'; } > "$d/result.txt"
	echo "$1 $2 $st"
	eval "res_$1=$st"
}

row_a1() {
	d=$out/A1; mkdir -p "$d"
	run_two_planes "$d/master.log" master
	run_two_planes "$d/kernel-lease.log" kernel-lease
	if ! grep -q '^lease ' "$d/kernel-lease.log"; then
		fail "kernel-lease: no lease record"
	fi
}

row_a2() {
	d=$out/A2; mkdir -p "$d"
	note "hold=${hold}s fps=${fps} (assumed) frames=$frames buffers=3 allow_drops=$allow_drops"
	# shellcheck disable=SC2086
	run_held A2 "" hold-start hold-end "$d/rotate.log" \
		"$harness" $common --source master --frames "$frames" --buffers 3 rotate
	check_verdict "$d/rotate.log" rotate "$rc"
	f=$d/rotate.log
	p=$(fld "$f" rotate_result presented)
	dr=$(fld "$f" rotate_result dropped)
	rel=$(fld "$f" rotate_result releases)
	ov=$(fld "$f" rotate_result order_violations)
	note "frames=$frames presented=$p dropped=$dr releases=$rel order_violations=$ov"
	if ! is_num "$p" || ! is_num "$dr" || ! is_num "$rel" || ! is_num "$ov"; then
		fail "rotate_result missing or incomplete"
		return
	fi
	[ "$p" -eq "$frames" ] || fail "presented=$p, expected $frames"
	[ "$dr" -le "$allow_drops" ] || fail "dropped=$dr exceeds $allow_drops"
	[ "$ov" -eq 0 ] || fail "order_violations=$ov"
	pm=$p
	[ "$pm" -eq 0 ] || pm=$((pm - 1))
	want=$((2 * pm + 2 * dr))
	[ "$rel" -gt 0 ] || fail "no releases"
	[ "$rel" -eq "$want" ] || fail "releases=$rel, expected $want"
	[ "$(count "$f" violation)" -eq 0 ] || fail "violation records present"
}

# stop_run LOG SIGNAL: start a long hold, signal it, check the exit.
stop_run() {
	log=$1 sig=$2
	# shellcheck disable=SC2086
	"$harness" $common --source master --seconds 600 two-planes > "$log" 2>&1 &
	pid=$!
	t=0
	while [ "$(count "$log" commit)" -eq 0 ] && kill -0 "$pid" 2>/dev/null \
		&& [ "$t" -lt $((stop_wait * 10)) ]; do
		sleep 0.1; t=$((t + 1))
	done
	if [ "$(count "$log" commit)" -eq 0 ]; then
		kill -KILL "$pid" 2>/dev/null; wait "$pid" 2>/dev/null
		fail "$sig: no commit before the signal"
		return
	fi
	hold_for A3 hold-start hold-end "$sig"
	kill -"$sig" "$pid"
	t=0
	while kill -0 "$pid" 2>/dev/null && [ "$t" -lt $((stop_wait * 10)) ]; do
		sleep 0.1; t=$((t + 1))
	done
	if kill -0 "$pid" 2>/dev/null; then
		kill -KILL "$pid" 2>/dev/null; wait "$pid" 2>/dev/null
		fail "$sig: still running after ${stop_wait}s"
		return
	fi
	wait "$pid"; rc=$?
	note "$sig: exit=$rc after commits=$(count "$log" commit)"
	hold_for A3 post-stop-start post-stop-end "$sig"
	case $sig:$rc in
	TERM:0|TERM:143|KILL:137) ;;
	*) fail "$sig: unexpected exit status $rc" ;;
	esac
}

row_a3() {
	d=$out/A3; mkdir -p "$d"
	for sig in TERM KILL; do
		stop_run "$d/$sig.log" "$sig"
		# Restart and repeat A1's check.
		run_two_planes "$d/restart-after-$sig.log" master A3 "$sig" \
			restart-start restart-end
	done
}

row_a4() {
	d=$out/A4; mkdir -p "$d"; f=$d/revoke.log
	# shellcheck disable=SC2086
	run_held A4 "" hold-start hold-end "$f" \
		"$harness" $common --source kernel-lease --frames 60 --buffers 3 \
		--seconds "$seconds" revoke
	check_verdict "$f" revoke "$rc"
	fa=$(fld "$f" revoke_result first_attempt)
	lost=$(fld "$f" revoke_result device_lost_events)
	busy=$(fld "$f" revoke_result still_busy)
	oa=$(fld "$f" revoke_result outputs_after)
	pa=$(fld "$f" revoke_result planes_after)
	note "lease_revoked=$(count "$f" lease_revoked) first_attempt=$fa device_lost_events=$lost still_busy=$busy outputs_after=$oa planes_after=$pa"
	if [ -z "$fa" ]; then fail "no revoke_result record"; return; fi
	case $fa in *DeviceLost*) ;; *) fail "first_attempt is not DeviceLost" ;; esac
	[ "$lost" = 1 ] || fail "device_lost_events=$lost, expected 1"
	[ "$busy" = - ] || fail "buffers still busy: $busy"
	[ "$oa" = 0 ] && [ "$pa" = 0 ] || fail "outputs/planes reported after the loss"
	# No commit may follow the revocation.
	after=$(awk '/^before_revoke /{s=1; next}
		s && ($1=="commit"||$1=="frame"||$1=="step_result") {n++}
		END{print n+0}' "$f")
	grep -q '^before_revoke ' "$f" || fail "no before_revoke record"
	note "commit-like records after before_revoke=$after"
	[ "$after" -eq 0 ] || fail "$after commit records after the revocation"
}

row_a5() {
	d=$out/A5; mkdir -p "$d"; f=$d/modeset.log
	s=1
	[ -z "$seconds_given" ] || s=$seconds
	# shellcheck disable=SC2086
	run_held A5 "" hold-start hold-end "$f" \
		"$harness" $common --source master --seconds "$s" modeset
	note "exit=$rc"
	note "connector=$(fld "$f" connector_modes connector) current=$(fld "$f" connector_modes current) modes listed=$(fld "$f" connector_modes count)"
	v=$(grep '^VERDICT modeset ' "$f" | tail -n 1)
	[ -z "$v" ] || note "$v"
	case $v in
	'')
		fail "modeset: no VERDICT line"
		return
		;;
	"VERDICT modeset BLOCKED"*)
		[ "$rc" -eq 3 ] || fail "modeset: BLOCKED with exit status $rc, expected 3"
		forced=BLOCKED
		note "BLOCKED: ${v#VERDICT modeset BLOCKED }"
		return
		;;
	"VERDICT modeset PASS"*)
		[ "$rc" -eq 0 ] || fail "modeset: exit status $rc"
		;;
	*)
		fail "modeset: $v"
		;;
	esac
	from=$(fld "$f" modeset_result from)
	to=$(fld "$f" modeset_result to)
	restored=$(fld "$f" modeset_result restored)
	note "from=$from to=$to restored=$restored"
	ac=$(awk '$1 == "commit" && / step=modeset-after / { n++ } END { print n + 0 }' "$f")
	al=$(awk '$1 == "commit" && / step=modeset-after / { for (i = 2; i <= NF; i++)
		if ($i ~ /^latency_us=[0-9]+$/) n++ } END { print n + 0 }' "$f")
	note "after the switch: commits=$ac latched=$al"
	[ "$al" -ge 1 ] || fail "no latch after the switch"
	[ "$al" -eq "$ac" ] || fail "after the switch latched=$al differs from commits=$ac"
	[ -n "$from" ] && [ "$restored" = "$from" ] ||
		fail "original mode ${from:-?} not restored (restored=${restored:-?})"
}

# wait_line LOG PID PATTERN SECS: until a line of LOG matches PATTERN, PID
# has exited or SECS have passed.
wait_line() {
	t=0
	while ! grep -q "$3" "$1" && kill -0 "$2" 2>/dev/null \
		&& [ "$t" -lt $(($4 * 10)) ]; do
		sleep 0.1; t=$((t + 1))
	done
}

# latched_step LOG STEP: "commits latched" of the commit records of STEP.
latched_step() {
	awk -v s="step=$2" '$1 == "commit" { m = 0
		for (i = 2; i <= NF; i++) { if ($i == s) m = 1 }
		if (!m) next; c++
		for (i = 2; i <= NF; i++) if ($i ~ /^latency_us=[0-9]+$/) l++ }
		END { print c + 0, l + 0 }' "$1"
}

row_a6() {
	d=$out/A6; mkdir -p "$d"; f=$d/recovery.log
	# shellcheck disable=SC2086
	"$harness" $common --source master --seconds "$seconds" ${loss:+--loss "$loss"} \
		recovery > "$f" 2>&1 &
	pid=$!
	limit=$((seconds + stop_wait))
	wait_line "$f" "$pid" '^commit .*step=recovery-before' "$stop_wait"
	runhook A6 hold-start
	wait_line "$f" "$pid" '^loss ' "$limit"
	runhook A6 hold-end
	wait_line "$f" "$pid" '^commit .*step=recovery-after' "$limit"
	runhook A6 hold-start
	wait "$pid"
	rc=$?
	runhook A6 hold-end
	check_verdict "$f" recovery "$rc"
	rr=recovery_result
	note "loss=$(fld "$f" loss method) made=$(fld "$f" "$rr" made) crtc_active_after_loss=$(fld "$f" loss crtc_active)"
	note "first=$(fld "$f" "$rr" first) first_ms=$(fld "$f" "$rr" first_ms) second=$(fld "$f" "$rr" second) master_back=$(fld "$f" "$rr" master_back)"
	rep=$(fld "$f" "$rr" reported)
	busy=$(fld "$f" "$rr" still_busy)
	rl=$(fld "$f" "$rr" relatched)
	act=$(fld "$f" "$rr" crtc_active_end)
	note "reported=$rep still_busy=$busy relatched=$rl crtc_active_end=$act"
	if ! grep -q "^$rr " "$f"; then fail "no $rr record"; return; fi
	[ "$rep" = true ] || fail "the engine did not report the loss (first=$(fld "$f" "$rr" first))"
	[ "$busy" = - ] || fail "buffers still held after the loss: $busy"
	is_num "$rl" && [ "$rl" -ge 1 ] || fail "the new run latched no commit"
	[ "$act" = true ] || fail "CRTC not on at the end (crtc_active_end=$act)"
	for step in recovery-before recovery-after; do
		cl=$(latched_step "$f" "$step")
		c=${cl% *} l=${cl#* }
		note "$step: commits=$c latched=$l"
		[ "$c" -ge 1 ] && [ "$l" -eq "$c" ] || fail "$step: latched=$l of commits=$c"
	done
	# The first run commits nothing once the display is lost.
	late=$(awk '/^loss /{s=1; next} s && $1 == "commit" && / step=recovery-before / {n++}
		END{print n+0}' "$f")
	[ "$late" -eq 0 ] || fail "$late commits of the first run after the loss"
	grep -q '^resources scenario=recovery ' "$f" || fail "no resources record"
}

sel=",$rows,"
overall=0
for r in A1 A2 A3 A4 A5 A6; do
	case $r in
	A1) name=startup ;; A2) name=steady-state ;; A3) name=stop-restart ;;
	A4) name=lease-loss ;; A5) name=modeset ;; A6) name=display-recovery ;;
	esac
	ev= bad= forced=
	case $sel in
	*",$r,"*) "row_$(echo "$r" | tr 'A' 'a')" ;;
	*)
		forced=NOT-RUN
		note "row not selected"
		;;
	esac
	finish "$r" "$name"
	eval "st=\$res_$r"
	case $st in
	PASS) ;;
	FAIL) overall=1 ;;
	*) [ "$overall" -eq 1 ] || { case $sel in *",$r,"*) overall=3 ;; esac; } ;;
	esac
done
exit "$overall"
