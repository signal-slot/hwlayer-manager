#!/bin/sh
# Check the WP5 proposal and demo docs for date, evidence tags and wording.
# Usage: check-wp5-docs.sh [repo-root]
# Checks docs/wp5-proposal.md and docs/wp5-demo.md. A missing file prints
# "skip <file>" and is not a failure. Prints one line per failure and exits 1;
# prints "ok" and exits 0 otherwise. A missing repo root is an error (exit 2).
set -u

root=${1:-$(cd "$(dirname "$0")/.." && pwd)}
status=0

if [ ! -d "$root" ]; then
    echo "error: $root not found" >&2
    exit 2
fi

date_re='[0-9]{4}-[0-9]{2}-[0-9]{2}'
# Tag words as in scripts/check-doc-tags.sh, plus "not measured" / "host only".
tag_re="observed[^0-9]{0,4}$date_re|read from source|inferred|not measured|host only"

fail() { echo "$1: $2"; status=1; }

for f in docs/wp5-proposal.md docs/wp5-demo.md; do
    p=$root/$f
    if [ ! -f "$p" ]; then
        echo "skip $f"
        continue
    fi
    head -n 10 "$p" | grep -qE -- "$date_re" || fail "$f" "no date line in the first 10 lines"
    grep -qiE -- "$tag_re" "$p" || fail "$f" "no evidence tag (observed <date> / read from source / inferred / not measured / host only)"
    case $f in
    */wp5-proposal.md)
        # Promise phrases are failures unless the same line says not/no.
        n=0
        while IFS= read -r line; do
            n=$((n + 1))
            if printf '%s\n' "$line" | grep -qiE 'will be accepted|guaranteed|accepted upstream'; then
                if ! printf '%s\n' "$line" | grep -qiE '(^|[^a-z])(not|no)([^a-z]|$)'; then
                    fail "$f:$n" "promises upstream acceptance"
                fi
            fi
        done < "$p"
        ;;
    */wp5-demo.md)
        grep -qiE '^#+[[:space:]]+.*walk-through' "$p" || fail "$f" "no 'Walk-through' heading"
        ;;
    esac
done

[ "$status" -eq 0 ] && echo ok
exit "$status"
