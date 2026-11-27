#!/bin/sh
# Host-only reproducible demo of the composition engine (SoW WP5).
#
# Builds the workspace, runs the composition-fake scene tests and the
# plan_time example, runs the two thumbv7em-none-eabihf builds of docs/ci.md,
# and prints a summary with the date and git revision.
#
# This is a HOST run. It claims nothing about hardware; see docs/host-demo.md.
#
# Usage: sh scripts/host-demo.sh
# Nothing is installed; a missing thumbv7em target is reported with the
# rustup command to run. Writes only under the cargo target directory and
# a temporary directory that is removed on exit.
set -eu

case ${1-} in
    -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
esac

cd "$(dirname "$0")/.."

T=thumbv7em-none-eabihf
if ! rustup target list --installed 2>/dev/null | grep -qx "$T"; then
    echo "FAIL  target $T is not installed (nothing was run)"
    echo "      install it with: rustup target add $T"
    exit 1
fi

tmp=$(mktemp -d "${TMPDIR:-/tmp}/host-demo.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

rev=$(git rev-parse --short=12 HEAD 2>/dev/null || echo unknown)
if ! git diff --quiet HEAD 2>/dev/null; then
    rev="$rev (+uncommitted changes)"
fi

echo "host demo: HOST RUN, NOT A HARDWARE RESULT"
echo "date:  $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "rev:   $rev"
echo "rustc: $(rustc --version)"
echo

n=0
step() {
    name=$1
    shift
    echo "==> $name: $*"
    "$@"
    n=$((n + 1))
    echo "PASS  $name"
    echo
}

step "build workspace" cargo build --locked --workspace

# Scenes composed against the fake backend: the planner assigns layers to
# planes and the engine emits events. Tests and examples are not modified.
step "scene tests: advise_scenes" \
    cargo test --locked -p composition-fake --test advise_scenes
step "scene tests: engine" \
    cargo test --locked -p composition-fake --test engine
step "scene tests: event_order" \
    cargo test --locked -p composition-fake --test event_order

echo "==> example plan_time (50 iterations, first lines; host timing, not hardware)"
cargo run --locked -q --release -p composition-fake --example plan_time 50 >"$tmp/plan_time.txt"
head -8 "$tmp/plan_time.txt"
echo "... ($(wc -l <"$tmp/plan_time.txt") lines in all)"
n=$((n + 1))
echo "PASS  example plan_time"
echo

step "build composition-core ($T)" \
    cargo build --locked -p composition-core --target "$T"
step "build composition-ffi-staticlib ($T, no std, release)" \
    cargo build --locked -p composition-ffi-staticlib --no-default-features \
    --features panic-handler --target "$T" --release

echo "----"
echo "summary: $n steps passed, 0 failed"
echo "date:    $(date -u +%Y-%m-%d)"
echo "rev:     $rev"
echo "This was a host run. It says nothing about any board or display."
