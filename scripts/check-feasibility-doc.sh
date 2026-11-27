#!/bin/sh
# Mechanical check of the i.MX95 Zephyr feasibility doc.
# Usage: scripts/check-feasibility-doc.sh [FILE]
# Default FILE: docs/imx95-zephyr-feasibility.md of this repository.
# Checks:
#   1. the file exists;
#   2. it contains each of "observed", "read from source", "inferred"
#      (case-insensitive);
#   3. it contains at least one YYYY-MM-DD date;
#   4. every http(s) URL to a git host (github.com, codeload.github.com,
#      raw.githubusercontent.com, git.toradex.com, source.codeaurora.org,
#      git.codelinaro.org, git.kernel.org, gitlab.com, ...) names a commit SHA
#      (7 to 40 hex characters, at least one digit) or a tag (v1.2, name-1.2,
#      refs/tags/, /releases/tag/) in the URL or on the same line.
# Prints "ok", or one "fail <line>: <reason>" per finding (line 0 = whole file).
# Exit: 0 pass, 1 failure, 2 usage error.

here=$(cd "$(dirname "$0")" && pwd) || exit 2

case $# in
    0) f=$here/../docs/imx95-zephyr-feasibility.md ;;
    1) case $1 in -*) echo "usage: $0 [FILE]" >&2; exit 2 ;; esac
       f=$1 ;;
    *) echo "usage: $0 [FILE]" >&2; exit 2 ;;
esac

if [ ! -f "$f" ]; then
    echo "fail 0: file not found"
    exit 1
fi

out=$(awk '
function has_sha(s,   t) {
    while (match(s, /[A-Za-z0-9]+/)) {
        t = substr(s, RSTART, RLENGTH)
        s = substr(s, RSTART + RLENGTH)
        if (length(t) >= 7 && length(t) <= 40 && t ~ /^[0-9a-f]+$/ && t ~ /[0-9]/) return 1
    }
    return 0
}
function has_tag(s) {
    return s ~ /(^|[^A-Za-z0-9])[vV][0-9]+\.[0-9]+/ ||
           s ~ /[A-Za-z][-_][0-9]+\.[0-9]+/ ||
           s ~ /refs\/tags\// || s ~ /releases\/tag\//
}
{
    l = tolower($0)
    if (l ~ /observed/) o = 1
    if (l ~ /read from source/) r = 1
    if (l ~ /inferred/) i = 1
    if ($0 ~ /[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]/) d = 1
    if (l ~ /https?:\/\/([a-z0-9.-]*\.)?(github\.com|githubusercontent\.com|git\.toradex\.com|source\.codeaurora\.org|git\.codelinaro\.org|git\.kernel\.org|gitlab\.com|googlesource\.com|bitbucket\.org)([\/:?#)>` ]|$)/ &&
        !has_sha($0) && !has_tag($0))
        printf "fail %d: git-host URL without a commit SHA or tag\n", NR
}
END {
    if (!o) print "fail 0: no \"observed\" tag"
    if (!r) print "fail 0: no \"read from source\" tag"
    if (!i) print "fail 0: no \"inferred\" tag"
    if (!d) print "fail 0: no YYYY-MM-DD date"
}' "$f")

if [ -n "$out" ]; then
    printf '%s\n' "$out"
    exit 1
fi
echo ok
