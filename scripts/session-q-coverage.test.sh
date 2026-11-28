#!/bin/bash
# Test for session-q-coverage.sh using fixture docs.
here=$(cd "$(dirname "$0")" && pwd)
S=$here/session-q-coverage.sh
T=$(mktemp -d "${TMPDIR:-/tmp}/sqc.XXXXXX") || exit 2
trap 'rm -rf "$T"' EXIT
fail=0
ck() { if eval "$2"; then echo "ok: $1"; else echo "FAIL: $1"; fail=1; fi; }

cat > "$T/a.md" <<'D'
# Session

## Q1 formats
Result (observed 2026-11-28): ok.

## Q2 kernel
Version string read from source.

## Q3 class
Probably linear first, inferred.

## Q4 sweep
No evidence here.

## Q5 date
observed but undated.

## Rows

| Q6 | zpos | observed 2026-11-28 |
| Q7 | zpos | nothing |

### Q8 parent
#### detail
read from source here
D
out=$(sh "$S" "$T/a.md" Q1 Q2 Q3 Q4 Q5 Q6 Q7 Q8 Q9 Q10 2>&1); rc=$?
ck "nonzero on missing" "[ $rc -ne 0 ]"
ck "Q1 observed" "echo \"\$out\" | grep -qx 'COVERED Q1 observed'"
ck "Q2 source" "echo \"\$out\" | grep -qx 'COVERED Q2 read-from-source'"
ck "Q3 inferred" "echo \"\$out\" | grep -qx 'COVERED Q3 inferred'"
ck "Q4 missing" "echo \"\$out\" | grep -q '^MISSING Q4'"
ck "Q5 undated observed missing" "echo \"\$out\" | grep -q '^MISSING Q5'"
ck "Q6 row covered" "echo \"\$out\" | grep -qx 'COVERED Q6 observed'"
ck "Q7 row scoped" "echo \"\$out\" | grep -q '^MISSING Q7'"
ck "Q8 subsection covered" "echo \"\$out\" | grep -qx 'COVERED Q8 read-from-source'"
ck "Q9 absent" "echo \"\$out\" | grep -qx 'MISSING Q9'"
ck "Q10 not matched by Q1" "echo \"\$out\" | grep -qx 'MISSING Q10'"

sh "$S" "$T/a.md" Q1 Q2 Q3 Q6 >/dev/null; ck "exit 0 when all covered" "[ $? -eq 0 ]"
sh "$S" "$T/nope.md" Q1 >/dev/null 2>&1; ck "exit 2 missing file" "[ $? -eq 2 ]"
sh "$S" "$T/a.md" >/dev/null 2>&1; ck "exit 2 no questions" "[ $? -eq 2 ]"
sh "$S" "$T/a.md" X1 >/dev/null 2>&1; ck "exit 2 bad id" "[ $? -eq 2 ]"
exit $fail
