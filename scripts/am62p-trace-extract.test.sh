#!/bin/sh
# Self-test for am62p-trace-extract.py. Run: sh scripts/am62p-trace-extract.test.sh
here=$(cd "$(dirname "$0")" && pwd)
tool=$here/am62p-trace-extract.py
data=$here/testdata/am62p-trace
scenes=$here/am62p-scenes.py
root=$(mktemp -d "${TMPDIR:-.}/am62p-trace-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
py() { python3 -I "$@"; }

for f in clean wrapped nul two-runs; do
    py "$tool" "$data/$f.txt" > "$root/$f.out" 2> "$root/$f.err"; rc=$?
    [ $rc -eq 0 ] && ok "$f exits 0" || bad "$f exit $rc"
    [ "$(grep -c '^SCENE ' "$root/$f.out")" -eq 8 ] && ok "$f has 8 SCENE lines" || bad "$f scene count"
    [ "$(grep -c '^BEGIN ' "$root/$f.out")" -eq 1 ] && ok "$f has one banner" || bad "$f banner count"
    [ "$(tail -n 1 "$root/$f.out")" = "RESULT PASS (8 scenes, vsyncs 480)" ] \
        && ok "$f ends with the last run's RESULT" || bad "$f result"
    if grep -q 'junk\|board test:' "$root/$f.out"; then bad "$f kept junk or pre-banner lines"; else ok "$f drops pre-banner and junk"; fi
    py "$scenes" summary "$root/$f.out" > /dev/null 2>&1 && ok "$f accepted by am62p-scenes summary" || bad "$f rejected by summary"
done
cmp -s "$root/clean.out" "$root/wrapped.out" && ok "wrapped equals clean" || bad "wrapped differs from clean"
cmp -s "$root/clean.out" "$root/nul.out" && ok "nul equals clean" || bad "nul differs from clean"
cmp -s "$root/clean.out" "$root/two-runs.out" && ok "two-runs equals clean" || bad "two-runs differs from clean"

py "$tool" < "$data/clean.txt" | cmp -s - "$root/clean.out" && ok "stdin" || bad "stdin"
py "$tool" - < "$data/clean.txt" | cmp -s - "$root/clean.out" && ok "dash" || bad "dash"

py "$tool" "$data/incomplete.txt" > "$root/o" 2> "$root/e"; rc=$?
if [ $rc -eq 1 ] && grep -q 'no RESULT' "$root/e" && [ ! -s "$root/o" ]; then ok "no RESULT: exit 1 with reason"; else bad "no RESULT (rc $rc)"; fi
py "$tool" "$data/incomplete-scene.txt" > "$root/o" 2> "$root/e"; rc=$?
if [ $rc -eq 1 ] && grep -q 'missing SCENE 7' "$root/e"; then ok "missing scene: exit 1 with reason"; else bad "missing scene (rc $rc)"; fi
printf 'nothing here\n' | py "$tool" > /dev/null 2>&1; rc=$?
[ $rc -eq 1 ] && ok "no run: exit 1" || bad "no run (rc $rc)"

py "$tool" --bad > /dev/null 2>&1; [ $? -eq 2 ] && ok "bad option: exit 2" || bad "bad option"
py "$tool" a b > /dev/null 2>&1; [ $? -eq 2 ] && ok "two args: exit 2" || bad "two args"
py "$tool" "$root/missing" > /dev/null 2>&1; [ $? -eq 2 ] && ok "missing file: exit 2" || bad "missing file"

[ $fail -eq 0 ] && echo "all ok"
exit $fail
