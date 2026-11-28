#!/bin/sh
# Host test for board-scenarios.sh against a mocked drm-harness: a fake
# executable that prints canned records in the harness's real format.
# No board, no DRM device. Behaviour is chosen by MOCK_* variables:
#   MOCK_TP       two-planes: ok | nolatch | exit1 | nocommit  (all sources)
#   MOCK_TP_LEASE the same, for --source kernel-lease only
#   MOCK_HOLD     the long two-planes (--seconds 600): ok | nocommit |
#                 ignoreterm | termexit1
#   MOCK_ROT      rotate: ok | drop | norelease | noresult | exit1
#   MOCK_REV      revoke: ok | commitafter | nolost | busy | exit1
#   MOCK_MS       modeset: ok | blocked | blockedexit0 | switchfail | nolatch |
#                 noafter | norestore | exit1 | noverdict
#   MOCK_LINGER   seconds rotate and revoke keep running after their `step`
#                 record, as a hold does
#   MOCK_ORDER    file to which rotate and revoke append `harness-done`
#                 after the linger
#   MOCK_CONN     connector name of the selected output (default HDMI-A-1;
#                 the real harness prints e.g. HDMIA-1, DisplayPort-2)
#   MOCK_REC      recovery: ok | fail | noverdict | stall | passbusy |
#                 norelatch | late | crtcoff

here=$(cd "$(dirname "$0")" && pwd)
script=$here/board-scenarios.sh
T=$(mktemp -d "${TMPDIR:-/tmp}/board-scen-test.XXXXXX") || exit 1
trap 'rm -rf "$T"' EXIT
trap 'exit 1' INT TERM

cat > "$T/harness" <<'EOF'
#!/bin/sh
# Mock drm-harness: the scenario is the last argument.
# `trace-check FILE` counts the lines of a trace file like the real one (the
# strict parse itself is tested in tools/drm-harness); MOCK_TRACE chooses what
# a --trace run writes: ok | mismatch (one latched too many) | malformed |
# norecord (no `trace` record) | nofile.
if [ "$1" = trace-check ]; then
	[ -f "$2" ] || { echo "cannot read the file" >&2; exit 2; }
	if grep -q ' t=abc' "$2"; then echo "line 3: malformed trace line: BadNumber(\"t\") at offset 30" >&2; exit 1; fi
	awk '/^CETRACE/ { l++ }
		/ ev=commit / && / result=ok/ { c++ }
		/ ev=latched / { la++ } / ev=released / { r++ } / ev=dropped / { d++ }
		/ ev=device_lost/ { dl++ }
		END { print "TRACE lines " l + 0; print "TRACE commit_ok " c + 0
			print "TRACE latched " la + 0; print "TRACE released " r + 0
			print "TRACE dropped " d + 0; print "TRACE device_lost " dl + 0 }' "$2"
	exit 0
fi
for a; do scen=$a; done
src=master; secs=5; tf=
prev=
for a; do
	[ "$prev" = --source ] && src=$a
	[ "$prev" = --seconds ] && secs=$a
	[ "$prev" = --trace ] && tf=$a
	prev=$a
done
hdrline='CETRACE v=1 ev=hdr seq=0 src=linux clock=monotonic_ns freq_hz=1000000000 t0=1 at_unit=ms build="mock" board="mock"'
body() {
echo "harness version=0.1.0 scenario=$scen args=\"$*\""
echo "session scenario=$scen source=$src device=/dev/dri/card0 driver=mockdrm master=yes"
echo "output index=0 id=31 connector=${MOCK_CONN:-HDMI-A-1} width=1920 height=1080 refresh_mhz=60000 mode=1920x1080 vrefresh=60 clock_khz=148500 selected=yes"
[ "$src" = kernel-lease ] && echo "lease route=kernel device=/dev/dri/card0 lessee=1 connector=31 connector_name=HDMI-A-1 crtc=40 primary=50 overlays=51,52 objects=5"
verdict() { echo "VERDICT $1 $2 $3"; }
case $scen in
two-planes)
	mode=${MOCK_TP:-ok}
	[ "$src" = kernel-lease ] && mode=${MOCK_TP_LEASE:-$mode}
	if [ "$secs" = 600 ]; then
		case ${MOCK_HOLD:-ok} in
		nocommit) exec sleep 30 ;;
		*)
			[ -z "$tf" ] || printf '%s\nCETRACE v=1 ev=commit seq=1 t=2 at=0 out=0 result=ok sub=1\nCETRACE v=1 ev=latched seq=2 t=3 at=0 out=0 sub=1\n' "$hdrline" > "$tf"
			echo "commit step=two-planes submission=1 validate_us=100 commit_us=200 latency_us=16700" ;;
		esac
		case ${MOCK_HOLD:-ok} in
		ignoreterm) trap '' TERM; while :; do sleep 0.2; done ;;
		termexit1) trap 'exit 1' TERM; while :; do sleep 0.2; done ;;
		*) exec sleep 600 ;;
		esac
	fi
	echo "assign step=two-planes layer=0 buffer=1 plane=50 kind=primary zpos=0"
	case $mode in
	ok) echo "commit step=two-planes submission=1 validate_us=100 commit_us=200 latency_us=16700" ;;
	nolatch) echo "commit step=two-planes submission=1 validate_us=100 commit_us=200" ;;
	exit1) echo "error scenario=two-planes message=\"not DRM master\""; verdict two-planes FAIL "not DRM master"; exit 1 ;;
	esac
	echo "hold step=two-planes seconds=$secs"
	verdict two-planes PASS "2 buffers on planes 50,51 (bottom,top)"
	;;
