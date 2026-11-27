#!/bin/sh
# Structural lint for the M1 (i.MX95) test report against SoW.md section 9 M1.
# Usage: check-m1-report.sh [report.md]   (default docs/m1-imx95-report.md)
#        check-m1-report.sh --self-test
#
# Report conventions checked (markdown bullets, bold, backticks are ignored):
#   Verdicts:  a line "Condition N: passed|failed|blocked" for N = 1..7, plus
#              "Overall M1 verdict: passed|failed|blocked".
#   Fields:    a line "<Label>: <value>" for each acceptance-condition-7 item
#              (board revision, panel and interface, BSP/kernel version, lease
#              route and version, resolution and refresh, plane assignments,
#              errors, frame loss, latency, CPU, memory); the value is
#              non-empty (a number, text, or "not measured").
#   Dates:     at least one YYYY-MM-DD.
#   If the overall verdict is "passed": condition 3 must be "passed", and its
#   block (its verdict line up to the next Condition line or heading) must cite
#   an existing picture (.png/.jpg/.jpeg, relative to the repository root or to
#   the report) and name plane IDs ("plane 31", "planes 31 and 45", "plane IDs: 31").
# Exit: 0 ok, 1 problems found, 2 report missing.
set -u

root=$(cd "$(dirname "$0")/.." && pwd)

check() {
    report=$1
    status=0
    if [ ! -f "$report" ]; then
        echo "check-m1-report: report not found: $report (not written yet?)" >&2
        return 2
    fi
    dir=$(dirname "$report")
    fail() {
        echo "$report: $*" >&2
        status=1
    }

    # lowercase, no emphasis/backticks, no leading bullets/pipes/quotes
    norm=$(tr 'A-Z' 'a-z' <"$report" | sed -e 's/[*`]//g' -e 's/^[ 	>|-]*//')

    verdict_3=
    n=1
    while [ "$n" -le 7 ]; do
        v=$(printf '%s\n' "$norm" |
            sed -n "s/^condition $n *[:=] *\(passed\|failed\|blocked\)\([^a-z].*\)*\$/\1/p" | head -n 1)
        [ -n "$v" ] || fail "missing or invalid verdict line 'Condition $n: passed|failed|blocked'"
        [ "$n" -eq 3 ] && verdict_3=$v
        n=$((n + 1))
    done

    for label in "board revision" "panel and interface" "bsp/kernel version" \
        "lease route and version" "resolution and refresh" "plane assignments" \
        "errors" "frame loss" "latency" "cpu" "memory"; do
        val=$(printf '%s\n' "$norm" | awk -v l="$label" '
            index($0, l) == 1 {
                s = substr($0, length(l) + 1)
                if (s !~ /^[^:]*:/) next
                sub(/^[^:]*:/, "", s)
                gsub(/^[ \t]+|[ \t|]+$/, "", s)
                if (s != "") { print s; exit }
            }')
        [ -n "$val" ] || fail "field '$label' missing or has no value (use 'not measured')"
    done

    printf '%s\n' "$norm" | grep -Eq '(^|[^0-9])[0-9]{4}-[0-9]{2}-[0-9]{2}([^0-9]|$)' ||
        fail "no date in YYYY-MM-DD form"

    overall=$(printf '%s\n' "$norm" |
        sed -n 's/^overall m1 verdict *[:=] *\(passed\|failed\|blocked\)\([^a-z].*\)*$/\1/p' | head -n 1)
    [ -n "$overall" ] || fail "missing 'Overall M1 verdict: passed|failed|blocked'"

    if [ "$overall" = passed ]; then
        if [ "$verdict_3" != passed ]; then
            fail "overall verdict is passed but condition 3 is '${verdict_3:-missing}' (must be passed)"
        else
            # block of the original text (case kept for file lookups)
            block=$(awk '
                tolower($0) ~ /^[ \t>|*-]*condition 3[^0-9]/ { on = 1; print; next }
                on && (tolower($0) ~ /^[ \t>|*-]*condition [0-9]/ || /^#/) { exit }
                on { print }' "$report" | tr -d '`*')
            found=0
            pics=$(printf '%s\n' "$block" | grep -Eio '[A-Za-z0-9_./-]+\.(png|jpe?g)' | sort -u)
            while IFS= read -r p; do
                [ -n "$p" ] || continue
                for base in "$root" "$dir"; do
                    [ -f "$base/$p" ] && found=1
                done
            done <<EOF
$pics
EOF
            [ "$found" = 1 ] ||
                fail "condition 3 is passed but cites no picture path that exists in the repository"
            printf '%s\n' "$block" | grep -Eiq 'planes? +(ids? *[:=#]? *)?#?[0-9]+' ||
                fail "condition 3 is passed but names no plane IDs"
        fi
    fi
    return "$status"
}

if [ "${1:-}" = "--self-test" ]; then
    td="$root/scripts/testdata/m1-report"
    rc=0
    check "$td/good.md" || {
        echo "self-test: good fixture was rejected" >&2
        rc=1
    }
    for fx in bad bad-pictures; do
        out=$(check "$td/$fx.md" 2>&1) && {
            echo "self-test: $fx fixture was accepted" >&2
            rc=1
        }
        case $fx in
        bad) wants="verdict line 'Condition 2|verdict line 'Condition 4|verdict line 'Condition 5|field 'latency'|field 'memory'|YYYY-MM-DD|condition 3 is 'blocked'" ;;
        *) wants="no picture path that exists|names no plane IDs" ;;
        esac
        oldifs=$IFS
        IFS='|'
        for want in $wants; do
            printf '%s\n' "$out" | grep -qF -- "$want" || {
                echo "self-test: $fx fixture lacks message: $want" >&2
                rc=1
            }
        done
        IFS=$oldifs
    done
    [ "$rc" = 0 ] && echo "self-test: ok"
    exit "$rc"
fi

check "${1:-$root/docs/m1-imx95-report.md}"
