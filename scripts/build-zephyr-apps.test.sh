#!/bin/bash
# Tests for build-zephyr-apps.sh with fake apps and a fake west; no Zephyr needed.
set -u
here=$(cd "$(dirname "$0")" && pwd)
script=$here/build-zephyr-apps.sh
tmp=$(mktemp -d "${TMPDIR:-/tmp}/zbuild.XXXXXX") || exit 1
trap 'rm -rf "$tmp"' EXIT
fails=0

check() { # name rc (0 = pass)
    if [ "$2" -eq 0 ]; then echo "pass $1"; else echo "FAIL $1"; fails=$((fails + 1)); fi
}

mkapp() { # dir readme-line
    mkdir -p "$1"
    : > "$1/CMakeLists.txt"
    : > "$1/prj.conf"
    printf '%s\n' "$2" > "$1/README.md"
}

root=$tmp/repo
mkapp "$root/tests/board/a" 'west build -p -b board_a/m7 tests/board/a'
mkapp "$root/tests/zephyr/b" 'west build -b board_b .'
mkdir -p "$root/tests/board/notapp"
echo 'west build -b nope' > "$root/tests/board/notapp/README.md"

# fake west: records its working directory and arguments; FAIL_ON makes a
# build whose arguments contain that word fail
mkdir -p "$tmp/bin"
cat > "$tmp/bin/west" <<'EOS'
#!/bin/sh
echo "$PWD $*" >> "$FAKE_LOG"
case "$*" in
*board_a*) [ "${FAIL_A:-0}" = 1 ] && exit 1 ;;
esac
exit 0
EOS
chmod +x "$tmp/bin/west"
ws=$tmp/ws
mkdir -p "$ws"
export FAKE_LOG=$tmp/log

# dry run: prints both commands, board parsed, runs nothing, needs no env
out=$(env -u ZEPHYR_WORKSPACE -u ZEPHYR_SDK_INSTALL_DIR PATH="$tmp/bin:$PATH" \
    sh "$script" --dry-run --build-root /bb "$root")
check dry-rc $?
echo "$out" | grep -qx "west build -p always -b board_a/m7 -d /bb/board_a $root/tests/board/a"
check dry-board-a $?
echo "$out" | grep -qx "west build -p always -b board_b -d /bb/zephyr_b $root/tests/zephyr/b"
check dry-board-b $?
! echo "$out" | grep -q notapp
check dry-skips-nonapp $?
[ ! -e "$FAKE_LOG" ]
check dry-runs-nothing $?

# missing environment -> 2
out=$(env -u ZEPHYR_WORKSPACE ZEPHYR_SDK_INSTALL_DIR=$ws PATH="$tmp/bin:$PATH" sh "$script" "$root" 2>&1)
rc=$?
[ "$rc" -eq 2 ] && echo "$out" | grep -q 'ZEPHYR_WORKSPACE is not set'
check missing-workspace $?
out=$(env -u ZEPHYR_SDK_INSTALL_DIR ZEPHYR_WORKSPACE=$ws PATH="$tmp/bin:$PATH" sh "$script" "$root" 2>&1)
rc=$?
[ "$rc" -eq 2 ] && echo "$out" | grep -q 'ZEPHYR_SDK_INSTALL_DIR is not set'
check missing-sdk $?
# no west: an empty bin directory with only the tools the script needs
mkdir -p "$tmp/nowest"
for t in sh sed tr dirname printf; do
    p=$(command -v "$t") && ln -sf "$p" "$tmp/nowest/$t"
done
out=$(ZEPHYR_WORKSPACE=$ws ZEPHYR_SDK_INSTALL_DIR=$ws PATH="$tmp/nowest" "$tmp/nowest/sh" "$script" "$root" 2>&1)
rc=$?
[ "$rc" -eq 2 ] && echo "$out" | grep -q 'west not found'
check missing-west $?

run() { # extra args...
    rm -f "$FAKE_LOG"
    ZEPHYR_WORKSPACE=$ws ZEPHYR_SDK_INSTALL_DIR=$ws PATH="$tmp/bin:$PATH" \
        sh "$script" --build-root "$tmp/out" "$@" "$root" 2>&1
}

# all build: two west calls, each run inside the workspace
out=$(run)
rc=$?
[ "$rc" -eq 0 ] && [ "$(wc -l < "$FAKE_LOG")" -eq 2 ] &&
    [ "$(grep -c "^$ws build -p always -b " "$FAKE_LOG")" -eq 2 ]
check build-all $?

# failure stops without --keep-going, continues with it
out=$(FAIL_A=1 run)
rc=$?
[ "$rc" -eq 1 ] && [ "$(wc -l < "$FAKE_LOG")" -eq 1 ]
check fail-stops $?
out=$(FAIL_A=1 run --keep-going)
rc=$?
[ "$rc" -eq 1 ] && [ "$(wc -l < "$FAKE_LOG")" -eq 2 ] && echo "$out" | grep -q 'built 1, failed 1'
check keep-going $?

# an app without a board line fails
mkapp "$root/tests/board/c" 'no build line here'
out=$(run --keep-going)
rc=$?
[ "$rc" -eq 1 ] && echo "$out" | grep -q 'fail board_c: no '
check no-board-line $?

# default build root is never /tmp
out=$(TMPDIR=/tmp sh "$script" --dry-run "$root" 2>&1)
! echo "$out" | grep -q -- '-d /tmp/'
check default-root-not-tmp $?

if command -v shellcheck > /dev/null 2>&1; then
    shellcheck "$script" "$0" || fails=$((fails + 1))
fi
if [ "$fails" -eq 0 ]; then
    echo "all passed"
    exit 0
fi
exit 1