rotate)
	mode=${MOCK_ROT:-ok}
	frames=0; prev=
	for a; do [ "$prev" = --frames ] && frames=$a; prev=$a; done
	echo "step label=rotate frames=$frames buffers_per_plane=3 period_us=16666"
	if [ -n "$MOCK_LINGER" ]; then sleep "$MOCK_LINGER"; [ -z "$MOCK_ORDER" ] || echo harness-done >> "$MOCK_ORDER"; fi
	pres=$frames; drop=0; rel=$((2 * (frames - 1)))
	[ "$mode" = drop ] && { drop=2; pres=$frames; rel=$((rel + 4)); }
	[ "$mode" = norelease ] && rel=0
	[ "$mode" = noresult ] || echo "rotate_result frames=$frames buffers_per_plane=3 presented=$pres dropped=$drop missed_vblanks=0 latch_timeouts=0 releases=$rel order_violations=0 refresh_mhz=60000"
	[ "$mode" = exit1 ] && { verdict rotate FAIL "late"; exit 1; }
	verdict rotate PASS "$frames frames"
	;;
revoke)
	mode=${MOCK_REV:-ok}
	echo "step label=revoke frames_before=60 buffers_per_plane=3"
	if [ -n "$MOCK_LINGER" ]; then sleep "$MOCK_LINGER"; [ -z "$MOCK_ORDER" ] || echo harness-done >> "$MOCK_ORDER"; fi
	echo "progress frame=60 presented=60 dropped=0 releases=118 violations=0"
	echo "before_revoke frames=60 releases=118 on_screen=3,6"
	[ "$mode" = commitafter ] && echo "commit step=revoke submission=61 validate_us=1 commit_us=1 latency_us=1"
	fa="Err(Backend(DeviceLost))"; lost=1; busy=-
	[ "$mode" = nolost ] && { fa="Ok(())"; lost=0; }
	[ "$mode" = busy ] && busy=3
	echo "lease_revoked lessee=1"
	echo "revoke_result first_attempt=$fa first_attempt_us=90 second_attempt=Err(Plan(UnknownOutput)) device_lost_events=$lost released=3,6 still_busy=$busy outputs_after=0 planes_after=0 events=DeviceLost"
	[ "$mode" = exit1 ] && { verdict revoke FAIL "x"; exit 1; }
	verdict revoke PASS "60 frames, then revoked"
	;;
modeset)
	mode=${MOCK_MS:-ok}
	conn=${MOCK_CONN:-HDMI-A-1}
	case $mode in blocked|blockedexit0)
		echo "connector_modes connector=$conn current=800x480@60 count=1 modes=800x480@60"
		echo "resources scenario=modeset wall_ms=50 cpu_user_ms=0 cpu_sys_ms=0 cpu_pct=0.0 rss_kib=900 peak_rss_kib=900"
		verdict modeset BLOCKED "the connector offers only one mode (800x480@60, 1 listed)"
		[ "$mode" = blocked ] && exit 3
		exit 0
		;;
	esac
	echo "connector_modes connector=$conn current=1920x1080@60 count=3 modes=1920x1080@60,1920x1080@50,1280x720@60"
	echo "mode_candidate index=1 mode=1920x1080@50 name=1920x1080 test_only=ok"
	echo "step label=modeset-before width=1920 height=1080 seconds=1"
	echo "commit step=modeset-before submission=1 validate_us=100 commit_us=30000 latency_us=30100"
	echo "commit step=modeset-before submission=2 validate_us=100 commit_us=200 latency_us=16700"
	if [ "$mode" = switchfail ]; then
		echo "modeset step=switch mode=1920x1080@50 mode_name=1920x1080 commit_us=90 result=\"Invalid argument (os error 22)\" crtc_mode=1920x1080@60"
		echo "modeset_result from=1920x1080@60 to=1920x1080@50 refused=- before_commits=2 before_latched=2 switch_us=- after_commits=0 after_latched=0 restored=-"
		verdict modeset FAIL "switch to 1920x1080@50: Invalid argument (os error 22)"
		exit 1
	fi
	echo "modeset step=switch mode=1920x1080@50 mode_name=1920x1080 commit_us=40000 result=ok crtc_mode=1920x1080@50"
	echo "replan width=1920 height=1080 refresh_mhz=50000"
	ac=2; al=2
	case $mode in
	noafter) ac=0; al=0 ;;
	nolatch)
		al=1
		echo "commit step=modeset-after submission=1 validate_us=100 commit_us=200 latency_us=20000"
		echo "commit step=modeset-after submission=2 validate_us=100 commit_us=200"
		;;
	*)
		echo "commit step=modeset-after submission=1 validate_us=100 commit_us=200 latency_us=20000"
		echo "commit step=modeset-after submission=2 validate_us=100 commit_us=200 latency_us=20000"
		;;
	esac
	echo "hold step=modeset-after seconds=$secs"
	restored=1920x1080@60
	[ "$mode" = norestore ] && restored=1920x1080@50
	echo "modeset step=restore mode=1920x1080@60 mode_name=1920x1080 commit_us=40000 result=ok crtc_mode=$restored"
	echo "modeset_result from=1920x1080@60 to=1920x1080@50 refused=- before_commits=2 before_latched=2 switch_us=40000 after_commits=$ac after_latched=$al restored=$restored"
	echo "resources scenario=modeset wall_ms=3000 cpu_user_ms=10 cpu_sys_ms=20 cpu_pct=1.0 rss_kib=1000 peak_rss_kib=1000"
	[ "$mode" = noverdict ] && exit 1
	[ "$mode" = exit1 ] && { verdict modeset FAIL "no latch after the switch to 1920x1080@50"; exit 1; }
	verdict modeset PASS "1920x1080@60 -> 1920x1080@50: 2 commits latched before, $al after the switch; 1920x1080@60 restored"
	;;
