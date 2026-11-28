#!/bin/sh
# Self-test for zpos-probe-expect.py with synthetic zpos_probe logs.
# Run: sh scripts/zpos-probe-expect.test.sh

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/zpos-probe-expect.py
root=$(mktemp -d "${TMPDIR:-.}/zpe-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }

# run NAME SCOPE D E C : build a log (C=none omits cross_equal), check scope.
run() {
    name=$1; want=$2; d=$3; e=$4; c=$5
    out=$root/$name.log
    echo 'DATE 2026-11-28T00:00:00Z' > "$out"
    echo 'CRTC index 0 id 80 connector 1 mode 1920x1080@60Hz' >> "$out"
    echo "ZPOS case=distinct planes=31,37 zpos=0,1 result=$d" >> "$out"
    echo "ZPOS case=equal planes=31,37 zpos=1,1 result=$e" >> "$out"
    if [ "$c" != none ]; then
        echo "ZPOS case=cross_equal planes=31,57 zpos=1,1 result=$c" >> "$out"
    fi
    echo 'VERDICT zpos_probe: DONE' >> "$out"
    if ! python3 -I "$tool" "$out" > "$root/$name.out"; then bad "$name exit"; return; fi
    if grep -qx "ZPOS scope=$want" "$root/$name.out"; then
        ok "$name scope=$want"
    else
        bad "$name scope (want $want)"; cat "$root/$name.out"
    fi
}
has() {
    if grep -qx "$2" "$root/$1.out"; then ok "$1: $2"; else bad "$1 lacks: $2"; fi
}

run perc per_crtc ok EINVAL ok
has perc 'ZPOS distinct accepted'
has perc 'ZPOS equal refused(EINVAL)'
has perc 'ZPOS cross_equal accepted'
run glob global ok EINVAL EINVAL
has glob 'ZPOS cross_equal refused(EINVAL)'
run eqok undetermined ok ok ok
run dbad undetermined EINVAL EINVAL ok
run onecrtc undetermined ok EINVAL none
has onecrtc 'ZPOS cross_equal not_run'
run other per_crtc ok EOPNOTSUPP ok
run odd global ok errno71 errno71
has odd 'ZPOS equal refused(errno71)'

# skipped lines count as not_run
cat > "$root/skip.log" <<'T'
ZPOS case=distinct result=skipped reason=1 plane(s) with mutable zpos on CRTC 0, need 2
ZPOS case=equal result=skipped reason=1 plane(s) with mutable zpos on CRTC 0, need 2
VERDICT zpos_probe: DONE
T
python3 -I "$tool" "$root/skip.log" > "$root/skip.out" || bad "skip exit"
has skip 'ZPOS distinct not_run'
has skip 'ZPOS equal not_run'
has skip 'ZPOS cross_equal not_run'
has skip 'ZPOS scope=undetermined'

# empty input on stdin
: > "$root/empty.log"
python3 -I "$tool" - < "$root/empty.log" > "$root/empty.out" || bad "empty exit"
has empty 'ZPOS scope=undetermined'

# malformed result and missing file exit 2
echo 'ZPOS case=equal planes=1,2 zpos=1,1 result=maybe' > "$root/mal.log"
python3 -I "$tool" "$root/mal.log" > /dev/null 2>&1
if [ $? -eq 2 ]; then ok "malformed exits 2"; else bad "malformed exit"; fi
python3 -I "$tool" "$root/none.log" > /dev/null 2>&1
if [ $? -eq 2 ]; then ok "missing file exits 2"; else bad "missing file exit"; fi

exit $fail
