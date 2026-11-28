#!/bin/sh
# Self-test for mode-test-expect.py.
# Run: bash scripts/mode-test-expect.test.sh
#
# Sample logs follow the output format asserted in the unit tests of
# crates/composition-drm/examples/mode_test.rs (lines); no board data.

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/mode-test-expect.py
fail=0
tmp=$(mktemp -d) || exit 1
trap 'rm -rf "$tmp"' EXIT
ok() { echo "ok: $1"; }

cat > "$tmp/all-ok.txt" <<'X'
DATE 2026-11-28T12:00:00Z
KERNEL 6.6.0-mock
DEVICE /dev/dri/card0
CONNECTOR 32 crtc 60 modes 2
LISTED 1920x1080 clock=148500 htotal=2200 vtotal=1125 vrefresh=60 flags=0x5 preferred
mode 640x480@60 clock=25200 test=ok
mode 1920x1080@60 clock=148500 test=ok
mode_test: tested=2 ok=2 highest_ok_clock=148500
X

cat > "$tmp/mixed.txt" <<'X'
DATE 2026-11-28T12:00:00Z
KERNEL 6.6.0-mock
DEVICE /dev/dri/card0
CONNECTOR 32 crtc 60 modes 3
mode 640x480@60 clock=25200 test=ok
mode 1920x1080@60 clock=148500 test=ok
mode 3840x2160@60 clock=594000 test=refused errno=28
mode_test: tested=3 ok=2 highest_ok_clock=148500
X

cat > "$tmp/none-ok.txt" <<'X'
mode 3840x2160@60 clock=594000 test=refused errno=22
mode 1920x1080@120 clock=300000 test=refused errno=22
mode_test: tested=2 ok=0 highest_ok_clock=0
X

cat > "$tmp/no-master.txt" <<'X'
DATE 2026-11-28T12:00:00Z
KERNEL 6.6.0-mock
DEVICE /dev/dri/card0
mode_test: DRM master is unavailable: Device or resource busy (16). Another process (a desktop or compositor) probably holds it; stop it for the run. This test never stops other processes
X

cat > "$tmp/truncated.txt" <<'X'
mode 640x480@60 clock=25200 test=ok
mode_test: dumb buffer 3840x2160: ENOMEM
X

cat > "$tmp/mode-then-master.txt" <<'X'
mode 640x480@60 clock=25200 test=ok
mode_test: DRM master is unavailable: Device or resource busy (16)
X

cat > "$tmp/master-and-summary.txt" <<'X'
mode 640x480@60 clock=25200 test=ok
mode_test: DRM master is unavailable: Device or resource busy (16)
mode_test: tested=1 ok=1 highest_ok_clock=25200
X

printf 'no mode lines here\n' > "$tmp/empty.txt"
printf 'mode 1920x1080@60 clock=abc test=ok\n' > "$tmp/bad-clock.txt"
printf 'mode 1920x1080 clock=148500 test=ok\n' > "$tmp/bad-name.txt"
printf 'mode 1920x1080@60 clock=148500 test=maybe\n' > "$tmp/bad-test.txt"
printf 'mode_test: tested=x ok=0 highest_ok_clock=0\n' > "$tmp/bad-summary.txt"
cat "$tmp/all-ok.txt" > "$tmp/two-summary.txt"
printf 'mode_test: tested=0 ok=0 highest_ok_clock=0\n' >> "$tmp/two-summary.txt"
printf 'mode 640x480@60 clock=25200 test=ok\nmode_test: tested=2 ok=2 highest_ok_clock=25200\n' > "$tmp/disagree.txt"

check() { # NAME WANT_RC PATTERN ARGS...
    name=$1 want=$2 pat=$3
    shift 3
    got=$(python3 -I "$tool" "$@" 2>&1)
    rc=$?
    if [ "$rc" != "$want" ]; then
        echo "FAIL $name: exit $rc, want $want"; printf '%s\n' "$got"; fail=1
    elif [ -n "$pat" ] && ! printf '%s\n' "$got" | grep -Fq -- "$pat"; then
        echo "FAIL $name: output lacks '$pat'"; printf '%s\n' "$got"; fail=1
    else
        echo "ok   $name"
    fi
}

