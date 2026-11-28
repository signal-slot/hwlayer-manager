#!/bin/sh
# Host test for scripts/imx95-m7-verdict.py: synthetic fixtures, no board.
here=$(cd "$(dirname "$0")" && pwd)
py="$here/imx95-m7-verdict.py"
fx="$here/testdata/imx95-m7-verdict"
fail=0
# t NAME EXIT PATTERN ARGS...
t() {
  name=$1 want=$2 pat=$3; shift 3
  out=$(python3 -I "$py" "$@" 2>/dev/null); rc=$?
  [ "$rc" -eq "$want" ] || { echo "FAIL: $name: exit $rc, want $want" >&2; fail=1; }
  [ "$want" -eq 64 ] || echo "$out" | grep -q "^M7-VERDICT: $pat" ||
    { echo "FAIL: $name: output '$out'" >&2; fail=1; }
}
t a-pass 0 "pass A readable" --candidate A --expect "$fx/a.expect" "$fx/a-pass.log"
t a-missing 2 "incomplete A missing=reg:DPU_CTRL" --candidate A --expect "$fx/a.expect" "$fx/a-missing.log"
t a-mismatch 1 "fail A mismatch=reg:DPU_ID:got=0x00000002" --candidate A --expect "$fx/a.expect" "$fx/a-mismatch.log"
t a-blocked 1 "fail A mismatch=reg:DPU_ID:got=FAULT" --candidate A --expect "$fx/a.expect" "$fx/a-blocked.log"
t a-blocked-noexp 0 "pass A blocked" --candidate A "$fx/a-blocked.log"
t a-hung 2 "incomplete A missing=summary:verdict-A" --candidate A "$fx/a-hung.log"
t a-empty 2 "incomplete A missing=all" --candidate A --expect "$fx/a.expect" "$fx/empty.log"
t b-pass 0 "pass B plan-equal=yes" --candidate B --expect "$fx/b.expect" "$fx/b-pass.log"
t b-mismatch 1 "fail B mismatch=" --candidate B --expect "$fx/b.expect" "$fx/b-mismatch.log"
t b-unchecked 2 "incomplete B missing=plan-unchecked" --candidate B "$fx/b-pass.log"
t b-empty 2 "incomplete B missing=all" --candidate B --expect "$fx/b.expect" "$fx/empty.log"
t stdin 0 "pass A" --candidate A --expect "$fx/a.expect" < "$fx/a-pass.log"
t usage 64 "" --bogus
t nocand 64 "" "$fx/a-pass.log"
t nofile 64 "" --candidate A "$fx/does-not-exist"
[ $fail -eq 0 ] && echo "imx95-m7-verdict.test.sh: ok"
exit $fail
