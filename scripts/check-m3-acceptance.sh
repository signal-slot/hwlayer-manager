#!/bin/sh
# check-m3-acceptance.sh - mechanical acceptance checks C1-C6 of the M3 (AM62P) plan.
# Usage: sh scripts/check-m3-acceptance.sh [--quick] [--root DIR]
#   --quick     skip the cargo steps (C1 prints SKIP)
#   --root DIR  tree to check (default: the repository this script lives in)
# Prints "C<n> PASS|FAIL|SKIP <reason>" per criterion, then
# "M3-ACCEPTANCE PASS" or "M3-ACCEPTANCE FAIL: C2 C4 ...". Exit 0 only if all pass.
#   C1 fmt, clippy -D warnings, test --workspace, two thumbv7em builds, board_scenes RESULT PASS
#   C2 docs/m3-am62p-report.md: >=8 '^SCENE [1-8]' lines, 'RESULT ', relative links exist
#   C3 report has a Torizon restore section that mentions ssh
#   C4 docs/observed/am62p.md exists; matrix contains 'observed (2026-'
#   C5 traceability no longer says "no AM62P observation"; both docs name m3-am62p-report
#   C6 check-no-board-secrets.sh, check-no-farm-ids.sh, check-doc-links.sh succeed
# C6 runs the scripts from DIR/scripts when present (else this script's directory),
# with DIR as the working directory; they expect DIR to be a git repository.

quick=0
here=$(cd "$(dirname "$0")" && pwd)
root=$(dirname "$here")
while [ $# -gt 0 ]; do
    case $1 in
        --quick) quick=1 ;;
        --root)
            [ $# -ge 2 ] || { echo "--root needs DIR" >&2; exit 2; }
            root=$2
            shift
            ;;
        *) echo "usage: $0 [--quick] [--root DIR]" >&2; exit 2 ;;
    esac
    shift
done
root=$(cd "$root" 2>/dev/null && pwd) || { echo "bad --root" >&2; exit 2; }
cd "$root" || exit 2

failed=
res() { # n verdict reason
    echo "C$1 $2 $3"
    if [ "$2" = FAIL ]; then failed="$failed C$1"; fi
    return 0
}
report=docs/m3-am62p-report.md
oneline() { tr '\n' ' ' | cut -c1-200; }
# add_why TEXT: append to $why with a separator
add_why() {
    if [ -z "$why" ]; then why=$1; else why="$why; $1"; fi
}

# ---- C1
if [ "$quick" = 1 ]; then
    res 1 SKIP "--quick: cargo steps not run"
else
    why=
    step() { # description, command...
        d=$1
        shift
        if out=$("$@" 2>&1); then return 0; fi
        why="$d failed: $(printf '%s' "$out" | tail -n 3 | oneline)"
        return 1
    }
    step "cargo fmt" cargo fmt --all --check &&
        step "cargo clippy" cargo clippy --workspace --all-targets -- -D warnings &&
        step "cargo test" cargo test --workspace &&
        step "core thumbv7em build" cargo build -p composition-core --target thumbv7em-none-eabihf &&
        step "staticlib thumbv7em build" cargo build -p composition-ffi-staticlib --no-default-features \
            --features panic-handler --target thumbv7em-none-eabihf --release &&
        {
            out=$(cargo test -p composition-twoplane board_scenes -- --nocapture 2>&1)
            printf '%s' "$out" | grep -q 'RESULT PASS' || why="board_scenes output lacks 'RESULT PASS'"
            [ -z "$why" ]
        }
    if [ -z "$why" ]; then res 1 PASS "all cargo steps"; else res 1 FAIL "$why"; fi
fi

# ---- C2
if [ ! -f "$report" ]; then
    res 2 FAIL "$report missing"
else
    n=$(grep -c '^SCENE [1-8]' "$report")
    why=
    [ "$n" -ge 8 ] || add_why "only $n '^SCENE [1-8]' lines (need 8)"
    grep -q 'RESULT ' "$report" || add_why "no 'RESULT ' line"
    missing=
    for l in $(grep -o ']([^)]*)' "$report" | sed 's/^](//; s/)$//' | sed 's/[[:space:]].*//'); do
        case $l in http:* | https:* | mailto:* | '#'* | '') continue ;; esac
        p=${l%%#*}
        p=${p%%\?*}
        [ -n "$p" ] || continue
        case $p in /*) t=$root$p ;; *) t=docs/$p ;; esac
        [ -e "$t" ] || missing="$missing $l"
    done
    [ -z "$missing" ] || add_why "missing link target(s):$missing"
    if [ -z "$why" ]; then
        res 2 PASS "$n SCENE lines, RESULT present, links resolve"
    else
        res 2 FAIL "$why"
    fi
fi

# ---- C3
if [ ! -f "$report" ]; then
    res 3 FAIL "$report missing"
elif ! grep -qi torizon "$report"; then
    res 3 FAIL "report never mentions Torizon"
else
    sec=$(awk '
        /^#+[ \t]/ { if (on) exit; if (tolower($0) ~ /restore/) on = 1 }
        on { print }' "$report")
    if [ -z "$sec" ]; then
        res 3 FAIL "no heading containing 'restore'"
    elif ! printf '%s\n' "$sec" | grep -qi torizon; then
        res 3 FAIL "restore section does not mention Torizon"
    elif ! printf '%s\n' "$sec" | grep -qi ssh; then
        res 3 FAIL "restore section does not mention ssh confirmation"
    else
        res 3 PASS "restore section confirms Torizon over ssh"
    fi
fi

# ---- C4
why=
[ -f docs/observed/am62p.md ] || add_why "docs/observed/am62p.md missing"
grep -q 'observed (2026-' docs/am62p-imx95-matrix.md 2>/dev/null ||
    add_why "docs/am62p-imx95-matrix.md lacks 'observed (2026-'"
if [ -z "$why" ]; then res 4 PASS "observed doc and matrix column present"; else res 4 FAIL "$why"; fi

# ---- C5
why=
tr=docs/sow-traceability.md
oi=docs/open-issues.md
if [ ! -f "$tr" ]; then
    add_why "$tr missing"
else
    if grep -q 'No document records an AM62P board observation' "$tr"; then
        add_why "$tr still says 'No document records an AM62P board observation'"
    fi
    grep -q 'm3-am62p-report' "$tr" || add_why "$tr lacks m3-am62p-report"
fi
grep -q 'm3-am62p-report' "$oi" 2>/dev/null || add_why "$oi lacks m3-am62p-report"
if [ -z "$why" ]; then res 5 PASS "traceability and open issues updated"; else res 5 FAIL "$why"; fi

# ---- C6
why=
for s in check-no-board-secrets.sh check-no-farm-ids.sh check-doc-links.sh; do
    if [ -f "$root/scripts/$s" ]; then p=$root/scripts/$s; else p=$here/$s; fi
    if ! out=$(sh "$p" 2>&1); then
        add_why "$s: $(printf '%s' "$out" | head -n 2 | oneline)"
    fi
done
if [ -z "$why" ]; then res 6 PASS "three hygiene checks succeed"; else res 6 FAIL "$why"; fi

if [ -z "$failed" ]; then
    echo "M3-ACCEPTANCE PASS"
    exit 0
fi
echo "M3-ACCEPTANCE FAIL:$failed"
exit 1
