#!/bin/sh
# Lint evidence tags, dates and source pins in the hardware docs.
# Usage: scripts/check-doc-tags.sh [FILE...]
# Default files: docs/imx95-*.md of the repository this script lives in.
# Checks per file:
#   1. at least one evidence tag occurs: "observed", "read from source",
#      "inferred" (case-insensitive);
#   2. every line using "observed" as a tag (written [observed], (observed,
#      **observed**, `observed`, "| observed |" or "observed:" at line start)
#      carries a YYYY-MM-DD date, or the nearest earlier line of the same
#      heading section does (a heading's own date counts);
#   3. every https URL to github.com, git.toradex.com, codeload.github.com or
#      raw.githubusercontent.com has a 40-hex SHA or a tag-looking ref
#      (v1.2, name-1.2, refs/tags/, /releases/tag/) on its line or within two
#      lines either side.
# Prints "file:line: reason" and exits 1 on findings, 2 on usage errors, else 0.

here=$(cd "$(dirname "$0")" && pwd) || exit 2

case ${1:-} in
    -*) echo "usage: $0 [FILE...]" >&2; exit 2 ;;
esac

if [ $# -eq 0 ]; then
    for f in "$here"/../docs/imx95-*.md; do
        [ -f "$f" ] && set -- "$@" "$f"
    done
    [ $# -gt 0 ] || { echo "usage: $0 [FILE...] (no docs/imx95-*.md found)" >&2; exit 2; }
fi

for f in "$@"; do
    [ -f "$f" ] || { echo "$0: not a file: $f" >&2; exit 2; }
done

rc=0
for f in "$@"; do
    out=$(awk -v F="$f" '
    function has_date(s) {
        return s ~ /[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]/
    }
    function has_sha(s,   t) {
        while (match(s, /[A-Za-z0-9]+/)) {
            t = substr(s, RSTART, RLENGTH)
            s = substr(s, RSTART + RLENGTH)
            if (length(t) == 40 && t ~ /^[0-9a-f]+$/) return 1
        }
        return 0
    }
    function has_tagref(s) {
        return s ~ /(^|[^A-Za-z0-9])[vV][0-9]+\.[0-9]+/ ||
               s ~ /[A-Za-z][-_][0-9]+\.[0-9]+/ ||
               s ~ /refs\/tags\// || s ~ /releases\/tag\//
    }
    { line[NR] = $0 }
    END {
        tagged = 0
        sec = 0
        for (i = 1; i <= NR; i++) {
            l = tolower(line[i])
            if (l ~ /observed|read from source|inferred/) tagged = 1

            if (l ~ /^#+[ \t]/) sec = 0
            if (has_date(l)) sec = 1
            if ((l ~ /(^|[\[(*`|])[ ]*observed($|[^a-z])/ || l ~ /^[ \t>*-]*observed:/) && !sec)
                printf "%s:%d: observed tag without a YYYY-MM-DD date\n", F, i

            if (l ~ /https:\/\/(www\.)?(github\.com|git\.toradex\.com|codeload\.github\.com|raw\.githubusercontent\.com)/) {
                ok = 0
                for (j = i - 2; j <= i + 2; j++)
                    if (j >= 1 && j <= NR && (has_sha(line[j]) || has_tagref(line[j]))) ok = 1
                if (!ok)
                    printf "%s:%d: git-host URL without a 40-hex SHA or tag ref nearby\n", F, i
            }
        }
        if (!tagged)
            printf "%s:1: no evidence tag (observed / read from source / inferred)\n", F
    }' "$f")
    if [ -n "$out" ]; then
        printf '%s\n' "$out"
        rc=1
    fi
done
exit "$rc"
