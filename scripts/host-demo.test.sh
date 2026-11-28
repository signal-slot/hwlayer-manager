#!/bin/bash
# Tests for host-demo.sh with stub cargo, rustup, rustc and git; nothing is
# really built, so the whole test runs in well under a second. A real full
# run needs cargo and the thumbv7em target and takes minutes: not done here.
set -u
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/hostdemo.XXXXXX") || exit 1
trap 'rm -rf "$tmp"' EXIT
fails=0

check() { # name rc (0 = pass)
    if [ "$2" -eq 0 ]; then echo "pass $1"; else echo "FAIL $1"; fails=$((fails + 1)); fi
}

# The script finds its root from its own location: run a copy in a fake repo.
root=$tmp/repo
mkdir -p "$root/scripts"
cp "$here/host-demo.sh" "$root/scripts/"
script=$root/scripts/host-demo.sh

# A PATH holding only the basic tools the script uses, plus stubs.
sys=$tmp/sys
mkdir -p "$sys"
for t in sh mktemp rm date head wc grep sed dirname; do
    p=$(command -v "$t") && ln -s "$p" "$sys/$t"
done

stubs=$tmp/stubs
mkdir -p "$stubs"
cat > "$stubs/rustup" <<'EOS'
#!/bin/sh
[ -n "${FAKE_NO_TARGET:-}" ] && exit 0
echo thumbv7em-none-eabihf
EOS
cat > "$stubs/rustc" <<'EOS'
#!/bin/sh
echo "rustc 0.0.0 (stub)"
EOS
cat > "$stubs/git" <<'EOS'
#!/bin/sh
case "$1" in
rev-parse) echo abc123def456 ;;
diff) exit 0 ;;
esac
EOS
cat > "$stubs/cargo" <<'EOS'
#!/bin/sh
echo "cargo $*" >> "$FAKE_LOG"
if [ -n "${FAKE_CARGO_FAIL:-}" ]; then
    case "$*" in *"$FAKE_CARGO_FAIL"*) exit 1 ;; esac
fi
case "$*" in *plan_time*) for i in 1 2 3 4 5 6 7 8 9 10; do echo "line $i"; done ;; esac
exit 0
EOS
chmod +x "$stubs"/*
export FAKE_LOG=$tmp/log

runit() { # args...; output in $tmp/out, status in $rc
    : > "$FAKE_LOG"
    PATH=$stubs:$sys "$@" > "$tmp/out" 2>&1
    rc=$?
}

# -h / --help: usage on stdout, exit 0, nothing run
for f in -h --help; do
    runit sh "$script" "$f"
    [ $rc -eq 0 ] && grep -q 'Usage: sh scripts/host-demo.sh' "$tmp/out" && [ ! -s "$FAKE_LOG" ]
    check "$f prints usage, runs nothing" $?
done

# missing target: fail, say how to install, run no cargo
FAKE_NO_TARGET=1 runit sh "$script"
[ $rc -eq 1 ] && grep -q 'rustup target add thumbv7em-none-eabihf' "$tmp/out" \
    && grep -q 'nothing was run' "$tmp/out" && [ ! -s "$FAKE_LOG" ]
check "missing target fails with install hint" $?

# full run with stubs
runit sh "$script"
[ $rc -eq 0 ]; check "run exits 0" $?
[ "$(grep -c '^PASS ' "$tmp/out")" -eq 7 ]; check "7 PASS lines" $?
grep -q '^summary: 7 steps passed, 0 failed' "$tmp/out"; check "summary line" $?
grep -q '^rev: *abc123def456$' "$tmp/out"; check "revision printed" $?
grep -q '^date: *20[0-9][0-9]-[0-9][0-9]-[0-9][0-9]' "$tmp/out"; check "date printed" $?
grep -q '^rustc: rustc 0.0.0' "$tmp/out"; check "rustc version printed" $?
grep -q 'cargo build --locked --workspace' "$FAKE_LOG"; check "workspace build invoked" $?
for t in advise_scenes engine event_order; do
    grep -q "cargo test --locked -p composition-fake --test $t\$" "$FAKE_LOG"
    check "scene test $t invoked" $?
done
grep -q 'plan_time 50$' "$FAKE_LOG"; check "plan_time example invoked with 50" $?
[ "$(grep -c -- '--target thumbv7em-none-eabihf' "$FAKE_LOG")" -eq 2 ]; check "two thumbv7em builds" $?
grep -q '(10 lines in all)' "$tmp/out"; check "plan_time output summarised" $?

# the not-a-hardware-result markings
grep -q '^host demo: HOST RUN, NOT A HARDWARE RESULT$' "$tmp/out"; check "header says not a hardware result" $?
grep -q 'This was a host run. It says nothing about any board or display.' "$tmp/out"; check "footer says host run, nothing about board" $?
grep -q 'host timing, not hardware' "$tmp/out"; check "plan_time marked not hardware" $?

# a failing step stops the run with non-zero status and no summary
FAKE_CARGO_FAIL="--test event_order" runit sh "$script"
[ $rc -ne 0 ] && ! grep -q '^summary:' "$tmp/out" && grep -q '^PASS  scene tests: engine' "$tmp/out" \
    && ! grep -q '^PASS  scene tests: event_order' "$tmp/out"
check "failing step stops the run" $?

if [ $fails -eq 0 ]; then echo "all passed"; else echo "$fails FAILED"; fi
[ $fails -eq 0 ]
