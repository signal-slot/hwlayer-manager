#!/bin/bash
# Tests for check-zephyr-apps.sh using fixture apps in a temporary directory.
set -u
here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-zephyr-apps.sh
tmp=$(mktemp -d "${TMPDIR:-/tmp}/zapps.XXXXXX") || exit 1
trap 'rm -rf "$tmp"' EXIT
fails=0

expect() { # name want_rc pattern root
    local out rc
    out=$(sh "$script" "$4"); rc=$?
    if [ "$rc" -ne "$2" ] || ! printf '%s\n' "$out" | grep -qE -- "$3"; then
        echo "FAIL $1 (rc=$rc): $out"
        fails=$((fails + 1))
    else
        echo "pass $1"
    fi
}

mkapp() { # dir
    mkdir -p "$1/src"
    : > "$1/CMakeLists.txt"
    : > "$1/prj.conf"
    echo 'int main(void) { return 0; }' > "$1/src/main.c"
    echo 'west build -b board .' > "$1/README.md"
}

# no apps at all
mkdir -p "$tmp/empty"
expect empty 0 '^$' "$tmp/empty"
mkdir -p "$tmp/empty2/tests/board"
expect empty-parent 0 '^$' "$tmp/empty2"

# passing apps in both locations
r=$tmp/good
mkapp "$r/tests/board/a"
mkapp "$r/tests/zephyr/b"
expect good-board 0 'ok .*/tests/board/a$' "$r"
expect good-zephyr 0 'ok .*/tests/zephyr/b$' "$r"

# failing variants: mutate a good app, expect a matching fail line
variant() { # name pattern command...
    local name=$1 pat=$2
    shift 2
    local d=$tmp/$name
    mkapp "$d/tests/board/x"
    (cd "$d/tests/board/x" && "$@")
    expect "$name" 1 "fail .*x: .*$pat" "$d"
}
variant no-cmake 'CMakeLists.txt' rm CMakeLists.txt
variant no-prjconf 'prj.conf' rm prj.conf
variant no-readme 'README.md' rm README.md
variant no-src 'src/' rm -r src
variant empty-src 'no source file' rm src/main.c
variant no-west 'west build' sh -c 'echo hi > README.md'
variant ipv4 'IPv4' sh -c 'echo "host 192.168.1.5" >> src/main.c'
variant tty '/dev/tty' sh -c 'echo "/dev/ttyUSB0" >> prj.conf'
variant tty-with-rpmsg '/dev/tty' sh -c 'echo "/dev/ttyRPMSG0 /dev/ttyUSB0" >> prj.conf'
variant password 'password' sh -c 'echo "Password=x" >> README.md'

# mixed: one good, one bad -> non-zero, both reported
d=$tmp/mixed
mkapp "$d/tests/board/good"
mkapp "$d/tests/zephyr/bad"
rm "$d/tests/zephyr/bad/prj.conf"
out=$(sh "$script" "$d")
rc=$?
if [ "$rc" -eq 1 ] && echo "$out" | grep -q '^ok .*good$' && echo "$out" | grep -q '^fail .*bad:'; then
    echo "pass mixed"
else
    echo "FAIL mixed: $out"
    fails=$((fails + 1))
fi

# version-like numbers are not IPv4
d=$tmp/ver
mkapp "$d/tests/board/v"
echo 'version 1.2.3' >> "$d/tests/board/v/prj.conf"
expect version-not-ip 0 '^ok ' "$d"

# the rpmsg_tty node is not a UART path
d=$tmp/rpmsg
mkapp "$d/tests/board/r"
echo 'open /dev/ttyRPMSG0 and /dev/ttyRPMSG1' >> "$d/tests/board/r/README.md"
expect rpmsg-allowed 0 '^ok ' "$d"

if command -v shellcheck > /dev/null 2>&1; then
    shellcheck "$script" "$0" || fails=$((fails + 1))
fi
if [ "$fails" -eq 0 ]; then
    echo "all passed"
    exit 0
fi
exit 1
