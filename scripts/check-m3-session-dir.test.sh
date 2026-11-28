#!/bin/sh
# Test of check-m3-session-dir.sh with synthetic session directories (no real data).
here=$(cd "$(dirname "$0")" && pwd)
t=$(mktemp -d "${TMPDIR:-/tmp}/m3sess.XXXXXX") || exit 1
trap 'rm -rf "$t"' EXIT
fail=0
mk() { # name
    s=$t/$1
    mkdir -p "$s/photos" "$s/verdicts"
    : > "$s/trace.txt"
    i=1
    while [ $i -le 8 ]; do
        echo "[trace] SCENE $i fake commits=1 latched=1" >> "$s/trace.txt"
        echo fake > "$s/photos/scene$i.jpg"
        echo "VERDICT: PASS" > "$s/verdicts/scene$i.txt"
        i=$((i + 1))
    done
    echo "[trace] RESULT PASS" >> "$s/trace.txt"
    echo "| fake | board |" > "$s/board-info.md"
    echo "fake=1" > "$s/state-before.txt"
}
expect() { # name status text
    out=$(sh "$here/check-m3-session-dir.sh" "$t/$1")
    st=$?
    if [ $st -ne "$2" ] || ! printf '%s\n' "$out" | grep -qF "$3"; then
        echo "FAIL $1: status $st, want $2 and '$3'"
        printf '%s\n' "$out"
        fail=1
    fi
}
mk ok
expect ok 0 COMPLETE
mk nophoto
rm "$t/nophoto/photos/scene5.jpg"
expect nophoto 1 "INCOMPLETE: 1 missing"
expect nophoto 1 "missing: photograph scene 5"
mk noresult
grep -v RESULT "$t/noresult/trace.txt" > "$t/x"
mv "$t/x" "$t/noresult/trace.txt"
expect noresult 1 "missing: trace RESULT line"
mk nosnap
rm "$t/nosnap/state-before.txt"
expect nosnap 1 "missing: before snapshot"
mk noverd
rm "$t/noverd/verdicts/scene2.txt"
expect noverd 1 "missing: verdict scene 2"
[ $fail -eq 0 ] && echo "check-m3-session-dir test: ok"
exit $fail
