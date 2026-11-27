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
#   MOCK_CONN     connector name of the selected output (default HDMI-A-1;
#                 the real harness prints e.g. HDMIA-1, DisplayPort-2)
#   MOCK_OUT2     if set, a second, unselected output record follows

here=$(cd "$(dirname "$0")" && pwd)
script=$here/board-scenarios.sh
T=$(mktemp -d "${TMPDIR:-/tmp}/board-scen-test.XXXXXX") || exit 1
trap 'rm -rf "$T"' EXIT
trap 'exit 1' INT TERM

cat > "$T/harness" <<'EOF'
#!/bin/sh
# Mock drm-harness: the scenario is the last argument.
for a; do scen=$a; done
src=master; secs=5
prev=
for a; do
	[ "$prev" = --source ] && src=$a
	[ "$prev" = --seconds ] && secs=$a
	prev=$a
done
echo "harness version=0.1.0 scenario=$scen args=\"$*\""
echo "session scenario=$scen source=$src device=/dev/dri/card0 driver=mockdrm master=yes"
echo "output index=0 id=31 connector=${MOCK_CONN:-HDMI-A-1} width=1920 height=1080 refresh_mhz=60000 mode=1920x1080 vrefresh=60 clock_khz=148500 selected=yes"
[ -n "$MOCK_OUT2" ] && echo "output index=1 id=32 connector=DSI-1 width=640 height=480 refresh_mhz=60000 mode=640x480 vrefresh=60 clock_khz=25000 selected=no"
[ "$src" = kernel-lease ] && echo "lease route=kernel device=/dev/dri/card0 lessee=1 connector=31 connector_name=HDMI-A-1 crtc=40 primary=50 overlays=51,52 objects=5"
verdict() { echo "VERDICT $1 $2 $3"; }
case $scen in
two-planes)
	mode=${MOCK_TP:-ok}
	[ "$src" = kernel-lease ] && mode=${MOCK_TP_LEASE:-$mode}
	if [ "$secs" = 600 ]; then
		case ${MOCK_HOLD:-ok} in
		nocommit) exec sleep 30 ;;
		*) echo "commit step=two-planes submission=1 validate_us=100 commit_us=200 latency_us=16700" ;;
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
	pres=$frames; drop=0; rel=$((2 * (frames - 1)))
	[ "$mode" = drop ] && { drop=2; pres=$frames; rel=$((rel + 4)); }
	[ "$mode" = norelease ] && rel=0
	[ "$mode" = noresult ] || echo "rotate_result frames=$frames buffers_per_plane=3 presented=$pres dropped=$drop missed_vblanks=0 latch_timeouts=0 releases=$rel order_violations=0 refresh_mhz=60000"
	[ "$mode" = exit1 ] && { verdict rotate FAIL "late"; exit 1; }
	verdict rotate PASS "$frames frames"
	;;
revoke)
	mode=${MOCK_REV:-ok}
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
esac
exit 0
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

# Sink with two modes, and with one.
mkdir -p "$T/sys2/card0-HDMI-A-1" "$T/sys1/card0-HDMI-A-1" "$T/sys0"
printf '1920x1080\n1280x720\n' > "$T/sys2/card0-HDMI-A-1/modes"
printf '1920x1080\n' > "$T/sys1/card0-HDMI-A-1/modes"

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

# ---- A5 (never PASS)
E="DRM_SYSFS=$T/sys1"; t "A5 single mode" A5 "SCENARIO modeset BLOCKED" 3
has A5 "only one mode"
E="DRM_SYSFS=$T/sys2"; t "A5 second mode" A5 "SCENARIO modeset BLOCKED" 3
has A5 "harness has no modeset scenario"
E="DRM_SYSFS=$T/sys0"; t "A5 no modes file" A5 "SCENARIO modeset BLOCKED" 3
E="DRM_SYSFS=$T/sys2 MOCK_TP=exit1"; t "A5 harness fails" A5 "SCENARIO modeset FAIL" 1

