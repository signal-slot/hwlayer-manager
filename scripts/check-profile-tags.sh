#!/bin/sh
# Lint evidence tags in DriverProfile doc comments.
# Usage: scripts/check-profile-tags.sh [FILE...]
# Default files: crates/composition-drm/src/profile.rs and
# crates/composition-drm/src/profile/*.rs (when present).
# Every "/// - " or "//! - " bullet (with its continuation doc lines, up to
# the next bullet or a blank doc line) must contain one of the words
# observed, source, inferred (case-insensitive, whole word; "sources" does not
# count). A bullet that uses "observed" must carry a YYYY-MM-DD date anywhere
# in the bullet. Source pins are not checked here.
# Prints "file:line: reason" (line of the bullet) and exits 1 on findings,
# 2 on usage errors, else 0. Not wired into CI or check-all.sh.

here=$(cd "$(dirname "$0")" && pwd) || exit 2

case ${1:-} in
    -*) echo "usage: $0 [FILE...]" >&2; exit 2 ;;
esac

if [ $# -eq 0 ]; then
    for f in "$here"/../crates/composition-drm/src/profile.rs \
             "$here"/../crates/composition-drm/src/profile/*.rs; do
        [ -f "$f" ] && set -- "$@" "$f"
    done
    [ $# -gt 0 ] || { echo "usage: $0 [FILE...] (no profile sources found)" >&2; exit 2; }
fi

for f in "$@"; do
    [ -f "$f" ] || { echo "$0: not a file: $f" >&2; exit 2; }
done

rc=0
for f in "$@"; do
    out=$(awk -v F="$f" '
    function flush(   t) {
        if (!inb) return
        t = tolower(txt)
        if (t !~ /(^|[^a-z0-9_])(observed|source|inferred)($|[^a-z0-9_])/)
            printf "%s:%d: bullet has no evidence tag (observed / source / inferred)\n", F, start
        else if (t ~ /(^|[^a-z0-9_])observed($|[^a-z0-9_])/ &&
                 t !~ /[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]/)
            printf "%s:%d: observed without a YYYY-MM-DD date\n", F, start
        inb = 0; txt = ""
    }
    {
        l = $0
        if (l !~ /^[ \t]*(\/\/\/|\/\/!)/) { flush(); next }
        sub(/^[ \t]*(\/\/\/|\/\/!)/, "", l)
        if (l ~ /^[ \t]*$/) { flush(); next }
        if (l ~ /^[ \t]*- /) { flush(); inb = 1; start = NR; txt = l; next }
        if (inb) txt = txt " " l
    }
    END { flush() }' "$f")
    if [ -n "$out" ]; then
        printf '%s\n' "$out"
        rc=1
    fi
done
exit "$rc"
