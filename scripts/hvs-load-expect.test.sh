#!/bin/sh
# Self-test for hvs-load-expect.py. Run: bash scripts/hvs-load-expect.test.sh
#
# All fixtures are synthetic text following the STEP / SUMMARY / VERDICT line
# formats of crates/composition-drm/examples/hvs_load_probe/{main.rs,debugfs.rs}.
# No board data.

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/hvs-load-expect.py
tmp=$(mktemp -d "${TMPDIR:-.}/hle-test.XXXXXX") || exit 2
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
fail=0

# check NAME WANT_RC PATTERN ARGS...
check() {
    name=$1 want=$2 pat=$3
    shift 3
    got=$(python3 -I "$tool" "$@" 2>&1)
    rc=$?
    if [ "$rc" != "$want" ]; then
        echo "FAIL $name: exit $rc, want $want"; printf '%s\n' "$got"; fail=1
    elif [ -n "$pat" ] && ! printf '%s\n' "$got" | grep -Fxq -- "$pat"; then
        echo "FAIL $name: output lacks line '$pat'"; printf '%s\n' "$got"; fail=1
    else
        echo "ok   $name"
    fi
}

head='DATE 2026-11-28T10:00:00Z
KERNEL 6.6.0
DEVICE /dev/dri/card0
MODE 720x1280@60Hz period_us=16666 connector 32 crtc 90
DEBUGFS dir=/sys/kernel/debug/dri/0 underrun=0 dlist_words=9 dlist=0:9,1:off,2:off'

# drop.txt: step 1 clean, step 2 rejected by TEST_ONLY (never a drop, even
# with a moving counter), step 3 underrun +2 and one late flip, step 4 a
# refused real commit.
cat > "$tmp/drop.txt" <<X
$head
STEP n=1 layers=1 size=360x640 src=360x640 scale=1.0000 test_only=ok commit=ok latched=120/120 late=0 missing=0 underrun_before=0 underrun_after=0 dlist_words=9 dlist=0:9,1:off,2:off
STEP n=2 layers=1 size=360x640 src=180x320 scale=2.0000 test_only=err:22 commit=not-run latched=0/0 late=0 missing=0 underrun_before=0 underrun_after=5 dlist_words=9 dlist=0:9,1:off,2:off
STEP n=3 layers=2 size=720x1280 src=1440x2560 scale=0.5000 test_only=ok commit=ok latched=120/120 late=1 missing=0 underrun_before=5 underrun_after=7 dlist_words=21 dlist=0:21,1:off,2:off
STEP n=4 layers=3 size=720x1280 src=1440x2560 scale=0.5000 test_only=ok commit=err:22 latched=0/120 late=0 missing=120 underrun_before=7 underrun_after=7 dlist_words=21 dlist=0:21,1:off,2:off
RESTORE ok saved state written back
SUMMARY steps=4 test_only_ok=3 test_only_err=1 skipped=0 drop_steps=2 first_drop=3 first_reject=2 underrun_delta=7
VERDICT hvs_load_probe: DONE
X
check drop-found 0 'FOUND first_drop step=3 layers=2 size=720x1280 src=1440x2560 scale=0.5000 reasons=underrun:+2,late:1' "$tmp/drop.txt"
check drop-steps 0 'STEPS total=4 test_only_ok=3 test_only_err=1 skipped=0 drop_steps=2' "$tmp/drop.txt"
check drop-reject 0 'FIRST_REJECT step=2 layers=1 size=360x640 src=180x320 scale=2.0000 errno=22' "$tmp/drop.txt"
check drop-commit 0 'DROP step=4 layers=3 size=720x1280 src=1440x2560 scale=0.5000 reasons=missing:120,commit:err:22' "$tmp/drop.txt"
check drop-dlist 0 'DLIST max_words=21' "$tmp/drop.txt"
check drop-stdin 0 'FOUND first_drop step=3 layers=2 size=720x1280 src=1440x2560 scale=0.5000 reasons=underrun:+2,late:1' - < "$tmp/drop.txt"

