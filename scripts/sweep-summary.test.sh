#!/usr/bin/env bash
# Self-test for sweep-summary.py. Fixtures are synthetic.
set -u
cd "$(dirname "$0")"
D=testdata/sweep-summary
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
out=$(python3 -I sweep-summary.py $D/*.txt) || bad "exit code"
has() { grep -qF -- "$1" <<<"$out" && ok "$2" || bad "$2"; }
has "| 42 | Overlay | XR24 | 4.0000 |" "scale row"
has "| 10 | 31,42,43,44,45,46,47,48,49,50 | none | EINVAL |" "combo row"
has "listed but refused" "listed-but-refused flag"
has "unlisted but accepted" "unlisted-but-accepted flag"
has "| EBUSY | 1 |" "replace row"
has "| 3 | 1 | 1 | 1 |" "agreement totals"
has "not reached" "spaced value"
l=$(grep -n "planner-looser" <<<"$out" | head -1 | cut -d: -f1)
s=$(grep -n "planner-stricter" <<<"$out" | head -1 | cut -d: -f1)
[ "$l" -lt "$s" ] && ok "looser first" || bad "looser first"
python3 -I sweep-summary.py --json $D/*.txt | python3 -I -c 'import json,sys; d=json.load(sys.stdin); assert len(d["scale"])==2 and d["checks"][0]["agree"]=="planner-looser"' && ok json || bad json
python3 -I sweep-summary.py < $D/check.txt | grep -q "Planner vs TEST_ONLY" && ok stdin || bad stdin
python3 -I sweep-summary.py /nonexistent >/dev/null 2>&1
[ $? -eq 2 ] && ok "exit 2" || bad "exit 2"
exit $fail
