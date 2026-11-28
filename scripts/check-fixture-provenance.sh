#!/bin/sh
# check-fixture-provenance.sh - lint recorded board fixtures in Rust test code.
#
# Plan criterion c4: recorded board data is copied into the tests (or into
# crates/*/tests/data/) and the tests say where it came from. Tests never
# read docs/ at run time. Two checks:
#   runtime-read  include_str!/include_bytes!/read_to_string/File::open/
#                 fs::read whose argument (this line or the next two)
#                 mentions docs/platform-capabilities.md or the retired
#                 notes directory under docs/. Comment lines are ignored.
#   provenance    a file that mentions a recorded board (nanopc, rk3588,
#                 bcm2711, bcm2712, cm4, rpi5; case-insensitive) and embeds
#                 data (a raw string literal, a const/static &str or &[..]
#                 item, or an include_str!/include_bytes!) needs one comment
#                 block (consecutive // lines) that names a tests/data/PATH
#                 or docs/platform-capabilities.md source AND a YYYY-MM-DD
#                 date. The retired notes directory is not a source. A comment line
#                 "provenance: synthetic" declares that the file holds no
#                 recorded board data (a board name used only as a string).
#
# Run: scripts/check-fixture-provenance.sh [FILE.rs ...]
#   no arguments: crates/*/tests/**/*.rs, crates/*/examples/*.rs and
#   tools/**/*.rs under the repository root.
# Output: "file:line: kind: message". Exit 0 clean, 1 findings, 2 usage.

case ${1-} in
-*)
    echo "usage: $0 [FILE.rs ...]" >&2
    exit 2
    ;;
esac

if [ "$#" -eq 0 ]; then
    root=$(cd "$(dirname "$0")/.." && pwd) || exit 2
    cd "$root" || exit 2
    list=$(mktemp "${TMPDIR:-/tmp}/cfp.XXXXXX") || exit 2
    trap 'rm -f "$list"' EXIT HUP INT TERM
    {
        find crates -path 'crates/*/tests/*' -name '*.rs' 2>/dev/null
        find crates -path 'crates/*/examples/*' -name '*.rs' 2>/dev/null
        find tools -name '*.rs' -not -path '*/target/*' 2>/dev/null
    } | sort -u >"$list"
    set --
    while IFS= read -r f; do set -- "$@" "$f"; done <"$list"
else
    for f in "$@"; do
        [ -f "$f" ] || { echo "check-fixture-provenance: not a file: $f" >&2; exit 2; }
    done
fi

rc=0
for f in "$@"; do
    out=$(awk -v F="$f" '
    function flush_block() {
        if (inblk && bpath && bdate) ok = 1
        inblk = 0; bpath = 0; bdate = 0
    }
    { line[NR] = $0 }
    END {
        for (i = 1; i <= NR; i++) {
            s = line[i]
            if (s ~ /^[ \t]*\/\//) {
                inblk = 1
                if (s ~ /(docs\/platform-capabilities\.md|tests\/data\/[A-Za-z0-9_.\/-]+)/) bpath = 1
                if (s ~ /provenance: synthetic/) synth = 1
                if (s ~ /[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]/) bdate = 1
            }
            else flush_block()
            if (tolower(s) ~ /nanopc|rk3588|bcm2711|bcm2712|cm4|rpi5/) board = 1
            if (s ~ /^[ \t]*\/\//) continue
            if (s ~ /(^|[^A-Za-z0-9_])r#*"/ || s ~ /(const|static)[ \t]+[A-Za-z0-9_]+[ \t]*:[ \t]*&(\[|str)/ || s ~ /include_(str|bytes)!/) embeds = 1
            if (s ~ /(include_str!|include_bytes!|read_to_string|File::open|fs::read)/) {
                w = s " " line[i + 1] " " line[i + 2]
                if (w ~ /docs\/(observed|platform-capabilities)/)
                    printf "%s:%d: runtime-read: loads a recorded-data doc (copy the data into the test with source and date)\n", F, i
            }
        }
        flush_block()
        if (board && embeds && !ok && !synth)
            printf "%s:1: provenance: board fixture without a comment naming a tests/data/ or docs/platform-capabilities.md source and a YYYY-MM-DD date\n", F
    }' "$f")
    if [ -n "$out" ]; then
        printf '%s\n' "$out"
        rc=1
    fi
done
[ "$rc" -eq 0 ] && echo "check-fixture-provenance: clean"
exit "$rc"