recovery)
	# Records as the real harness prints them on vkms.
	mode=${MOCK_REC:-ok}
	lossm=master; prev=
	for a; do [ "$prev" = --loss ] && lossm=$a; prev=$a; done
	echo "hold step=recovery-before seconds=$secs"
	echo "step label=recovery-before width=1920 height=1080 seconds=$secs expect=\"blue full screen\""
	echo "commit step=recovery-before submission=1 validate_us=100 commit_us=30000 latency_us=30100"
	echo "commit step=recovery-before submission=2 validate_us=100 commit_us=200 latency_us=16700"
	echo "before_loss commits=2 latched=2 on_screen=7,8"
	echo "loss method=$lossm crtc=40 result=ok crtc_active=false"
	[ "$mode" = late ] && echo "commit step=recovery-before submission=3 validate_us=1 commit_us=1 latency_us=1"
	first="error:validate:Backend(Platform(13))"; rep=true; busy=-; rl=2; act=true
	case $mode in
	stall) first=silent; rep=false ;;
	fail|passbusy) busy=7,8 ;;
	norelatch) rl=0 ;;
	crtcoff) act=false ;;
	esac
	echo "loss_attempt step=after-loss report=$first after_ms=0 backend_failures=NotMaster{operation:Check,errno:13} device_lost=false pending_latch=false events=-"
	echo "master_back other_closed=true result=ok"
	echo "buffers_after_loss states=7:Scanning,8:Scanning,9:Ready busy=$busy"
	echo "hold step=recovery-after seconds=$secs"
	if [ "$rl" -gt 0 ]; then
		echo "commit step=recovery-after submission=1 validate_us=100 commit_us=17000 latency_us=17100"
		echo "commit step=recovery-after submission=2 validate_us=100 commit_us=200 latency_us=16700"
	fi
	echo "recovery_result loss=$lossm made=ok first=$first first_ms=0 reported=$rep second=error:commit:Backend(Rejected) still_busy=$busy master_back=ok relatched=$rl crtc_active_end=$act"
	echo "resources scenario=recovery wall_ms=2211 cpu_user_ms=0 cpu_sys_ms=140 cpu_pct=6.3 rss_kib=1060 peak_rss_kib=1080"
	case $mode in
	noverdict) exit 1 ;;
	fail) verdict recovery FAIL "buffers 7,8 still submitted or scanning after the loss was reported: the engine did not release them"; exit 1 ;;
	stall) verdict recovery FAIL "no error, DeviceLost or LatchTimeout within 2000 ms of the loss: the engine stalled silently"; exit 1 ;;
	esac
	verdict recovery PASS "$lossm loss reported as $first after 0 ms; no buffer held; a new run latched $rl commits"
	;;
