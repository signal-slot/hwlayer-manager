#!/bin/sh
# Self-test for pending-probe-expect.py.
# Run: bash scripts/pending-probe-expect.test.sh
#
# Synthetic logs follow the line formats in the header of
# crates/composition-drm/examples/pending_probe.rs; no board data.

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/pending-probe-expect.py
fail=0
tmp=$(mktemp -d) || exit 1
trap 'rm -rf "$tmp"' EXIT

cat > "$tmp/flips.txt" <<'X'
SETUP commit=ok
SETUP events=1
FLIP n=1 fb=77 commit=ok seq=10 ts_us=100 vblank_before=9 vblank_after=10 submit_to_event_us=16700
FLIP n=2 fb=78 commit=ok seq=11 ts_us=200 vblank_before=10 vblank_after=11 submit_to_event_us=16700
SUMMARY flips: submitted=2 events=2 missed=0 mean_interval_us=100 seq_gaps=0
RESTORE ok
VERDICT pending_probe: DONE
X
cat > "$tmp/missed.txt" <<'X'
FLIP n=1 fb=77 commit=ok seq=10 ts_us=100 vblank_before=9 vblank_after=10 submit_to_event_us=1
FLIP n=2 fb=78 commit=ok seq=n/a ts_us=n/a vblank_before=10 vblank_after=n/a submit_to_event_us=n/a
SUMMARY flips: submitted=2 events=1 missed=1 mean_interval_us=n/a seq_gaps=0
VERDICT pending_probe: DONE
X
head -3 "$tmp/flips.txt" > "$tmp/trunc-flips.txt"
cat > "$tmp/ebusy.txt" <<'X'
A commit=ok fb=78
B_TEST_ONLY commit=EBUSY fb=79
B commit=EBUSY fb=79
EVENT n=1 seq=5 ts_us=1
EVENTS arrived=1 expected=1 a_event=yes b_event=n/a
PLANE_FB plane=42 fb=78 shows=A
VERDICT replace_pending: refused errno=EBUSY
VERDICT pending_probe: DONE
X
sed -e 's/B commit=EBUSY/B commit=ok/' -e 's/^VERDICT replace_pending.*/VERDICT replace_pending: accepted/' \
    "$tmp/ebusy.txt" > "$tmp/accepted.txt"
sed -e 's/B commit=EBUSY/B commit=EINVAL/' -e 's/errno=EBUSY/errno=EINVAL/' \
    "$tmp/ebusy.txt" > "$tmp/einval.txt"
head -3 "$tmp/ebusy.txt" > "$tmp/trunc-replace.txt"
head -1 "$tmp/ebusy.txt" > "$tmp/trunc-a.txt"
printf 'A commit=EINVAL fb=1\nB commit=ok fb=2\nVERDICT replace_pending: unknown (A was refused errno=EINVAL)\n' > "$tmp/unknown.txt"
printf 'hello\nworld\n' > "$tmp/junk.txt"

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

check flips-counts 0 "PENDING commits=2 flips=2 latched=2 missed=0" "$tmp/flips.txt"
check flips-expect-ok 0 "EXPECT ok" "$tmp/flips.txt" --expect-flips 2
check flips-expect-wrong-n 1 "EXPECT fail" "$tmp/flips.txt" --expect-flips 5
check missed-counts 0 "PENDING commits=2 flips=1 latched=1 missed=1" "$tmp/missed.txt"
check missed-expect 1 "EXPECT fail" "$tmp/missed.txt" --expect-flips 2
check trunc-flips 1 "PENDING truncated=yes" "$tmp/trunc-flips.txt"
check trunc-flips-counts 1 "PENDING commits=1 flips=1 latched=1 missed=0" "$tmp/trunc-flips.txt"
check trunc-flips-expect 1 "EXPECT fail" "$tmp/trunc-flips.txt" --expect-flips 2
check ebusy 0 "PENDING replace result=ebusy" "$tmp/ebusy.txt"
check ebusy-expect 0 "EXPECT ok" "$tmp/ebusy.txt" --expect-replace ebusy
check ebusy-vs-accepted 1 "EXPECT fail" "$tmp/ebusy.txt" --expect-replace accepted
check accepted 0 "PENDING replace result=accepted" "$tmp/accepted.txt"
check accepted-expect 0 "EXPECT ok" "$tmp/accepted.txt" --expect-replace accepted
check other-errno 0 "PENDING replace result=EINVAL" "$tmp/einval.txt"
check other-errno-expect 1 "EXPECT fail" "$tmp/einval.txt" --expect-replace ebusy
check unknown 0 "PENDING replace result=unknown" "$tmp/unknown.txt"
check trunc-replace 1 "PENDING truncated=yes" "$tmp/trunc-replace.txt"
check trunc-replace-expect 1 "EXPECT fail" "$tmp/trunc-replace.txt" --expect-replace ebusy
check trunc-a-only 1 "no replace data" "$tmp/trunc-a.txt" --expect-replace ebusy
check wrong-mode 1 "no replace data" "$tmp/flips.txt" --expect-replace ebusy
check stdin 0 "PENDING replace result=ebusy" - < "$tmp/ebusy.txt"
check junk 2 "no pending_probe lines" "$tmp/junk.txt"
check missing-file 2 "cannot read" "$tmp/none"
check bad-expect 2 "accepted or ebusy" "$tmp/ebusy.txt" --expect-replace maybe
check bad-n 2 "bad arguments" "$tmp/flips.txt" --expect-flips x

exit $fail