# clean.txt: no drop, a skipped step (buffer refused), underrun counter steady.
cat > "$tmp/clean.txt" <<X
$head
STEP n=1 layers=1 size=360x640 src=360x640 scale=1.0000 test_only=ok commit=ok latched=120/120 late=0 missing=0 underrun_before=0 underrun_after=0 dlist_words=9 dlist=0:9,1:off,2:off
STEP n=2 layers=1 size=720x1280 src=1440x2560 scale=0.5000 test_only=skipped:alloc:12 commit=not-run latched=0/0 late=0 missing=0 underrun_before=0 underrun_after=0 dlist_words=9 dlist=0:9,1:off,2:off
SUMMARY steps=2 test_only_ok=1 test_only_err=0 skipped=1 drop_steps=0 first_drop=none first_reject=none underrun_delta=0
VERDICT hvs_load_probe: DONE
X
check clean-not-found 0 'NOT_FOUND' "$tmp/clean.txt"
check clean-steps 0 'STEPS total=2 test_only_ok=1 test_only_err=0 skipped=1 drop_steps=0' "$tmp/clean.txt"
check clean-no-reject 0 'FIRST_REJECT none' "$tmp/clean.txt"

# na.txt: debugfs unavailable (vkms, no root): underrun cannot drop, missing can.
cat > "$tmp/na.txt" <<'X'
DEBUGFS unavailable reason=/sys/kernel/debug/dri/0/hvs_underrun: Permission denied
STEP n=1 layers=1 size=512x384 src=512x384 scale=1.0000 test_only=ok commit=ok latched=120/120 late=0 missing=0 underrun_before=na underrun_after=na dlist_words=na dlist=na
STEP n=2 layers=2 size=512x384 src=512x384 scale=1.0000 test_only=ok commit=ok latched=119/120 late=0 missing=1 underrun_before=na underrun_after=na dlist_words=na dlist=na
SUMMARY steps=2 test_only_ok=2 test_only_err=0 skipped=0 drop_steps=1 first_drop=2 first_reject=none underrun_delta=na
VERDICT hvs_load_probe: DONE
X
check na-underrun 0 'UNDERRUN unavailable (no step has both counts; drops rest on flips only)' "$tmp/na.txt"
check na-found 0 'FOUND first_drop step=2 layers=2 size=512x384 src=512x384 scale=1.0000 reasons=missing:1' "$tmp/na.txt"

# A SUMMARY that disagrees with the judged steps gives a NOTE.
sed 's/first_drop=3/first_drop=4/' "$tmp/drop.txt" > "$tmp/note.txt"
check summary-note 0 'NOTE probe SUMMARY first_drop=4, judged 3' "$tmp/note.txt"

# Two files are read as one log.
grep -v VERDICT "$tmp/clean.txt" > "$tmp/part1.txt"
printf 'VERDICT hvs_load_probe: DONE\n' > "$tmp/part2.txt"
check two-files 0 'NOT_FOUND' "$tmp/part1.txt" "$tmp/part2.txt"

# Input errors.
check incomplete 2 '' "$tmp/part1.txt"
printf '%s\nVERDICT hvs_load_probe: DONE\n' "$head" > "$tmp/nostep.txt"
check no-step 2 'ERROR no STEP line' "$tmp/nostep.txt"
sed 's/late=1/late=x/' "$tmp/drop.txt" > "$tmp/bad1.txt"
check bad-late 2 'ERROR line 8: bad late=x' "$tmp/bad1.txt"
sed 's/latched=120\/120 late=0 missing=0 underrun_before=0 underrun_after=0 dlist_words=9 dlist=0:9,1:off,2:off/latched=120 late=0 missing=0 underrun_before=0 underrun_after=0 dlist_words=9 dlist=0:9,1:off,2:off/' "$tmp/clean.txt" > "$tmp/bad2.txt"
check bad-latched 2 '' "$tmp/bad2.txt"
sed 's/test_only=ok commit=ok latched=120\/120 late=1/test_only=maybe commit=ok latched=120\/120 late=1/' "$tmp/drop.txt" > "$tmp/bad3.txt"
check bad-test-only 2 'ERROR line 8: bad test_only=maybe' "$tmp/bad3.txt"
sed 's/ missing=0 underrun_before=5/ underrun_before=5/' "$tmp/drop.txt" > "$tmp/bad4.txt"
check missing-key 2 'ERROR line 8: STEP without missing=' "$tmp/bad4.txt"
sed 's/STEP n=4 /STEP n=3 /' "$tmp/drop.txt" > "$tmp/dup.txt"
check duplicate 2 'ERROR line 9: step 3 given twice' "$tmp/dup.txt"
check unreadable 2 '' "$tmp/does-not-exist.txt"
check bad-option 2 '' --no-such-option

[ "$fail" -eq 0 ] && echo "hvs-load-expect self-test: PASS" || echo "hvs-load-expect self-test: FAIL"
exit "$fail"
