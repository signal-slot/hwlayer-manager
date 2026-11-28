#!/bin/sh
# Lint WP5 support docs for untagged advantage or acceptance claims.
# Usage: sh scripts/check-wp5-claims.sh [file ...]
# Exit: 0 clean, 1 findings, 2 usage/missing file. See check-wp5-claims-README.md.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
docs=$here/../docs
if [ $# -eq 0 ]; then
    set -- "$docs/wp5-gap-analysis.md" "$docs/wp5-evidence-plan.md" \
        "$docs/advisory-planner-design.md" "$docs/libliftoff-crosscheck.md" \
        "$docs/liftoff-crosscheck-report.md"
    for f in "$docs"/wp5-*.md; do
        case $f in
            */wp5-proposal.md|*/wp5-demo.md) continue ;;
            */wp5-gap-analysis.md|*/wp5-evidence-plan.md) continue ;;
        esac
        [ -f "$f" ] && set -- "$@" "$f"
    done
fi
for f in "$@"; do
    [ -f "$f" ] || { echo "check-wp5-claims: no such file: $f" >&2; exit 2; }
done
status=0
for f in "$@"; do
    awk -v file="$f" '
    function claim(s) {
        s = tolower(s)
        return s ~ /faster|lower latency|less power|will be accepted|upstream will|guarantees/
    }
    function tag(s) {
        s = tolower(s)
        gsub(/unmeasured/, "", s)
        return s ~ /not measured|observed|measured|inferred|read from source/
    }
    function flush(   i, tagged) {
        tagged = 0
        for (i = 1; i <= n; i++) if (tag(lines[i])) tagged = 1
        for (i = 1; i <= n; i++)
            if (claim(lines[i]) && !tagged) {
                printf "%s:%d: untagged claim: %s\n", file, start[i], lines[i]
                bad = 1
            }
        n = 0
    }
    /^[ \t]*$/ { flush(); next }
    { n++; lines[n] = $0; start[n] = NR }
    END { flush(); exit bad }
    ' "$f" || status=1
done
exit $status