esac
exit 0
}
if [ -z "$tf" ] || [ "$secs" = 600 ]; then body "$@"; exit $?; fi
# A --trace run: print what the scenario prints, then write the trace the
# real engine would have produced for those records.
( body "$@" ) > "$tf.mock"
rc=$?
cat "$tf.mock"
tm=${MOCK_TRACE:-ok}
if [ "$tm" != nofile ]; then
	{
		echo "$hdrline"
		awk '
		function ev(s) { n++; print "CETRACE v=1 ev=" s }
		function kv(s, k,   i, v) { for (i = 2; i <= NF; i++) if (index($i, k "=") == 1) v = substr($i, length(k) + 2); return v }
		$1 == "commit" { ev("commit seq=" n " t=1 at=0 out=0 result=ok sub=" kv($0, "submission"))
			if ($0 ~ /latency_us=/) ev("latched seq=" n " t=1 at=0 out=0 sub=" kv($0, "submission")) }
		$1 == "rotate_result" { p = kv($0, "presented") + 0; dr = kv($0, "dropped") + 0; rel = kv($0, "releases") + 0
			for (i = 0; i < p; i++) { ev("commit seq=" n " t=1 at=0 out=0 result=ok sub=1"); ev("latched seq=" n " t=1 at=0 out=0 sub=1") }
			for (i = 0; i < dr; i++) ev("dropped seq=" n " t=1 at=0 out=0 sub=1 reason=replaced")
			for (i = 0; i < rel; i++) ev("released seq=" n " t=1 at=0 buf=1 sub=1") }
		$1 == "before_revoke" { f = kv($0, "frames") + 0
			for (i = 0; i < f; i++) { ev("commit seq=" n " t=1 at=0 out=0 result=ok sub=1"); ev("latched seq=" n " t=1 at=0 out=0 sub=1") } }
		$1 == "revoke_result" && kv($0, "device_lost_events") == 1 { ev("device_lost seq=" n " t=1 at=0") }
		' "$tf.mock"
		case $tm in
		mismatch) echo "CETRACE v=1 ev=latched seq=99 t=1 at=0 out=0 sub=1" ;;
		malformed) echo "CETRACE v=1 ev=latched seq=99 t=abc at=0 out=0 sub=1" ;;
		esac
	} > "$tf"
	[ "$tm" = norecord ] || echo "trace lines=$(grep -c '^CETRACE' "$tf") failed=0"
fi
exit $rc
EOF
chmod +x "$T/harness"

fails=0
n=0
ok() { n=$((n + 1)); }
bad() { fails=$((fails + 1)); echo "FAIL: $*" >&2; }

# t NAME ROW WANT_FIRST_LINE WANT_EXIT [script options...]  (MOCK_* from env)
t() {
	name=$1 row=$2 want=$3 wantrc=$4
	shift 4
	rm -rf "$T/out"
	# shellcheck disable=SC2086
	env $E sh "$script" --out "$T/out" --rows "$row" --seconds 0 --stop-wait 3 "$@" "$T/harness" > "$T/stdout" 2>&1
	rc=$?
	E=
	got=$(sed -n 1p "$T/out/$row/result.txt" 2>/dev/null)
	if [ "$got" = "$want" ] && [ "$rc" -eq "$wantrc" ]; then ok; else
		bad "$name: got '$got' rc=$rc, want '$want' rc=$wantrc"
		cat "$T/stdout" >&2
	fi
}
# has ROW TEXT: result.txt of the last run contains TEXT.
has() {
	if grep -q -- "$2" "$T/out/$1/result.txt" 2>/dev/null; then ok; else
		bad "result.txt of $1 lacks '$2'"; cat "$T/out/$1/result.txt" >&2
	fi
}

# ---- A1
t "A1 pass" A1 "SCENARIO startup PASS" 0
has A1 "commits=1 latched=1"
E="MOCK_TP=nolatch"; t "A1 no latch" A1 "SCENARIO startup FAIL" 1
E="MOCK_TP_LEASE=nolatch"; t "A1 lease no latch" A1 "SCENARIO startup FAIL" 1
has A1 "latched=0 differs from commits=1"
E="MOCK_TP=exit1"; t "A1 nonzero exit" A1 "SCENARIO startup FAIL" 1
has A1 "exit status 1"
E="MOCK_TP=nocommit"; t "A1 no commit" A1 "SCENARIO startup FAIL" 1

# ---- A2
t "A2 pass" A2 "SCENARIO steady-state PASS" 0 --hold 2 --fps 30
has A2 "hold=2s fps=30 (assumed) frames=60"
has A2 "presented=60 dropped=0 releases=118"
t "A2 default stated" A2 "SCENARIO steady-state PASS" 0 --frames 12
E="MOCK_ROT=drop"; t "A2 drops not allowed" A2 "SCENARIO steady-state FAIL" 1 --frames 12
E="MOCK_ROT=drop"; t "A2 drops allowed" A2 "SCENARIO steady-state PASS" 0 --frames 12 --allow-drops 2
E="MOCK_ROT=norelease"; t "A2 no releases" A2 "SCENARIO steady-state FAIL" 1 --frames 12
E="MOCK_ROT=noresult"; t "A2 no result" A2 "SCENARIO steady-state FAIL" 1 --frames 12
E="MOCK_ROT=exit1"; t "A2 nonzero" A2 "SCENARIO steady-state FAIL" 1 --frames 12

# ---- A3
t "A3 pass" A3 "SCENARIO stop-restart PASS" 0
has A3 "TERM: exit=143"
has A3 "KILL: exit=137"
E="MOCK_HOLD=nocommit"; t "A3 no commit before signal" A3 "SCENARIO stop-restart FAIL" 1
E="MOCK_HOLD=ignoreterm"; t "A3 term ignored" A3 "SCENARIO stop-restart FAIL" 1
has A3 "still running after"
E="MOCK_HOLD=termexit1"; t "A3 term exit 1" A3 "SCENARIO stop-restart FAIL" 1
has A3 "unexpected exit status 1"
E="MOCK_TP=nolatch"; t "A3 restart without latch" A3 "SCENARIO stop-restart FAIL" 1

