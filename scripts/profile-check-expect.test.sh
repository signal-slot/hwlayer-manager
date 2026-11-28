#!/bin/sh
# Self-test for profile-check-expect.py.
# Run: bash scripts/profile-check-expect.test.sh
#
# Sample lines follow the CHECK formats asserted in the unit tests of
# crates/composition-drm/examples/profile_check.rs (check_and_agreement_lines);
# no board data.

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/profile-check-expect.py
fail=0
tmp=$(mktemp -d) || exit 1
trap 'rm -rf "$tmp"' EXIT

cat > "$tmp/agree.txt" <<'X'
CHECK layers-1 planner=plan:33 test_only=ok agree=yes via=plan candidates=1
CHECK refuse-1 planner=refuse:layer1:SCALING test_only=not reached agree=yes via=none candidates=0 note=a:b
X
cat > "$tmp/stricter.txt" <<'X'
CHECK layers-1 planner=plan:33 test_only=ok agree=yes via=plan candidates=1
CHECK scale-2 planner=refuse:layer1:SCALING test_only=ok agree=planner-stricter via=forced candidates=1 note=x
AGREEMENT total=2 agree=1 stricter=1 looser=0
X
cat > "$tmp/looser.txt" <<'X'
CHECK scale-8 planner=plan:33,45 test_only=ERANGE agree=planner-looser via=plan candidates=3
X
printf 'profile "Generic" driver x: 0 frames\n' > "$tmp/none.txt"

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

check agree 0 "VERDICT: PASS" "$tmp/agree.txt"
check agree-counts 0 "total=2 yes=2 no=0" "$tmp/agree.txt"
check stricter 1 "DISAGREE scale-2 agree=planner-stricter" "$tmp/stricter.txt"
check stricter-fail 1 "VERDICT: FAIL" "$tmp/stricter.txt"
check allow-stricter 0 "VERDICT: PASS" --allow planner-stricter "$tmp/stricter.txt"
check allow-count 0 "VERDICT: PASS" --allow-disagree 1 "$tmp/stricter.txt"
check looser-not-allowed 1 "planner-looser=1" --allow planner-stricter "$tmp/looser.txt"
check looser-detail 1 "test_only=ERANGE" "$tmp/looser.txt"
check no-lines 1 "no CHECK lines" "$tmp/none.txt"
check markdown 0 "| planner-stricter | 0 |" --markdown "$tmp/agree.txt"
check markdown-bad 1 "| scale-2 | planner-stricter |" --markdown "$tmp/stricter.txt"
check stdin 0 "VERDICT: PASS" - < "$tmp/agree.txt"
check bad-allow 2 "bad --allow" --allow bogus "$tmp/agree.txt"
check missing-file 2 "No such file" "$tmp/absent.txt"
check bad-option 2 "" --bogus

[ "$fail" = 0 ] && echo "all passed"
exit "$fail"
