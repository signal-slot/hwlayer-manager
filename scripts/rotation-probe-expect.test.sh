#!/bin/bash
# Self-test for rotation-probe-expect.py. Run: bash scripts/rotation-probe-expect.test.sh
#
# All fixtures are synthetic text following the ROT line format of the
# rotation_probe example. No board data.

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/rotation-probe-expect.py
tmp=$(mktemp -d "${TMPDIR:-.}/rpe-test.XXXXXX") || exit 2
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
fail=0

# check NAME WANT_RC PATTERN ARGS...
check() {
    local name=$1 want=$2 pat=$3 got rc
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

# bits per value (synthetic numbers; only the 0x form matters to the parser)
bits() {
    case $1 in
        rotate-0) echo 0x1 ;;
        rotate-90) echo 0x2 ;;
        rotate-180) echo 0x4 ;;
        rotate-270) echo 0x8 ;;
        reflect-x) echo 0x10 ;;
        reflect-y) echo 0x20 ;;
        rotate-90\|reflect-x) echo 0x12 ;;
        *) echo 0x31 ;;
    esac
}
ALL=(rotate-0 rotate-90 rotate-180 rotate-270 reflect-x reflect-y
     "rotate-90|reflect-x" "rotate-0|reflect-x|reflect-y")

# plane ID TYPE ACCEPTED...  : one ROT line per value, ok for those listed
# and errno=22 for the rest.
plane() {
    local id=$1 type=$2 v a r
    shift 2
    for v in "${ALL[@]}"; do
        r="errno=22"
        for a in "$@"; do [ "$a" = "$v" ] && r=ok; done
        echo "ROT plane=$id type=$type value=$v bits=$(bits "$v") result=$r"
    done
}

CLUSTER_OK=("${ALL[@]}")
ESMART_OK=(rotate-0 reflect-y)
HVS_OK=(rotate-0 rotate-180 reflect-x reflect-y "rotate-0|reflect-x|reflect-y")

# rk-good: a Cluster and an Esmart plane, both as source says.
{
    echo "DATE 2026-11-28T10:00:00Z"
    plane 71 Primary "${CLUSTER_OK[@]}"
    plane 72 Overlay "${ESMART_OK[@]}"
    echo "SUMMARY planes=2 tests=16"
} > "$tmp/rk-good.txt"
check rk-match 0 "VERDICT match" --profile rk3588 "$tmp/rk-good.txt"
check rk-match-plane-cluster 0 "VERDICT plane=71 match class=cluster" --profile rk3588 "$tmp/rk-good.txt"
check rk-match-plane-esmart 0 "VERDICT plane=72 match class=esmart" --profile rk3588 "$tmp/rk-good.txt"
check table-line 0 "TABLE plane=72 type=Overlay accepted=rotate-0,reflect-y refused=rotate-90(errno=22)" "$tmp/rk-good.txt"
check rk-explicit-class 0 "VERDICT plane=72 match class=esmart" --profile rk3588 --cluster 71 --esmart 72 "$tmp/rk-good.txt"

# RK split: an Esmart plane that accepts everything is a mismatch when the
# plane is declared Esmart, and match when declared Cluster.
plane 73 Overlay "${CLUSTER_OK[@]}" > "$tmp/rk-wide.txt"
check rk-esmart-too-wide 1 "VERDICT plane=73 mismatch accepted_unexpected=rotate-90,rotate-180,rotate-270,reflect-x,rotate-90|reflect-x,rotate-0|reflect-x|reflect-y" --profile rk3588 --esmart 73 "$tmp/rk-wide.txt"
check rk-cluster-declared 0 "VERDICT match" --profile rk3588 --cluster 73 "$tmp/rk-wide.txt"
# A Cluster plane that refuses reflect-x and rotate-90 (inferred as Esmart
# because rotate-90 is refused) is a mismatch on reflect-x alone.
plane 74 Primary rotate-0 reflect-y "rotate-0|reflect-x|reflect-y" reflect-x > "$tmp/rk-narrow.txt"
check rk-narrow 1 "VERDICT plane=74 mismatch" --profile rk3588 "$tmp/rk-narrow.txt"
check rk-cluster-narrow 1 "refused_unexpected=rotate-90,rotate-180,rotate-270,rotate-90|reflect-x" --profile rk3588 --cluster 74 "$tmp/rk-narrow.txt"