# A5: the harness's connector name is mapped to sysfs's (bug 1).
mkdir -p "$T/sysh/card0-HDMI-A-1" "$T/sysd/card1-DP-2" "$T/sysb/card0-DVI-I-1"
printf '1920x1080\n1280x720\n' > "$T/sysh/card0-HDMI-A-1/modes"
printf '1920x1080\n1280x720\n' > "$T/sysd/card1-DP-2/modes"
printf '1920x1080\n1280x720\n' > "$T/sysb/card0-DVI-I-1/modes"
E="DRM_SYSFS=$T/sysh MOCK_CONN=HDMIA-1"; t "A5 HDMIA-1 maps to HDMI-A-1" A5 "SCENARIO modeset BLOCKED" 3
has A5 "harness has no modeset scenario"
has A5 "connector=HDMIA-1 (sysfs HDMI-A-1)"
E="DRM_SYSFS=$T/sysd MOCK_CONN=DisplayPort-2"; t "A5 DisplayPort-2 maps to DP-2" A5 "SCENARIO modeset BLOCKED" 3 --device /dev/dri/card1
has A5 "harness has no modeset scenario"
E="DRM_SYSFS=$T/sysd MOCK_CONN=DisplayPort-2"; t "A5 DP any card" A5 "SCENARIO modeset BLOCKED" 3
has A5 "harness has no modeset scenario"
E="DRM_SYSFS=$T/sysd MOCK_CONN=DisplayPort-2"; t "A5 card number from --device" A5 "SCENARIO modeset BLOCKED" 3 --device /dev/dri/card0
has A5 "no readable modes file for connector DP-2"
E="DRM_SYSFS=$T/sysb MOCK_CONN=DVII-1"; t "A5 DVII-1 maps to DVI-I-1" A5 "SCENARIO modeset BLOCKED" 3
has A5 "harness has no modeset scenario"
E="DRM_SYSFS=$T/sysh MOCK_CONN=HDMI-A-1"; t "A5 sysfs-style name kept" A5 "SCENARIO modeset BLOCKED" 3
has A5 "harness has no modeset scenario"
# The selected mode is reported, not the last output's.
E="DRM_SYSFS=$T/sys2 MOCK_OUT2=1"; t "A5 selected mode among outputs" A5 "SCENARIO modeset BLOCKED" 3
has A5 "selected mode=1920x1080"

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
rm -f "$T/hook3.log"
E="DRM_SYSFS=$T/sys2"; t "hooks A5" A5 "SCENARIO modeset BLOCKED" 3 --hook "$T/hook3"
[ "$(cat "$T/hook3.log")" = "A5 hold-start -
A5 hold-end -" ] && ok || { bad "A5 hook calls"; cat "$T/hook3.log" >&2; }
# A hook failure is only noted.
printf '#!/bin/sh\nexit 1\n' > "$T/hookfail"; chmod +x "$T/hookfail"
t "failing hook" A1 "SCENARIO startup PASS" 0 --hook "$T/hookfail"
has A1 "hook A1 hold-start master failed"
# hold-start runs while the run is under way: its log already has a commit.
printf '#!/bin/sh\ngrep -c "^commit" "%s/out/$1/probe.log" >> "%s/hook4.log"\n' "$T" "$T" > "$T/hook4"
chmod +x "$T/hook4"; rm -f "$T/hook4.log"
E="DRM_SYSFS=$T/sys2"; t "hold-start after first commit" A5 "SCENARIO modeset BLOCKED" 3 --hook "$T/hook4"
[ "$(sed -n 1p "$T/hook4.log")" = 1 ] && ok || bad "hold-start before the first commit"
# --seconds reaches the A5 probe, and the A3 post-stop hold lasts that long.
E="DRM_SYSFS=$T/sys2"; t "A5 probe uses --seconds" A5 "SCENARIO modeset BLOCKED" 3 --seconds 2
grep -q 'seconds=2' "$T/out/A5/probe.log" && ok || bad "A5 probe lacks --seconds 2"
printf '#!/bin/sh\ndate +%%s >> "%s/hook5.log"\n' "$T" > "$T/hook5"; chmod +x "$T/hook5"
rm -f "$T/hook5.log"
t "A3 holds last --seconds" A3 "SCENARIO stop-restart PASS" 0 --seconds 1 --hook "$T/hook5"
# calls: hold-start, hold-end, post-stop-start, post-stop-end, ...
a=$(sed -n 3p "$T/hook5.log"); b=$(sed -n 4p "$T/hook5.log")
[ $((b - a)) -ge 1 ] && ok || bad "post-stop hold shorter than --seconds ($a..$b)"

# ---- selection, summary, hook, usage
rm -rf "$T/out"
sh "$script" --out "$T/out" --rows A1,A4 --seconds 0 "$T/harness" > "$T/stdout" 2>&1
rc=$?
[ "$rc" -eq 0 ] && ok || bad "A1,A4 rc=$rc"
grep -q '^A1 startup PASS$' "$T/stdout" && grep -q '^A4 lease-loss PASS$' "$T/stdout" &&
	grep -q '^A2 steady-state NOT-RUN$' "$T/stdout" && ok || bad "summary lines"
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