check_lines() { # NAME FILE EXACT-STDOUT-LINES...
    name=$1 file=$2
    shift 2
    python3 -I "$tool" "$file" > "$tmp/got.txt" 2>"$tmp/err.txt"
    rc=$?
    printf '%s\n' "$@" > "$tmp/want.txt"
    if [ "$rc" != 0 ] || ! cmp -s "$tmp/got.txt" "$tmp/want.txt"; then
        echo "FAIL $name: exit $rc"; cat "$tmp/got.txt" "$tmp/err.txt"; fail=1
    else
        echo "ok   $name"
    fi
}

# all modes pass / some refused / nothing passes
check_lines all-ok "$tmp/all-ok.txt" \
    "MODE name=640x480@60 clock=25200 result=ok" \
    "MODE name=1920x1080@60 clock=148500 result=ok" \
    "MODE highest_ok_clock=148500" \
    "MODE refused=0"
check_lines mixed "$tmp/mixed.txt" \
    "MODE name=640x480@60 clock=25200 result=ok" \
    "MODE name=1920x1080@60 clock=148500 result=ok" \
    "MODE name=3840x2160@60 clock=594000 result=28" \
    "MODE highest_ok_clock=148500" \
    "MODE refused=1"
check_lines none-ok "$tmp/none-ok.txt" \
    "MODE name=3840x2160@60 clock=594000 result=22" \
    "MODE name=1920x1080@120 clock=300000 result=22" \
    "MODE highest_ok_clock=0" \
    "MODE refused=2"

# --expect-min-clock: ok at the boundary, fail above it
check all-ok-expect 0 "EXPECT ok" --expect-min-clock 148500 "$tmp/all-ok.txt"
check mixed-expect-bound 0 "EXPECT ok" --expect-min-clock 148500 "$tmp/mixed.txt"
check mixed-expect-fail 1 "EXPECT fail" --expect-min-clock 148501 "$tmp/mixed.txt"
check none-ok-expect 1 "EXPECT fail" --expect-min-clock 1 "$tmp/none-ok.txt"
check none-ok-expect-zero 0 "EXPECT ok" --expect-min-clock 0 "$tmp/none-ok.txt"
python3 -I "$tool" "$tmp/mixed.txt" > "$tmp/got.txt" 2>&1
rc=$?
if [ "$rc" != 0 ] || grep -q "EXPECT" "$tmp/got.txt"; then
    echo "FAIL no-expect-no-line"; cat "$tmp/got.txt"; fail=1
else
    echo "ok   no-expect-no-line"
fi

# a run without DRM master: nothing tested, no EXPECT even when asked
python3 -I "$tool" "$tmp/no-master.txt" --expect-min-clock 1 > "$tmp/got.txt" 2>&1
rc=$?
if [ "$rc" != 1 ] || ! grep -q "^MODE incomplete reason=drm-master$" "$tmp/got.txt" \
        || grep -q "EXPECT" "$tmp/got.txt"; then
    echo "FAIL no-master"; cat "$tmp/got.txt"; fail=1
else
    echo "ok   no-master"
fi
grep -q "^MODE highest_ok_clock=0$" "$tmp/got.txt" &&
    grep -q "^MODE refused=0$" "$tmp/got.txt" && ok "no-master counts" || {
    echo "FAIL no-master counts"; fail=1; }

# unfinished runs and malformed input
check truncated 1 "MODE incomplete reason=no-summary" "$tmp/truncated.txt"
check truncated-echoes 1 "dumb buffer" "$tmp/truncated.txt"
check mode-then-master 1 "MODE incomplete reason=drm-master" "$tmp/mode-then-master.txt"
check no-lines 1 "MODE incomplete reason=no-summary" "$tmp/empty.txt"
check bad-clock 2 "bad mode line" "$tmp/bad-clock.txt"
check bad-name 2 "bad mode line" "$tmp/bad-name.txt"
check bad-test 2 "bad mode line" "$tmp/bad-test.txt"
check bad-summary 2 "bad summary line" "$tmp/bad-summary.txt"
check two-summaries 2 "mixes runs" "$tmp/two-summary.txt"
check summary-disagree 2 "disagrees" "$tmp/disagree.txt"
check master-and-summary 2 "error line and a summary line" "$tmp/master-and-summary.txt"

check stdin 0 "EXPECT ok" --expect-min-clock 148500 - < "$tmp/all-ok.txt"
check missing-file 2 "No such file" "$tmp/absent.txt"
check bad-option 2 "" --bogus
check negative-clock 2 "must be >= 0" --expect-min-clock -1 "$tmp/all-ok.txt"
check nonnumeric-clock 2 "" --expect-min-clock abc "$tmp/all-ok.txt"

[ "$fail" = 0 ] && echo "all passed"
exit "$fail"
