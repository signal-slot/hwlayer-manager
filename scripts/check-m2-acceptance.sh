#!/bin/bash
# check-m2-acceptance.sh - run the M2 / WP5 acceptance checks c1-c8 mechanically.
# Usage: scripts/check-m2-acceptance.sh [--cargo] [BASE_REF]   (default origin/main)
# Prints one "PASS cN", "FAIL cN: reason" or "SKIP cN: reason" line per check.
# Exit 1 if any FAIL, else 0. Root of the checked tree: M2_ACCEPT_ROOT, else
# the repository this script lives in (the tests point it at throwaway repos).
#   c1 feasibility doc has the three evidence tags and a date (+ its own lint)
#   c2 platform-capabilities.md points at the doc; diff touches only 'i.MX95, Zephyr'
#   c3 tests/board/imx95_m7 has CMakeLists.txt, prj.conf and a non-empty src/
#   c4 feasibility doc mentions remoteproc
#   c5 README.md / CONTRIBUTING.md exist with the required strings
#   c6 SKIP unless --cargo: fmt, clippy, tests and the two thumbv7em builds
#   c7 no address-like, tty, password or user@host strings in the diff since BASE_REF
#   c8 board-side, always SKIP

cargo=0 base=origin/main
for a in "$@"; do
    case $a in
        --cargo) cargo=1 ;;
        -*) echo "usage: $0 [--cargo] [BASE_REF]" >&2; exit 2 ;;
        *) base=$a ;;
    esac
done
here=$(cd "$(dirname "$0")" && pwd)
root=${M2_ACCEPT_ROOT:-$(dirname "$here")}
cd "$root" || exit 2
fail=0
pass() { echo "PASS $1"; }
bad() { echo "FAIL $1: $2"; fail=1; }
skip() { echo "SKIP $1: $2"; }

doc=docs/imx95-zephyr-feasibility.md
cap=docs/platform-capabilities.md

# c1
if [ ! -f "$doc" ]; then
    bad c1 "$doc missing"
else
    miss=
    for t in 'observed' 'read from source' 'inferred'; do
        grep -qi -- "$t" "$doc" || miss="$miss '$t'"
    done
    grep -Eq '[0-9]{4}-[0-9]{2}-[0-9]{2}' "$doc" || miss="$miss 'YYYY-MM-DD date'"
    if [ -n "$miss" ]; then
        bad c1 "doc lacks$miss"
    elif [ -f scripts/check-feasibility-doc.sh ] &&
        ! out=$(sh scripts/check-feasibility-doc.sh "$doc" 2>&1); then
        bad c1 "check-feasibility-doc.sh: $(printf '%s' "$out" | head -n 3 | tr '\n' ' ')"
    else
        pass c1
    fi
fi

have_base=1 why=
if ! git rev-parse --verify -q "$base^{commit}" >/dev/null; then
    have_base=0 why="base ref '$base' does not resolve"
elif [ "$(git rev-parse "$base^{commit}")" = "$(git rev-parse HEAD)" ]; then
    have_base=0 why="base ref '$base' equals HEAD"
fi

# c2
if ! grep -q 'imx95-zephyr-feasibility\.md' "$cap" 2>/dev/null; then
    bad c2 "$cap does not mention imx95-zephyr-feasibility.md"
elif [ "$have_base" = 0 ]; then
    skip c2 "$why (pointer present, diff not checked)"
else
    # section range in the working-tree file: heading to the line before the next
    # heading of the same or a higher level
    range=$(awk '
        s == 0 && /^#+[ \t]/ && index($0, "i.MX95, Zephyr") { s = NR; match($0, /^#+/); lvl = RLENGTH; next }
        s && /^#+[ \t]/ { match($0, /^#+/); if (RLENGTH <= lvl) { print s, NR - 1; found = 1; exit } }
        END { if (s && !found) print s, NR }' "$cap")
    if [ -z "$range" ]; then
        bad c2 "no 'i.MX95, Zephyr' heading in $cap"
    else
        read -r lo hi <<<"$range"
        outside=
        while read -r _ _ new _; do
            new=${new#+}
            c=${new%%,*}
            n=1
            case $new in *,*) n=${new#*,} ;; esac
            if [ "$n" = 0 ]; then # pure deletion after line c
                if [ "$c" -lt "$lo" ] || [ "$c" -gt "$hi" ]; then outside="$outside $new"; fi
            elif [ "$c" -lt "$lo" ] || [ $((c + n - 1)) -gt "$hi" ]; then
                outside="$outside $new"
            fi
        done < <(git diff -U0 "$base" -- "$cap" | grep '^@@')
        if [ -n "$outside" ]; then
            bad c2 "changes outside lines $lo-$hi:$outside"
        else
            pass c2
        fi
    fi
fi

# c3
d=tests/board/imx95_m7
if [ ! -f $d/CMakeLists.txt ] || [ ! -f $d/prj.conf ]; then
    bad c3 "$d/CMakeLists.txt or prj.conf missing"
elif [ -z "$(ls -A $d/src 2>/dev/null)" ]; then
    bad c3 "$d/src missing or empty"
else
    pass c3
fi

# c4
if [ -f "$doc" ] && grep -qi remoteproc "$doc"; then
    pass c4
else
    bad c4 "$doc missing or does not mention remoteproc"
fi

# c5
miss=
[ -f README.md ] || miss="$miss README.md"
[ -f CONTRIBUTING.md ] || miss="$miss CONTRIBUTING.md"
if [ -z "$miss" ]; then
    grep -q 'forbid(unsafe_code)' CONTRIBUTING.md || miss="$miss CONTRIBUTING:forbid(unsafe_code)"
    grep -q 'composition_engine\.h' CONTRIBUTING.md || miss="$miss CONTRIBUTING:composition_engine.h"
    grep -q 'SoW\.md' README.md || miss="$miss README:SoW.md"
    grep -q 'imx95-zephyr-feasibility\.md' README.md || miss="$miss README:imx95-zephyr-feasibility.md"
fi
if [ -n "$miss" ]; then bad c5 "missing$miss"; else pass c5; fi

# c6
if [ "$cargo" = 0 ]; then
    skip c6 "pass --cargo to run the cargo checks"
else
    T=thumbv7em-none-eabihf
    cfail=
    cargo fmt --all --check >&2 || cfail="$cfail fmt"
    cargo clippy --locked --workspace --all-targets -- -D warnings >&2 || cfail="$cfail clippy"
    cargo test --locked --workspace >&2 || cfail="$cfail test"
    cargo build --locked -p composition-core --target $T >&2 || cfail="$cfail build-core"
    cargo build --locked -p composition-ffi-staticlib --no-default-features \
        --features panic-handler --target $T --release >&2 || cfail="$cfail build-staticlib"
    if [ -n "$cfail" ]; then bad c6 "failed:$cfail"; else pass c6; fi
fi

# c7
if [ "$have_base" = 0 ]; then
    skip c7 "$why"
else
    hits=$(git diff "$base" -- README.md CONTRIBUTING.md docs/imx95-zephyr-feasibility.md \
        docs/imx95-zephyr tests/board |
        grep -Ei '([0-9]{1,3}\.){3}[0-9]{1,3}|/dev/tty|password|@[a-z0-9-]+\.')
    if [ -n "$hits" ]; then
        bad c7 "$(printf '%s\n' "$hits" | wc -l) matching diff line(s), first: $(printf '%s\n' "$hits" | head -n 1 | cut -c1-80)"
    else
        pass c7
    fi
fi

skip c8 "board-side, see the board item reports"
exit $fail
