#!/bin/sh
# Host test for check-zephyr-app-layout.sh using fixture trees.
set -u
here=$(cd "$(dirname "$0")" && pwd)
chk=$here/check-zephyr-app-layout.sh
fx=$here/testdata/check-zephyr-app-layout
fails=0

# expect NAME ROOT EXIT-CODE PATTERN...
expect() {
    name=$1
    dir=$2
    code=$3
    shift 3
    out=$(sh "$chk" "$dir" 2>&1)
    rc=$?
    if [ "$rc" -ne "$code" ]; then
        echo "FAIL $name: exit $rc, want $code"
        fails=$((fails + 1))
    fi
    for p in "$@"; do
        if ! printf '%s\n' "$out" | grep -q -- "$p"; then
            echo "FAIL $name: no '$p' in:"
            echo "$out"
            fails=$((fails + 1))
        fi
    done
}

expect good "$fx/good" 0 '^ok app1$' '^skip boards: not an app'
expect bad "$fx/bad" 1 '^fail noconf: missing prj.conf' '^fail nosrc: missing src/' \
    '^fail emptysrc: src/ is empty' '^fail noreadme: missing README.md' \
    '^fail nowest: README.md has no west build' '^fail showcase: refers to MCUI_SHOWCASE_DIR'
sh "$chk" /nonexistent/dir >/dev/null 2>&1
if [ $? -ne 2 ]; then
    echo "FAIL missing root: want exit 2"
    fails=$((fails + 1))
fi
[ "$fails" -eq 0 ] && echo "all passed"
[ "$fails" -eq 0 ]