# hvs: 0, 180, reflect-x, reflect-y and their combination; no 90/270.
{
    plane 40 Primary "${HVS_OK[@]}"
    plane 41 Cursor "${HVS_OK[@]}"
} > "$tmp/hvs-good.txt"
check hvs6-match 0 "VERDICT match" --profile hvs6 "$tmp/hvs-good.txt"
check hvs5-match 0 "VERDICT plane=41 match class=hvs5" --profile hvs5 "$tmp/hvs-good.txt"
plane 42 Overlay "${HVS_OK[@]}" rotate-90 > "$tmp/hvs-bad.txt"
check hvs-90-accepted 1 "accepted_unexpected=rotate-90" --profile hvs6 "$tmp/hvs-bad.txt"
check hvs-mismatch-final 1 "VERDICT mismatch" --profile hvs5 "$tmp/hvs-bad.txt"

# errno parsing: the number is read from result=errno=<n>.
{
    echo "ROT plane=9 type=Primary value=rotate-0 bits=0x1 result=ok"
    echo "ROT plane=9 type=Primary value=rotate-90 bits=0x2 result=errno=95"
} > "$tmp/errno.txt"
check errno-parsed 1 "refused=rotate-90(errno=95)" --profile hvs6 "$tmp/errno.txt"
check errno-incomplete 1 "VERDICT plane=9 incomplete missing=rotate-180," --profile hvs6 "$tmp/errno.txt"
check errno-final-incomplete 1 "VERDICT incomplete" --profile hvs6 "$tmp/errno.txt"
check errno-no-profile 1 "VERDICT incomplete" "$tmp/errno.txt"
check complete-no-profile 0 "VERDICT unchecked" "$tmp/hvs-good.txt"
# mismatch outranks incomplete
{
    echo "ROT plane=9 type=Primary value=rotate-90 bits=0x2 result=ok"
} > "$tmp/mix.txt"
check mismatch-over-incomplete 1 "VERDICT mismatch" --profile hvs6 "$tmp/mix.txt"

# no ROT lines for a plane named by a PLANE line: incomplete
{
    echo "PLANE id 55 Overlay possible_on_crtc true"
    plane 40 Primary "${HVS_OK[@]}"
} > "$tmp/noplane.txt"
check plane-no-rot 1 "VERDICT plane=55 incomplete" --profile hvs6 "$tmp/noplane.txt"

# conflicting duplicate results (two runs concatenated)
{
    echo "ROT plane=8 type=Primary value=rotate-0 bits=0x1 result=ok"
    echo "ROT plane=8 type=Primary value=rotate-0 bits=0x1 result=errno=22"
} > "$tmp/conflict.txt"
check conflict 1 "conflicting=rotate-0" --profile hvs6 "$tmp/conflict.txt"

# unknown lines are ignored
{
    echo "KERNEL 6.18.0"
    echo "garbage line"
    cat "$tmp/hvs-good.txt"
} > "$tmp/noise.txt"
check unknown-ignored 0 "VERDICT match" --profile hvs6 "$tmp/noise.txt"
check stdin 0 "VERDICT match" --profile hvs6 - < "$tmp/hvs-good.txt"

# empty and unparsable input: exit 2
: > "$tmp/empty.txt"
echo "KERNEL 6.18.0" > "$tmp/nolines.txt"
echo "ROT plane=abc type=Primary value=rotate-0 bits=0x1 result=ok" > "$tmp/badplane.txt"
echo "ROT plane=1 type=Primary value=rotate-0 bits=0x1 result=errno=zz" > "$tmp/baderrno.txt"
echo "ROT plane=1 type=Sprite value=rotate-0 bits=0x1 result=ok" > "$tmp/badtype.txt"
echo "ROT plane=1 type=Primary value=rotate-0 bits=0xzz result=ok" > "$tmp/badbits.txt"
check empty 2 "no ROT or PLANE lines" --profile hvs6 "$tmp/empty.txt"
check no-lines 2 "no ROT or PLANE lines" "$tmp/nolines.txt"
check bad-plane 2 "bad ROT line" "$tmp/badplane.txt"
check bad-errno 2 "bad ROT result" "$tmp/baderrno.txt"
check bad-type 2 "bad ROT type" "$tmp/badtype.txt"
check bad-bits 2 "bad ROT line" "$tmp/badbits.txt"
check missing-file 2 "cannot read input" "$tmp/absent.txt"
check bad-option 2 "" --bogus
check bad-profile 2 "" --profile rk3399 "$tmp/hvs-good.txt"
check bad-id-list 2 "bad plane list" --profile rk3588 --cluster x "$tmp/hvs-good.txt"

[ "$fail" = 0 ] && echo "all passed"
exit "$fail"