# ---- A4
t "A4 pass" A4 "SCENARIO lease-loss PASS" 0
has A4 "device_lost_events=1"
E="MOCK_REV=commitafter"; t "A4 commit after revoke" A4 "SCENARIO lease-loss FAIL" 1
has A4 "commit records after the revocation"
E="MOCK_REV=nolost"; t "A4 no DeviceLost" A4 "SCENARIO lease-loss FAIL" 1
E="MOCK_REV=busy"; t "A4 buffer busy" A4 "SCENARIO lease-loss FAIL" 1
E="MOCK_REV=exit1"; t "A4 nonzero exit" A4 "SCENARIO lease-loss FAIL" 1

# ---- A5: the harness's modeset scenario
t "A5 pass" A5 "SCENARIO modeset PASS" 0
has A5 "VERDICT modeset PASS"
has A5 "from=1920x1080@60 to=1920x1080@50 restored=1920x1080@60"
has A5 "after the switch: commits=2 latched=2"
has A5 "connector=HDMI-A-1 current=1920x1080@60 modes listed=3"
grep -q -- '--source master --seconds 0 modeset' "$T/out/A5/modeset.log" && ok ||
	bad "A5 does not run modeset --source master with the given --seconds"
E="MOCK_MS=blocked"; t "A5 one mode" A5 "SCENARIO modeset BLOCKED" 3
has A5 "BLOCKED: the connector offers only one mode (800x480@60, 1 listed)"
E="MOCK_MS=blockedexit0"; t "A5 BLOCKED with exit 0" A5 "SCENARIO modeset FAIL" 1
has A5 "BLOCKED with exit status 0, expected 3"
E="MOCK_MS=switchfail"; t "A5 switch refused" A5 "SCENARIO modeset FAIL" 1
has A5 "Invalid argument"
E="MOCK_MS=exit1"; t "A5 harness FAIL" A5 "SCENARIO modeset FAIL" 1
has A5 "modeset: VERDICT modeset FAIL no latch after the switch"
E="MOCK_MS=noverdict"; t "A5 no VERDICT" A5 "SCENARIO modeset FAIL" 1
has A5 "modeset: no VERDICT line"
# The script's own checks hold even when the harness says PASS.
E="MOCK_MS=noafter"; t "A5 no commit after the switch" A5 "SCENARIO modeset FAIL" 1
has A5 "no latch after the switch"
E="MOCK_MS=nolatch"; t "A5 unlatched commit after the switch" A5 "SCENARIO modeset FAIL" 1
has A5 "latched=1 differs from commits=2"
E="MOCK_MS=norestore"; t "A5 mode not restored" A5 "SCENARIO modeset FAIL" 1
has A5 "original mode 1920x1080@60 not restored (restored=1920x1080@50)"
# The connector name is the harness's, whatever its spelling.
E="MOCK_CONN=HDMIA-1"; t "A5 harness connector name" A5 "SCENARIO modeset PASS" 0
has A5 "connector=HDMIA-1 current=1920x1080@60"
# --device and --connector reach the modeset run.
t "A5 passes device and connector" A5 "SCENARIO modeset PASS" 0 --device /dev/dri/card1 --connector DP-2
grep -q -- '--device /dev/dri/card1 --connector DP-2 --source master' "$T/out/A5/modeset.log" && ok ||
	bad "A5 lacks --device and --connector"

# ---- A6: the harness's recovery scenario
t "A6 pass" A6 "SCENARIO display-recovery PASS" 0
has A6 "VERDICT recovery PASS"
has A6 "loss=master made=ok crtc_active_after_loss=false"
has A6 "first=error:validate:Backend(Platform(13)) first_ms=0"
has A6 "reported=true still_busy=- relatched=2 crtc_active_end=true"
has A6 "recovery-before: commits=2 latched=2"
has A6 "recovery-after: commits=2 latched=2"
grep -q -- '--source master --seconds 0 recovery' "$T/out/A6/recovery.log" && ok ||
	bad "A6 does not run recovery --source master with the given --seconds"
