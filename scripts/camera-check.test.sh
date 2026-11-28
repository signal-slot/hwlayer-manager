#!/bin/bash
# Host test for camera-check.py: pass, fail, compare, argument and input errors.
# Run: bash scripts/camera-check.test.sh
# Pictures are generated into a mktemp directory by testdata/camera-check/gen.py
# (standard library only). Skips with exit 0 if python3 is missing.

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/camera-check.py
fx=$here/testdata/camera-check
if ! command -v python3 >/dev/null 2>&1; then
    echo "SKIP: python3 not found"; exit 0
fi
root=$(mktemp -d "${TMPDIR:-/tmp}/cc-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
cam() { python3 -I "$tool" "$@" >"$root/out" 2>"$root/err"; echo $?; }
# expect NAME WANT_CODE ARGS...
expect() {
    local name=$1 want=$2 got; shift 2
    got=$(cam "$@")
    if [ "$got" = "$want" ]; then ok "$name (exit $want)"; else bad "$name: exit $got, want $want"; cat "$root/out" "$root/err"; fi
}
gen() { python3 -I "$fx/gen.py" "$@" || { echo "FAIL: gen $*"; exit 2; }; }

gen "$root/btop.ppm" b-top
gen "$root/atop.ppm" a-top
gen "$root/moved.ppm" b-moved
gen "$root/btop.png" b-top
z=$fx/zorder-b-top.json

if python3 -I "$tool" --self-test >"$root/st" 2>&1 && tail -1 "$root/st" | grep -qx PASS; then
    ok "self-test"
else
    bad "self-test"; cat "$root/st"
fi

expect "pass: B on top (PPM)" 0 "$root/btop.ppm" "$z"
grep -q 'VERDICT: PASS' "$root/out" && ok "pass verdict line" || bad "pass verdict line"
expect "pass: B on top (PNG)" 0 "$root/btop.png" "$z"
expect "fail: swapped z-order" 1 "$root/atop.ppm" "$z"
if grep -q 'VERDICT: FAIL' "$root/out" && grep -q '^FAIL  *overlap' "$root/out"; then
    ok "fail verdict names overlap"
else
    bad "fail verdict names overlap"
fi
expect "--panel full override" 0 "$root/btop.ppm" "$z" --panel full
expect "wrong --rotation fails" 1 "$root/btop.ppm" "$z" --rotation 180
expect "--json pass" 0 "$root/btop.ppm" "$z" --json
grep -q '"pass": true' "$root/out" && ok "json pass true" || bad "json pass true"

expect "compare: moved plane passes" 0 "$root/btop.ppm" "$fx/move.json" --compare "$root/moved.ppm"
expect "compare: identical pictures fail" 1 "$root/btop.ppm" "$fx/move.json" --compare "$root/btop.ppm"

# argument and input errors: exit 2 with a message on stderr
expect "no arguments" 2
expect "one positional" 2 "$root/btop.ppm"
expect "unknown option" 2 "$root/btop.ppm" "$z" --bogus
[ -s "$root/err" ] && ok "errors go to stderr" || bad "errors go to stderr"
expect "--compare without value" 2 "$root/btop.ppm" "$z" --compare
expect "--rotation not a number" 2 "$root/btop.ppm" "$z" --rotation x
expect "--panel bad value" 2 "$root/btop.ppm" "$z" --panel oval
expect "missing picture" 2 "$root/none.ppm" "$z"
expect "missing expectation" 2 "$root/btop.ppm" "$root/none.json"
expect "invalid JSON" 2 "$root/btop.ppm" "$fx/bad.json"
expect "nothing to check" 2 "$root/btop.ppm" "$fx/no-checks.json"
expect "compare in file, no second picture" 2 "$root/btop.ppm" "$fx/move.json"
expect "invalid rotation value" 2 "$root/btop.ppm" "$z" --rotation 45
expect "--help" 0 --help
printf 'GIF89a' > "$root/x.gif"
if ! python3 -I -c 'import PIL' 2>/dev/null; then
    expect "unsupported format without Pillow" 2 "$root/x.gif" "$z"
fi

[ $fail -eq 0 ] && echo "all ok"
exit $fail
