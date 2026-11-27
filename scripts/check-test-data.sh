#!/bin/bash
# Lint the dated board data under crates/composition-drm/tests/data.
# Usage: scripts/check-test-data.sh [DATA_ROOT]
# Default root: crates/composition-drm/tests/data of the repository this
# script lives in. Each DATA_ROOT/<soc>/ folder is checked:
#   1. README.md exists;
#   2. every other file (recursively) is named in README.md (its relative
#      path or its base name, as a fixed string);
#   3. every data file carries a YYYY-MM-DD date in its name, in its first
#      10 lines, or on a README.md line that names it;
#   4. no file is over 1 MiB;
#   5. scripts/check-no-board-secrets.sh reports nothing for the folder. That
#      script takes no paths: it scans the added lines of the git diff against
#      the base ref, so it is run from the data root and only its hits under
#      the folder count. A root outside a git work tree skips this check
#      (noted on stderr). CHECK_NO_BOARD_SECRETS=PATH replaces the script
#      (used by the self-test).
# A missing root, or one with no folders, prints a note and exits 0.
# Prints "FILE: reason" per problem; exit 1 on problems, 2 on usage errors.

here=$(cd "$(dirname "$0")" && pwd) || exit 2
secrets=${CHECK_NO_BOARD_SECRETS:-$here/check-no-board-secrets.sh}

case ${1:-} in
    -*) echo "usage: $0 [DATA_ROOT]" >&2; exit 2 ;;
esac
[ $# -le 1 ] || { echo "usage: $0 [DATA_ROOT]" >&2; exit 2; }
root=${1:-$here/../crates/composition-drm/tests/data}

if [ ! -d "$root" ]; then
    echo "check-test-data: note: no data root $root; nothing to check"
    exit 0
fi
root=$(cd "$root" && pwd) || exit 2

date_re='[0-9]{4}-[0-9]{2}-[0-9]{2}'
rc=0
nfolders=0
prefix=
in_git=0
if (cd "$root" && git rev-parse --show-prefix >/dev/null 2>&1); then
    in_git=1
    prefix=$(cd "$root" && git rev-parse --show-prefix)
fi

problem() { printf '%s: %s\n' "$1" "$2"; rc=1; }

# One secrets scan for the whole root; hits are filtered per folder below.
secrets_out=
if [ "$in_git" = 1 ] || [ -n "${CHECK_NO_BOARD_SECRETS:-}" ]; then
    secrets_out=$(cd "$root" && sh "$secrets" 2>&1 >/dev/null)
else
    echo "check-test-data: note: $root is not in a git work tree; secrets check skipped" >&2
fi

for dir in "$root"/*/; do
    [ -d "$dir" ] || continue
    soc=$(basename "$dir")
    nfolders=$((nfolders + 1))
    readme=$dir/README.md
    if [ ! -f "$readme" ]; then
        problem "$soc/README.md" "missing README.md"
    fi
    while IFS= read -r f; do
        rel=${f#"$dir"}
        [ "$rel" = README.md ] && continue
        label=$soc/$rel
        lines=
        if [ -f "$readme" ]; then
            base=$(basename "$rel")
            lines=$(grep -F -e "$rel" -e "$base" "$readme")
            if [ -z "$lines" ]; then
                problem "$label" "not named in README.md"
            fi
        fi
        if ! printf '%s\n' "$rel" | grep -Eq "$date_re" &&
           ! head -n 10 "$f" 2>/dev/null | grep -aEq "$date_re" &&
           ! printf '%s\n' "$lines" | grep -Eq "$date_re"; then
            problem "$label" "no YYYY-MM-DD date in name, first 10 lines or README line"
        fi
        size=$(wc -c <"$f")
        if [ "$size" -gt 1048576 ]; then
            problem "$label" "over 1 MiB ($size bytes)"
        fi
    done < <(find "$dir" -type f | LC_ALL=C sort)

    if [ -n "$secrets_out" ]; then
        hits=$(printf '%s\n' "$secrets_out" | grep -F -e ": $prefix$soc/")
        if [ -n "$hits" ]; then
            printf '%s\n' "$hits" | sed -E "s#^([^:]*): ([^:]*):([0-9]+): .*#\2: board secret (\1) at line \3#"
            rc=1
        fi
    fi
done

if [ "$nfolders" = 0 ]; then
    echo "check-test-data: note: no data folders under $root yet; nothing to check"
    exit 0
fi
exit "$rc"