E="MOCK_REC=fail"; t "A6 harness FAIL" A6 "SCENARIO display-recovery FAIL" 1
has A6 "recovery: exit status 1"
has A6 "recovery: VERDICT recovery FAIL buffers 7,8 still submitted or scanning"
has A6 "buffers still held after the loss: 7,8"
E="MOCK_REC=noverdict"; t "A6 no VERDICT" A6 "SCENARIO display-recovery FAIL" 1
has A6 "recovery: no VERDICT line"
E="MOCK_REC=stall"; t "A6 silent stall" A6 "SCENARIO display-recovery FAIL" 1
has A6 "the engine did not report the loss (first=silent)"
# The script's own checks hold even when the harness says PASS.
E="MOCK_REC=passbusy"; t "A6 PASS with buffers held" A6 "SCENARIO display-recovery FAIL" 1
has A6 "buffers still held after the loss: 7,8"
E="MOCK_REC=norelatch"; t "A6 no latch after the recovery" A6 "SCENARIO display-recovery FAIL" 1
has A6 "the new run latched no commit"
has A6 "recovery-after: latched=0 of commits=0"
E="MOCK_REC=late"; t "A6 commit after the loss" A6 "SCENARIO display-recovery FAIL" 1
has A6 "1 commits of the first run after the loss"
E="MOCK_REC=crtcoff"; t "A6 CRTC left off" A6 "SCENARIO display-recovery FAIL" 1
has A6 "CRTC not on at the end (crtc_active_end=false)"
# --loss, --device and --connector reach the recovery run.
t "A6 loss disable" A6 "SCENARIO display-recovery PASS" 0 --loss disable --device /dev/dri/card1 --connector DP-2
grep -q -- '--device /dev/dri/card1 --connector DP-2 --source master --seconds 0 --loss disable recovery' \
	"$T/out/A6/recovery.log" && ok || bad "A6 lacks --device, --connector or --loss"
has A6 "loss=disable made=ok"
sh "$script" --loss unplug --rows A6 "$T/harness" > /dev/null 2>&1; [ $? -eq 2 ] && ok || bad "bad loss exit"

# ---- --trace: trace files, trace-check, count comparison
for row in A1 A2 A3 A4 A5 A6; do
	case $row in
	A1) nm=startup ;; A2) nm=steady-state ;; A3) nm=stop-restart ;;
	A4) nm=lease-loss ;; A5) nm=modeset ;; A6) nm=display-recovery ;;
	esac
	t "trace $row pass" "$row" "SCENARIO $nm PASS" 0 --trace --frames 12
	has "$row" "trace: commit="
	[ "$row" != A2 ] || has A2 "trace: commit=12 latched=12 released=22 dropped=0"
	[ "$row" != A4 ] || has A4 "device_lost=1"
	case $row in
	A1) tfiles="A1/master.trace A1/kernel-lease.trace" ;;
	A2) tfiles=A2/rotate.trace ;;
	A3) tfiles="A3/TERM.trace A3/KILL.trace A3/restart-after-TERM.trace" ;;
	A4) tfiles=A4/revoke.trace ;;
	A5) tfiles=A5/modeset.trace ;;
	A6) tfiles=A6/recovery.trace ;;
	esac
	for f in $tfiles; do
		[ -s "$T/out/$f" ] && grep -q '^CETRACE v=1 ev=hdr ' "$T/out/$f" && ok || bad "no trace file $f"
	done
	for f in "$T"/out/$row/*.log; do
		case $f in */KILL.log | */TERM.log) continue ;; esac
		grep -q -- "--trace ${f%.log}.trace" "$f" 2>/dev/null && ok ||
			bad "the run of ${f##*/} lacks --trace FILE"
	done
done
# Without --trace nothing is passed and no trace line is judged.
t "no trace without the option" A1 "SCENARIO startup PASS" 0
grep -q -- '--trace' "$T/out/A1/master.log" && bad "--trace passed without the option" || ok
# A count that differs from the harness's own record fails the row.
for row in A1 A2 A4 A5 A6; do
	case $row in
	A1) nm=startup ;; A2) nm=steady-state ;; A4) nm=lease-loss ;; A5) nm=modeset ;;
	A6) nm=display-recovery ;;
	esac
	E="MOCK_TRACE=mismatch"; t "trace $row count mismatch" "$row" "SCENARIO $nm FAIL" 1 --trace --frames 12
	[ "$row" != A1 ] || has A1 "trace: latched=2 but the harness's records say 1"
done
E="MOCK_TRACE=mismatch"; t "trace A2 mismatch text" A2 "SCENARIO steady-state FAIL" 1 --trace --frames 12
has A2 "trace: latched=13 but the harness's records say 12"
# A malformed trace line fails the row, with trace-check's reason.
E="MOCK_TRACE=malformed"; t "trace malformed" A5 "SCENARIO modeset FAIL" 1 --trace
has A5 "trace: trace-check exit 1: line 3: malformed trace line"
# No trace file, or no `trace` record in the log, fails; a mismatch on a
# TERM-stopped run only needs the file.
E="MOCK_TRACE=nofile"; t "trace file missing" A6 "SCENARIO display-recovery FAIL" 1 --trace
has A6 "trace: no trace file written"
E="MOCK_TRACE=norecord"; t "trace record missing" A4 "SCENARIO lease-loss FAIL" 1 --trace
has A4 "trace: no trace record in the log"
# A trace run keeps the other checks: a count match does not hide a failed row.
E="MOCK_REV=nolost"; t "trace with a failed row" A4 "SCENARIO lease-loss FAIL" 1 --trace
# The A3 hold writes its file at once; a SIGKILL run is only parsed.
t "trace A3 KILL parsed" A3 "SCENARIO stop-restart PASS" 0 --trace
has A3 "TERM: exit=143"

