#!/bin/sh
# Self-test for scale-probe-expect.py. Run: bash scripts/scale-probe-expect.test.sh
#
# All fixtures are synthetic text following the SCALE and RESULT line formats
# of crates/composition-drm/examples/scale_probe.rs. No board data.

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/scale-probe-expect.py
tmp=$(mktemp -d "${TMPDIR:-.}/spe-test.XXXXXX") || exit 2
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
fail=0

# check NAME WANT_RC PATTERN ARGS...  (stdin comes from stdin of the caller)
check() {
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

# good.txt: one plane, three axes, ladder plus binary search, downscales.
# Accepted per axis: h up to 2.5000 (first ladder rejection 3.0000, binary
# 2.5000 accepted, 2.7500 refused); v up to 3.5000 (4.0000 refused, 3.5000
# accepted); both up to 2.2500 (3.0000 and 2.5000 refused, 2.2500 accepted).
# Downscales: 0.5000 refused on h and both, accepted on v and everywhere 0.75
# and 0.90 are accepted.
cat > "$tmp/good.txt" <<'X'
DATE 2026-11-28T10:00:00Z
KERNEL 6.6.0
DEVICE /dev/dri/card0
MODE 200x200@60Hz connector 120 crtc 100
PLANE id 42 Overlay possible_on_crtc true formats XR24
SCALE plane=42 axis=h src=100.0000x100.0000 dst=100x100 factor=1.0000 test_only=accepted errno=0
SCALE plane=42 axis=h src=80.0000x100.0000 dst=100x100 factor=1.2500 test_only=accepted errno=0
SCALE plane=42 axis=h src=66.6667x100.0000 dst=100x100 factor=1.5000 test_only=accepted errno=0
SCALE plane=42 axis=h src=50.0000x100.0000 dst=100x100 factor=2.0000 test_only=accepted errno=0
SCALE plane=42 axis=h src=33.3333x100.0000 dst=100x100 factor=3.0000 test_only=rejected errno=22
SCALE plane=42 axis=h src=40.0000x100.0000 dst=100x100 factor=2.5000 test_only=accepted errno=0
SCALE plane=42 axis=h src=36.3636x100.0000 dst=100x100 factor=2.7500 test_only=rejected errno=22
SCALE plane=42 axis=v src=100.0000x100.0000 dst=100x100 factor=1.0000 test_only=accepted errno=0
SCALE plane=42 axis=v src=100.0000x80.0000 dst=100x100 factor=1.2500 test_only=accepted errno=0
SCALE plane=42 axis=v src=100.0000x66.6667 dst=100x100 factor=1.5000 test_only=accepted errno=0
SCALE plane=42 axis=v src=100.0000x50.0000 dst=100x100 factor=2.0000 test_only=accepted errno=0
SCALE plane=42 axis=v src=100.0000x33.3333 dst=100x100 factor=3.0000 test_only=accepted errno=0
SCALE plane=42 axis=v src=100.0000x25.0000 dst=100x100 factor=4.0000 test_only=rejected errno=22
SCALE plane=42 axis=v src=100.0000x28.5714 dst=100x100 factor=3.5000 test_only=accepted errno=0
SCALE plane=42 axis=both src=100.0000x100.0000 dst=100x100 factor=1.0000 test_only=accepted errno=0
SCALE plane=42 axis=both src=80.0000x80.0000 dst=100x100 factor=1.2500 test_only=accepted errno=0
SCALE plane=42 axis=both src=66.6667x66.6667 dst=100x100 factor=1.5000 test_only=accepted errno=0
SCALE plane=42 axis=both src=50.0000x50.0000 dst=100x100 factor=2.0000 test_only=accepted errno=0
SCALE plane=42 axis=both src=33.3333x33.3333 dst=100x100 factor=3.0000 test_only=rejected errno=22
SCALE plane=42 axis=both src=40.0000x40.0000 dst=100x100 factor=2.5000 test_only=rejected errno=22
SCALE plane=42 axis=both src=44.4444x44.4444 dst=100x100 factor=2.2500 test_only=accepted errno=0
SCALE plane=42 axis=h src=200.0000x100.0000 dst=100x100 factor=0.5000 test_only=rejected errno=22
SCALE plane=42 axis=h src=133.3333x100.0000 dst=100x100 factor=0.7500 test_only=accepted errno=0
SCALE plane=42 axis=h src=111.1111x100.0000 dst=100x100 factor=0.9000 test_only=accepted errno=0
SCALE plane=42 axis=v src=100.0000x200.0000 dst=100x100 factor=0.5000 test_only=accepted errno=0
SCALE plane=42 axis=v src=100.0000x133.3333 dst=100x100 factor=0.7500 test_only=accepted errno=0
SCALE plane=42 axis=v src=100.0000x111.1111 dst=100x100 factor=0.9000 test_only=accepted errno=0
SCALE plane=42 axis=both src=200.0000x200.0000 dst=100x100 factor=0.5000 test_only=rejected errno=22
SCALE plane=42 axis=both src=133.3333x133.3333 dst=100x100 factor=0.7500 test_only=accepted errno=0
SCALE plane=42 axis=both src=111.1111x111.1111 dst=100x100 factor=0.9000 test_only=accepted errno=0
RESULT plane=42 kind=Overlay format=XR24 max_upscale_h=2.5000 max_upscale_v=3.5000 max_upscale_both=2.2500 downscale=partial min_downscale=0.5000
VERDICT scale_probe: OK 1 plane(s) probed at dst 100x100, resolution 256/65536
X

python3 -I "$tool" "$tmp/good.txt" > "$tmp/good.out" 2>&1
[ $? -eq 0 ] || { echo "FAIL good-exit"; fail=1; }
want() {
    if grep -Fxq -- "$1" "$tmp/good.out"; then
        echo "ok   $2"
    else
        echo "FAIL $2: missing '$1'"
        cat "$tmp/good.out"
        fail=1
    fi
}
want "VERDICT: PASS" good-pass
want "SCALE plane=42 axis=h up_max=2.5000 down_min=0.7500" good-h
want "SCALE plane=42 axis=v up_max=3.5000 down_min=0.5000" good-v
want "SCALE plane=42 axis=both up_max=2.2500 down_min=0.7500" good-both
want "SUMMARY planes=1 judged=3 expect=0 ok=0 fail=0 unproven=0 non_monotonic=0 notes=0" good-summary

check claims-fail 1 "EXPECT plane=42 axis=h up_max=2.5000 profile_up=2.0000 down_min=0.7500 profile_down=2.0000 fail reason=down_refused_at=0.5000" "$tmp/good.txt" --profile-up 2 --profile-down 2
check claims-mixed 1 "EXPECT plane=42 axis=v up_max=3.5000 profile_up=2.0000 down_min=0.5000 profile_down=2.0000 ok" "$tmp/good.txt" --profile-up 2 --profile-down 2
check claims-count 1 "SUMMARY planes=1 judged=3 expect=3 ok=1 fail=2 unproven=0 non_monotonic=0 notes=0" "$tmp/good.txt" --profile-up 2 --profile-down 2
check claims-refused 1 "up_refused_at=4.0000" "$tmp/good.txt" --profile-up 4 --profile-down 1
check claims-pass 0 "VERDICT: PASS" "$tmp/good.txt" --profile-up 2 --profile-down 1

# unproven.txt: ladder ends without a rejection at 16.0000 (RESULT 16.0000+);
# a claim of 32 was not tested. Axis h has test lines; v and both are judged
# from RESULT.
cat > "$tmp/unproven.txt" <<'X'
SCALE plane=47 axis=h src=100.0000x100.0000 dst=100x100 factor=1.0000 test_only=accepted errno=0
SCALE plane=47 axis=h src=50.0000x100.0000 dst=100x100 factor=2.0000 test_only=accepted errno=0
SCALE plane=47 axis=h src=12.5000x100.0000 dst=100x100 factor=8.0000 test_only=accepted errno=0
SCALE plane=47 axis=h src=6.2500x100.0000 dst=100x100 factor=16.0000 test_only=accepted errno=0
SCALE plane=47 axis=h src=200.0000x100.0000 dst=100x100 factor=0.5000 test_only=accepted errno=0
SCALE plane=47 axis=h src=133.3333x100.0000 dst=100x100 factor=0.7500 test_only=accepted errno=0
SCALE plane=47 axis=h src=111.1111x100.0000 dst=100x100 factor=0.9000 test_only=accepted errno=0
NOTE plane=47 axis=h no rejection up to 16.0000 (limited by --max-factor)
RESULT plane=47 kind=Primary format=XR24 max_upscale_h=16.0000+ max_upscale_v=16.0000+ max_upscale_both=16.0000+ downscale=accepted min_downscale=0.5000
X
check unproven 1 "EXPECT plane=47 axis=h up_max=16.0000 profile_up=32.0000 unproven reason=up_unproven" "$tmp/unproven.txt" --profile-up 32
check unproven-fallback 0 "SCALE plane=47 axis=v up_max=16.0000+ down_min=0.5000" "$tmp/unproven.txt"
check proven 0 "VERDICT: PASS" "$tmp/unproven.txt" --profile-up 16 --profile-down 2

# gap.txt: 2.0000 refused between accepted 1.0000 and 3.0000.
cat > "$tmp/gap.txt" <<'X'
SCALE plane=45 axis=h src=100.0000x100.0000 dst=100x100 factor=1.0000 test_only=accepted errno=0
SCALE plane=45 axis=h src=50.0000x100.0000 dst=100x100 factor=2.0000 test_only=rejected errno=22
SCALE plane=45 axis=h src=33.3333x100.0000 dst=100x100 factor=3.0000 test_only=accepted errno=0
X
check gap 1 "SCALE plane=45 axis=h non_monotonic" "$tmp/gap.txt"
check gap-values 1 "SCALE plane=45 axis=h up_max=3.0000 down_min=none" "$tmp/gap.txt"

# conflict.txt: the same factor accepted in one run and refused in another.
cat > "$tmp/conflict.txt" <<'X'
SCALE plane=46 axis=h src=100.0000x100.0000 dst=100x100 factor=1.0000 test_only=accepted errno=0
SCALE plane=46 axis=h src=50.0000x100.0000 dst=100x100 factor=2.0000 test_only=accepted errno=0
SCALE plane=46 axis=h src=33.3333x100.0000 dst=100x100 factor=3.0000 test_only=rejected errno=22
SCALE plane=46 axis=h src=50.0000x100.0000 dst=100x100 factor=2.0000 test_only=rejected errno=22
X
check conflict 1 "SCALE plane=46 axis=h non_monotonic" "$tmp/conflict.txt"
check two-files 1 "non_monotonic=2" "$tmp/gap.txt" "$tmp/conflict.txt"

# refused_note.txt: the tests refuse 2.5000 although RESULT claims a maximum
# of 3.0000; a NOTE line reports it and a gap flag also fires.
cat > "$tmp/refused_note.txt" <<'X'
SCALE plane=50 axis=h src=100.0000x100.0000 dst=100x100 factor=1.0000 test_only=accepted errno=0
SCALE plane=50 axis=h src=50.0000x100.0000 dst=100x100 factor=2.0000 test_only=accepted errno=0
SCALE plane=50 axis=h src=33.3333x100.0000 dst=100x100 factor=3.0000 test_only=accepted errno=0
SCALE plane=50 axis=h src=40.0000x100.0000 dst=100x100 factor=2.5000 test_only=rejected errno=22
RESULT plane=50 kind=Overlay format=XR24 max_upscale_h=3.0000 max_upscale_v=3.0000 max_upscale_both=3.0000 downscale=untested min_downscale=none
X
check refused-note 1 "NOTE plane=50 axis=h result_max_upscale=3.0000 measured_refused_at=2.5000" "$tmp/refused_note.txt"

# mismatch.txt: tests accepted up to 2.5000 although RESULT says 3.0000, and
# the accepted minimum downscale is 0.7500 although RESULT says 0.5000.
cat > "$tmp/mismatch.txt" <<'X'
SCALE plane=49 axis=h src=100.0000x100.0000 dst=100x100 factor=1.0000 test_only=accepted errno=0
SCALE plane=49 axis=h src=40.0000x100.0000 dst=100x100 factor=2.5000 test_only=accepted errno=0
SCALE plane=49 axis=h src=133.3333x100.0000 dst=100x100 factor=0.7500 test_only=accepted errno=0
RESULT plane=49 kind=Primary format=XR24 max_upscale_h=3.0000 max_upscale_v=3.0000 max_upscale_both=3.0000 downscale=accepted min_downscale=0.5000
X
check mismatch-max 0 "NOTE plane=49 axis=h result_max_upscale=3.0000 measured_up_max=2.5000" "$tmp/mismatch.txt"
check mismatch-down 0 "NOTE plane=49 result_min_downscale=0.5000 measured_down_min=0.7500" "$tmp/mismatch.txt"
check mismatch-pass 0 "VERDICT: PASS" "$tmp/mismatch.txt"

# mixdst.txt: two --dst runs of one plane; groups are judged apart and each
# line carries a dst token.
cat > "$tmp/mixdst.txt" <<'X'
SCALE plane=51 axis=h src=100.0000x100.0000 dst=100x100 factor=1.0000 test_only=accepted errno=0
SCALE plane=51 axis=h src=40.0000x100.0000 dst=100x100 factor=2.5000 test_only=accepted errno=0
SCALE plane=51 axis=h src=200.0000x200.0000 dst=200x200 factor=1.0000 test_only=accepted errno=0
SCALE plane=51 axis=h src=88.8889x88.8889 dst=200x200 factor=2.2500 test_only=accepted errno=0
SCALE plane=51 axis=h src=80.0000x80.0000 dst=200x200 factor=2.5000 test_only=rejected errno=22
X
check mixdst-a 0 "SCALE plane=51 axis=h dst=100x100 up_max=2.5000 down_min=none" "$tmp/mixdst.txt"
check mixdst-b 0 "SCALE plane=51 axis=h dst=200x200 up_max=2.2500 down_min=none" "$tmp/mixdst.txt"
python3 -I "$tool" "$tmp/mixdst.txt" | grep -q non_monotonic && { echo "FAIL mixdst-no-flag"; fail=1; } || echo "ok   mixdst-no-flag"

# fallback.txt: a RESULT line only.
printf 'RESULT plane=48 kind=Primary format=XR24 max_upscale_h=8.0000 max_upscale_v=none max_upscale_both=8.0000 downscale=rejected min_downscale=none\n' > "$tmp/fallback.txt"
check fallback 0 "SCALE plane=48 axis=v up_max=none down_min=none" "$tmp/fallback.txt"
check fallback-claim 1 "EXPECT plane=48 axis=h up_max=8.0000 profile_up=32.0000 unproven reason=up_unproven" "$tmp/fallback.txt" --profile-up 32

# stdin
check stdin 0 "VERDICT: PASS" - < "$tmp/good.txt"

# input and usage errors
cat > "$tmp/dateonly.txt" <<'X'
DATE 2026-11-28T10:00:00Z
KERNEL 6.6.0
X
cat > "$tmp/badfactor.txt" <<'X'
SCALE plane=42 axis=h src=100.0000x100.0000 dst=100x100 factor=abc test_only=accepted errno=0
X
cat > "$tmp/badaxis.txt" <<'X'
SCALE plane=42 axis=x src=100.0000x100.0000 dst=100x100 factor=1.0000 test_only=accepted errno=0
X
cat > "$tmp/badresult.txt" <<'X'
RESULT plane=42 max_upscale_h=zz max_upscale_v=none max_upscale_both=none min_downscale=none
X
check missing-file 2 "cannot read input" "$tmp/absent.txt"
check no-lines 1 "no SCALE or RESULT lines" "$tmp/dateonly.txt"
check bad-option 2 "" --bogus
check bad-claim 2 "at least 1" --profile-up 0 "$tmp/good.txt"
check bad-factor 2 "bad SCALE line" "$tmp/badfactor.txt"
check bad-axis 2 "bad SCALE axis" "$tmp/badaxis.txt"
check bad-result 2 "bad number" "$tmp/badresult.txt"

[ "$fail" = 0 ] && echo "all passed"
exit "$fail"
