#!/bin/sh
# Lint board-side scripts for constructs that busybox sh/awk (Torizon) lacks.
# Usage: check-busybox-compat.sh [FILE...]
#   default: scripts/am62p-*.sh except *.test.sh
# Prints file:line:pattern per hit, exits 1 on any hit (2 on usage error).
# Opt out on a line with:  # busybox-ok: <reason>   (a reason is required).
# Full-line comments are not checked.
set -u
here=$(cd "$(dirname "$0")" && pwd)
if [ $# -eq 0 ]; then
    for f in "$here"/am62p-*.sh; do
        [ -e "$f" ] || continue
        case $f in *.test.sh) continue ;; esac
        set -- "$@" "$f"
    done
fi
[ $# -gt 0 ] || {
    echo "no files to check" >&2
    exit 2
}
for f in "$@"; do
    [ -f "$f" ] || {
        echo "$f: not a file" >&2
        exit 2
    }
done

# One pattern per line: NAME|ERE (NAME contains no '|').
patterns='awk strtonum|(^|[^[:alnum:]_])strtonum[[:space:]]*\(
awk gensub|(^|[^[:alnum:]_])gensub[[:space:]]*\(
printf %x of a string|printf[^#]*%[-0-9.]*[xX][^#]*[ ,]["'\''][A-Za-z_]
stat -c|(^|[^[:alnum:]_-])stat[[:space:]]+(-[a-zA-Z]+[[:space:]]+)*-c
bash array assignment|(^|[[:space:];])[A-Za-z_][A-Za-z_0-9]*\+?=\(
bash array expansion|\$\{[#!]?[A-Za-z_][A-Za-z_0-9]*\[
declare -a/-A|(^|[[:space:];])(declare|typeset)[[:space:]]+-[a-zA-Z]*[aA]
[[ ]]|(^|[[:space:];&|(])\[\[([[:space:]]|$)
local -n|(^|[[:space:];])local[[:space:]]+-[a-zA-Z]*n
readarray/mapfile|(^|[^[:alnum:]_-])(readarray|mapfile)([^[:alnum:]_]|$)
sed -E/-r|(^|[^[:alnum:]_-])sed[[:space:]]([^|;#]*[[:space:]])?-[a-zA-Z]*[Er]([[:space:]]|[a-zA-Z]*$)
grep -P|(^|[^[:alnum:]_-])e?grep[[:space:]]([^|;#]*[[:space:]])?-[a-zA-Z]*P
process substitution|[<>]\([^)]'

out=$(
    for f in "$@"; do
        printf '%s\n' "$patterns" | while IFS= read -r line; do
            name=${line%%|*}
            re=${line#*|}
            grep -nE -e "$re" "$f" | while IFS= read -r hit; do
                n=${hit%%:*}
                text=${hit#*:}
                trimmed=${text#"${text%%[![:space:]]*}"}
                case $trimmed in
                '#'*) continue ;;
                esac
                case $text in
                *'# busybox-ok:'*[![:space:]]*) continue ;;
                esac
                printf '%s:%s:%s\n' "$f" "$n" "$name"
            done
        done | sort -t: -k2,2n
    done
)
if [ -n "$out" ]; then
    printf '%s\n' "$out"
    exit 1
fi
exit 0