# ---- picture hooks: ROW PHASE LABEL per call; A3 has post-stop and restart
# phases.
printf '#!/bin/sh\necho "$1 $2 ${3:--}" >> "%s/hook3.log"\n' "$T" > "$T/hook3"
chmod +x "$T/hook3"
hookrun() { # ROW NAME
	rm -f "$T/hook3.log"
	t "hooks $1" "$1" "SCENARIO $2 PASS" 0 --hook "$T/hook3"
}
hookrun A1 startup
[ "$(cat "$T/hook3.log")" = "A1 hold-start master
A1 hold-end master
A1 hold-start kernel-lease
A1 hold-end kernel-lease" ] && ok || { bad "A1 hook calls"; cat "$T/hook3.log" >&2; }
hookrun A3 stop-restart
[ "$(cat "$T/hook3.log")" = "A3 hold-start TERM
A3 hold-end TERM
A3 post-stop-start TERM
A3 post-stop-end TERM
A3 restart-start TERM
A3 restart-end TERM
A3 hold-start KILL
A3 hold-end KILL
A3 post-stop-start KILL
A3 post-stop-end KILL
A3 restart-start KILL
A3 restart-end KILL" ] && ok || { bad "A3 hook calls"; cat "$T/hook3.log" >&2; }
hookrun A4 lease-loss
[ "$(cat "$T/hook3.log")" = "A4 hold-start -
A4 hold-end -" ] && ok || { bad "A4 hook calls"; cat "$T/hook3.log" >&2; }
hookrun A5 modeset
[ "$(cat "$T/hook3.log")" = "A5 hold-start -
A5 hold-end -" ] && ok || { bad "A5 hook calls"; cat "$T/hook3.log" >&2; }
# A2 and A4 print no `commit` record: the hook must fire on their `step`
# record, while the harness still runs, not after --stop-wait.
printf '#!/bin/sh\necho "hook $1 $2" >> "%s/order.log"\n' "$T" > "$T/hookord"
chmod +x "$T/hookord"
for r in "A2 steady-state" "A4 lease-loss"; do
	set -- $r
	rm -f "$T/order.log"
	E="MOCK_LINGER=2 MOCK_ORDER=$T/order.log"
	t "hook order $1" "$1" "SCENARIO $2 PASS" 0 --hook "$T/hookord"
	[ "$(sed -n 1p "$T/order.log")" = "hook $1 hold-start" ] && [ "$(sed -n 2p "$T/order.log")" = "harness-done" ] \
		&& ok || { bad "$1 hold-start hook fired after the harness ended"; cat "$T/order.log" >&2; }
done
rm -f "$T/hook3.log"
hookrun A6 display-recovery
[ "$(cat "$T/hook3.log")" = "A6 hold-start -
A6 hold-end -
A6 hold-start -
A6 hold-end -" ] && ok || { bad "A6 hook calls"; cat "$T/hook3.log" >&2; }
rm -f "$T/hook3.log"
E="MOCK_REC=noverdict"; t "hooks A6 no VERDICT" A6 "SCENARIO display-recovery FAIL" 1 --hook "$T/hook3"
[ "$(wc -l < "$T/hook3.log")" -eq 4 ] && ok || { bad "A6 hook calls without VERDICT"; cat "$T/hook3.log" >&2; }
# Each A6 hold-start comes once its hold is under way: the first after a
# commit before the loss, the second after a commit of the new run.
printf '#!/bin/sh\n[ "$2" = hold-start ] && grep -c "^commit .*step=recovery-after" "%s/out/$1/recovery.log" >> "%s/hook6.log"\nexit 0\n' "$T" "$T" > "$T/hook6"
chmod +x "$T/hook6"; rm -f "$T/hook6.log"
t "A6 hold-start order" A6 "SCENARIO display-recovery PASS" 0 --hook "$T/hook6"
[ "$(sed -n 2p "$T/hook6.log")" -ge 1 ] 2>/dev/null && ok || bad "A6 second hold-start before the new run's commit"
rm -f "$T/hook3.log"
E="MOCK_MS=blocked"; t "hooks A5 blocked" A5 "SCENARIO modeset BLOCKED" 3 --hook "$T/hook3"
[ "$(cat "$T/hook3.log")" = "A5 hold-start -
A5 hold-end -" ] && ok || { bad "A5 blocked hook calls"; cat "$T/hook3.log" >&2; }
# A hook failure is only noted.
printf '#!/bin/sh\nexit 1\n' > "$T/hookfail"; chmod +x "$T/hookfail"
t "failing hook" A1 "SCENARIO startup PASS" 0 --hook "$T/hookfail"
has A1 "hook A1 hold-start master failed"
# hold-start runs while the run is under way: its log already has a commit.
printf '#!/bin/sh\n[ "$2" = hold-start ] && grep -c "^commit" "%s/out/$1/modeset.log" >> "%s/hook4.log"\nexit 0\n' "$T" "$T" > "$T/hook4"
chmod +x "$T/hook4"; rm -f "$T/hook4.log"
t "hold-start after first commit" A5 "SCENARIO modeset PASS" 0 --hook "$T/hook4"
[ "$(sed -n 1p "$T/hook4.log")" -ge 1 ] 2>/dev/null && ok || bad "hold-start before the first commit"
# --seconds reaches the A5 modeset run, and the A3 post-stop hold lasts that long.
t "A5 modeset uses --seconds" A5 "SCENARIO modeset PASS" 0 --seconds 2
grep -q -- '--seconds 2 modeset' "$T/out/A5/modeset.log" && ok || bad "A5 modeset lacks --seconds 2"
printf '#!/bin/sh\ndate +%%s >> "%s/hook5.log"\n' "$T" > "$T/hook5"; chmod +x "$T/hook5"
rm -f "$T/hook5.log"
t "A3 holds last --seconds" A3 "SCENARIO stop-restart PASS" 0 --seconds 1 --hook "$T/hook5"
# calls: hold-start, hold-end, post-stop-start, post-stop-end, ...
a=$(sed -n 3p "$T/hook5.log"); b=$(sed -n 4p "$T/hook5.log")
[ $((b - a)) -ge 1 ] && ok || bad "post-stop hold shorter than --seconds ($a..$b)"

# A3 pause (bug 2): at post-stop-start the restart has not begun yet.
printf '#!/bin/sh\n[ "$2" = post-stop-start ] && { [ -e "%s/out/A3/restart-after-$3.log" ] && echo "$3 restarted" || echo "$3 not-restarted"; } >> "%s/hook7.log"\nexit 0\n' "$T" "$T" > "$T/hook7"
chmod +x "$T/hook7"; rm -f "$T/hook7.log"
t "A3 pause before restart" A3 "SCENARIO stop-restart PASS" 0 --seconds 1 --hook "$T/hook7"
[ "$(cat "$T/hook7.log")" = "TERM not-restarted
KILL not-restarted" ] && ok || { bad "A3 post-stop hook ran after the restart began"; cat "$T/hook7.log" >&2; }
# --pause sets the post-stop hold independently of --seconds.
rm -f "$T/hook8.log"
printf '#!/bin/sh\necho "$2 $(date +%%s)" >> "%s/hook8.log"\n' "$T" > "$T/hook8"
chmod +x "$T/hook8"
t "A3 --pause" A3 "SCENARIO stop-restart PASS" 0 --seconds 0 --pause 2 --hook "$T/hook8"
a=$(grep -m1 '^post-stop-start' "$T/hook8.log" | cut -d' ' -f2); b=$(grep -m1 '^post-stop-end' "$T/hook8.log" | cut -d' ' -f2)
[ $((b - a)) -ge 1 ] && ok || bad "--pause 2 not held ($a..$b)"
sh "$script" --pause x --out "$T/o2" "$T/harness" > /dev/null 2>&1; [ $? -eq 2 ] && ok || bad "--pause x accepted"
# A5 (bug 1): the harness's own connector spelling needs no sysfs lookup, and
# a BLOCKED row names the mode count from the harness.
E="MOCK_MS=blocked MOCK_CONN=HDMIA-1"; t "A5 blocked, harness connector name" A5 "SCENARIO modeset BLOCKED" 3
has A5 "connector=HDMIA-1 current=800x480@60 modes listed=1"
has A5 "1 listed"

# ---- selection, summary, hook, usage
rm -rf "$T/out"
sh "$script" --out "$T/out" --rows A1,A4 --seconds 0 "$T/harness" > "$T/stdout" 2>&1
rc=$?
[ "$rc" -eq 0 ] && ok || bad "A1,A4 rc=$rc"
grep -q '^A1 startup PASS$' "$T/stdout" && grep -q '^A4 lease-loss PASS$' "$T/stdout" &&
	grep -q '^A2 steady-state NOT-RUN$' "$T/stdout" &&
	grep -q '^A6 display-recovery NOT-RUN$' "$T/stdout" && ok || bad "summary lines"
[ "$(sed -n 1p "$T/out/A2/result.txt")" = "SCENARIO steady-state NOT-RUN" ] && ok || bad "A2 NOT-RUN file"

printf '#!/bin/sh\necho "$1 $2" >> "%s/hook.log"\n' "$T" > "$T/hook"
chmod +x "$T/hook"
t "hook" A2 "SCENARIO steady-state PASS" 0 --frames 12 --hook "$T/hook"
[ "$(cat "$T/hook.log")" = "A2 hold-start
A2 hold-end" ] && ok || bad "hook calls"

sh "$script" > /dev/null 2>&1; [ $? -eq 2 ] && ok || bad "usage exit"
sh "$script" --rows A9 "$T/harness" > /dev/null 2>&1; [ $? -eq 2 ] && ok || bad "bad row exit"
sh "$script" "$T/nonexistent" > /dev/null 2>&1; [ $? -eq 2 ] && ok || bad "missing harness exit"

if [ "$fails" -eq 0 ]; then
	echo "board-scenarios.test.sh: $n checks passed"
	exit 0
fi
echo "board-scenarios.test.sh: $fails failed" >&2
exit 1
