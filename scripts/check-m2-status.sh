#!/bin/sh
# Check docs/m2-status.md against the proposed M2 item list of
# docs/imx95-zephyr-feasibility.md.
#   sh scripts/check-m2-status.sh [FEASIBILITY_DOC [STATUS_DOC]]
# Item ids: the M2-NN tokens in the section "Proposed M2 items" of the
# feasibility doc; if it has none, its numbered items, as "feasibility N".
# Checks: every id has a table row in the status doc; no table row says
# "observed on board" without a YYYY-MM-DD date.
# Exit 0 with "ok", or 1 with one line per problem.
here=$(cd "$(dirname "$0")" && pwd)
root=$(dirname "$here")
feas=${1:-$root/docs/imx95-zephyr-feasibility.md}
status=${2:-$root/docs/m2-status.md}
for f in "$feas" "$status"; do
    if [ ! -r "$f" ]; then
        echo "cannot read $f"
        exit 1
    fi
done

section=$(awk '/^## /{on=($0 ~ /[Pp]roposed M2 items/)} on' "$feas")
ids=$(printf '%s\n' "$section" | grep -oE 'M2-[0-9]+' | sort -u)
mode=m2
if [ -z "$ids" ]; then
    mode=num
    ids=$(printf '%s\n' "$section" | sed -n 's/^\([0-9][0-9]*\)\. .*/\1/p' | sort -un)
fi
if [ -z "$ids" ]; then
    echo "no proposed M2 items found in $feas"
    exit 1
fi

rows=$(grep '^|' "$status")
problems=""
nl='
'
for id in $ids; do
    if [ "$mode" = m2 ]; then
        pat="^\\| *$id([^0-9]|\$)"
        label=$id
    else
        pat="[Ff]easibility $id([^0-9]|\$)"
        label="feasibility $id"
    fi
    if ! printf '%s\n' "$rows" | grep -qE -- "$pat"; then
        problems="$problems${nl}no row for $label in $(basename "$status")"
    fi
done

bad=$(printf '%s\n' "$rows" | grep -iE 'observed on board' |
    grep -vE '[0-9]{4}-[0-9]{2}-[0-9]{2}' | cut -c1-60)
if [ -n "$bad" ]; then
    while IFS= read -r l; do
        problems="$problems${nl}'observed on board' without a YYYY-MM-DD date: $l"
    done <<EOT
$bad
EOT
fi

if [ -n "$problems" ]; then
    printf '%s\n' "$problems" | sed 1d
    exit 1
fi
echo ok
