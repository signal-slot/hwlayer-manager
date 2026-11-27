#!/bin/sh
# Check relative links in the repository's tracked Markdown files.
# Usage: scripts/check-doc-links.sh [--anchors]
# Run inside a git work tree (any subdirectory). Checks inline [t](target) and
# reference-definition "[label]: target" links; skips http(s), mailto and pure
# #anchor links. With --anchors, #fragments (including pure #anchor links) are
# also checked against the target file's headings (GitHub slug style).
# Prints "file:line: missing target" and exits 1 on any miss.

anchors=0
case ${1:-} in
    --anchors) anchors=1 ;;
    '') ;;
    *) echo "usage: $0 [--anchors]" >&2; exit 2 ;;
esac

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2

out=$(mktemp "${TMPDIR:-/tmp}/doc-links.XXXXXX") || exit 2
trap 'rm -f "$out"' EXIT HUP INT TERM

# Print "line<TAB>target" for each link outside fenced code blocks.
extract_links() {
    awk '
    /^ {0,3}(```|~~~)/ { fence = !fence; next }
    fence { next }
    {
        line = $0
        gsub(/`[^`]*`/, "", line)
        if (match(line, /^ {0,3}\[[^]]+\]:[ \t]*/)) {
            t = substr(line, RLENGTH + 1)
            sub(/[ \t].*$/, "", t)
            print NR "\t" t
        }
        while (match(line, /\]\([^)]*\)/)) {
            t = substr(line, RSTART + 2, RLENGTH - 3)
            line = substr(line, RSTART + RLENGTH)
            sub(/^[ \t]+/, "", t)
            if (t ~ /^</) { sub(/^</, "", t); sub(/>.*$/, "", t) }
            else sub(/[ \t].*$/, "", t)
            print NR "\t" t
        }
    }' "$1"
}

# Print the GitHub-style anchor slug of every heading in file $1.
slugs() {
    awk '
    /^ {0,3}(```|~~~)/ { fence = !fence; next }
    fence { next }
    /^ {0,3}#{1,6}[ \t]/ {
        h = $0
        sub(/^ {0,3}#+[ \t]+/, "", h)
        sub(/[ \t]+#+[ \t]*$/, "", h)
        gsub(/`/, "", h)
        gsub(/\]\([^)]*\)/, "", h)
        gsub(/[][]/, "", h)
        h = tolower(h)
        gsub(/[^[:alnum:] _-]/, "", h)
        gsub(/ /, "-", h)
        n = seen[h]++
        print (n ? h "-" n : h)
    }' "$1"
}

tab=$(printf '\t')
files=$(git ls-files '*.md')

for f in $files; do
    [ -f "$f" ] || continue
    dir=$(dirname "$f")
    extract_links "$f" | while IFS=$tab read -r ln target; do
        case $target in
            '' | http://* | https://* | mailto:*) continue ;;
            '#'*) [ "$anchors" = 1 ] || continue ;;
        esac
        frag=
        case $target in
            *'#'*) frag=${target#*'#'} ;;
        esac
        path=${target%%'#'*}
        path=${path%%'?'*}
        path=$(printf '%s' "$path" | sed 's/%20/ /g')
        if [ -z "$path" ]; then
            dest=$f
        else
            case $path in
                /*) dest=$root$path ;;
                *) dest=$dir/$path ;;
            esac
            if [ ! -e "$dest" ]; then
                echo "$f:$ln: missing target $target"
                continue
            fi
        fi
        if [ "$anchors" = 1 ] && [ -n "$frag" ] && [ -f "$dest" ]; then
            case $dest in
                *.md)
                    want=$(printf '%s' "$frag" | tr 'A-Z' 'a-z')
                    slugs "$dest" | grep -Fxq -- "$want" ||
                        echo "$f:$ln: missing anchor $target"
                    ;;
            esac
        fi
    done >> "$out"
done

if [ -s "$out" ]; then
    cat "$out"
    exit 1
fi
exit 0
