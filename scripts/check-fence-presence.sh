#!/bin/sh
# Check docs/fences/property-presence.md against the recorded probe files.
# Usage: scripts/check-fence-presence.sh [--rows] [DOC]
#   DOC defaults to docs/fences/property-presence.md of this repository.
#   --rows prints, for every recorded file that lists planes or CRTCs, the
#   expected table cells: "path | planes cell | CRTCs cell".
# A table row is a line with a backticked path to a recorded file. Its
# cells are: SoC | board, kernel | path | date | planes | CRTCs | tag, where
# the planes cell starts "N/M: ids" (N of M planes list IN_FENCE_FD, ids =
# those plane ids) and the CRTCs cell likewise for OUT_FENCE_PTR.
# Checks: every row agrees with its file; the date occurs in the file name,
# its first 20 lines or its directory's README.md; every recorded file that
# lists planes or CRTCs has a row.
# Recorded files: crates/composition-drm/tests/data/*/*.txt,
# docs/imx95-probe-*.txt, docs/observed/*/*.txt.
# Prints "doc:line: reason" and exits 1 on findings, 2 on usage errors.

here=$(cd "$(dirname "$0")" && pwd) || exit 2
root=$(cd "$here/.." && pwd) || exit 2
rows=0
if [ "$1" = --rows ]; then rows=1; shift; fi
[ $# -le 1 ] || { echo "usage: $0 [--rows] [DOC]" >&2; exit 2; }
doc=$1
[ -n "$doc" ] || doc=docs/fences/property-presence.md
cd "$root" || exit 2

# cells FILE -> "N/M: ids|N/M: ids" (planes, CRTCs); empty if no listing.
cells() {
    awk '
    /^(plane [0-9]+( type |:)|crtc [0-9]+ (\(index|active|inactive))/ {
        id = $2; sub(/:$/, "", id)
        if ($1 == "plane") { sec = "p"; np++ } else { sec = "c"; nc++ }
        next
    }
    /^(planes|crtcs|connector|connectors|plane|device|raw|output)([ :]|$)/ { sec = ""; next }
    sec == "p" && /^[ \t]+IN_FENCE_FD[ :(]/ { pi = pi " " id; ip++; sec = "" }
    sec == "c" && /^[ \t]+OUT_FENCE_PTR[ :(]/ { co = co " " id; ic++; sec = "" }
    END {
        if (np + nc == 0) exit
        printf "%d/%d%s|%d/%d%s\n", ip, np, (ip ? ":" pi : ""), ic, nc, (ic ? ":" co : "")
    }' "$1"
}

files=$(ls crates/composition-drm/tests/data/*/*.txt docs/imx95-probe-*.txt \
    docs/observed/*/*.txt 2>/dev/null)

if [ $rows -eq 1 ]; then
    for f in $files; do
        c=$(cells "$f")
        [ -n "$c" ] && printf '%s | %s\n' "$f" "$(printf '%s' "$c" | sed 's/|/ | /')"
    done
    exit 0
fi

[ -f "$doc" ] || { echo "$0: not a file: $doc" >&2; exit 2; }
rc=0
seen=$(mktemp "$TMPDIR/fence-seen.XXXXXX" 2>/dev/null || mktemp /tmp/fence-seen.XXXXXX) || exit 2
trap 'rm -f "$seen"' EXIT HUP INT TERM

n=0
while IFS= read -r line; do
    n=$((n + 1))
    case $line in '|'*) ;; *) continue ;; esac
    path=$(printf '%s\n' "$line" | sed -n 's/.*`\([^` ]*\.txt\)`.*/\1/p')
    [ -n "$path" ] || continue
    if [ ! -f "$path" ]; then
        echo "$doc:$n: no such recorded file: $path"; rc=1; continue
    fi
    echo "$path" >> "$seen"
    want=$(cells "$path")
    # awk fields (separator |): $1 empty, $2 SoC, $3 board, $4 path, $5 date,
    # $6 planes, $7 CRTCs.
    date=$(printf '%s\n' "$line" | awk -F'|' '{ gsub(/^ +| +$/, "", $5); print $5 }')
    pc=$(printf '%s\n' "$line" | awk -F'|' '{ gsub(/^ +| +$/, "", $6); print $6 }')
    cc=$(printf '%s\n' "$line" | awk -F'|' '{ gsub(/^ +| +$/, "", $7); print $7 }')
    if [ -z "$want" ]; then
        echo "$doc:$n: $path lists no plane or CRTC but has a row"; rc=1; continue
    fi
    wp=${want%%|*}; wc=${want#*|}
    case $pc in "$wp"*) ;; *) echo "$doc:$n: $path planes: table '$pc', data '$wp'"; rc=1 ;; esac
    case $cc in "$wc"*) ;; *) echo "$doc:$n: $path CRTCs: table '$cc', data '$wc'"; rc=1 ;; esac
    case $date in
        [0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]) ;;
        *) echo "$doc:$n: $path: bad date cell '$date'"; rc=1; continue ;;
    esac
    case $path in *"$date"*) continue ;; esac
    if head -n 20 "$path" | grep -q "$date"; then continue; fi
    dir=$(dirname "$path")
    if [ -f "$dir/README.md" ] && grep -q "$date" "$dir/README.md"; then continue; fi
    echo "$doc:$n: $path: date $date not found in the file or its README.md"; rc=1
done < "$doc"

for f in $files; do
    [ -n "$(cells "$f")" ] || continue
    grep -qxF "$f" "$seen" || { echo "$doc: no row for recorded file $f"; rc=1; }
done
exit $rc
