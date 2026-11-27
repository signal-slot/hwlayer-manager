#!/bin/sh
# Lint commit subjects against the "Commit messages" section of CONTRIBUTING.md.
#
# Usage: scripts/check-commit-subjects.sh [BASE_REF]   (default: origin/main,
#        falling back to main when origin/main does not exist)
#
# For every non-merge commit in BASE_REF..HEAD:
#   - the subject is not empty and has at most 72 characters;
#   - a subject with a component prefix ("engine: ...", "CI: ...") does not end
#     in a period (a plain sentence may);
#   - the subject holds no "WIP" word and does not start with "fixup!";
#   - the commit has a body when its diff touches a path under crates/.
# CONTRIBUTING.md asks for the design in the body of every commit; only the
# crates/ case is enforced, as a heuristic. Nothing else is checked.
#
# Output: "ok <sha>" or "fail <sha>: <reason>" per commit; exit 1 on any fail,
# 2 when the base ref cannot be resolved.
set -u

base=${1:-origin/main}

if ! git rev-parse --verify --quiet "$base^{commit}" >/dev/null; then
    if [ $# -eq 0 ] && git rev-parse --verify --quiet "main^{commit}" >/dev/null; then
        base=main
    else
        echo "error: cannot resolve base ref '$base'" >&2
        exit 2
    fi
fi

list=$(mktemp "${TMPDIR:-/tmp}/commit-subjects.XXXXXX") || exit 2
trap 'rm -f "$list"' EXIT HUP INT TERM
git rev-list --no-merges --reverse "$base..HEAD" >"$list" || exit 2

rc=0
while read -r sha; do
    [ -n "$sha" ] || continue
    short=$(git rev-parse --short "$sha")
    subject=$(git log -1 --format=%s "$sha")
    reason=
    if [ -z "$subject" ]; then
        reason="empty subject"
    elif [ "${#subject}" -gt 72 ]; then
        reason="subject is ${#subject} characters, at most 72 allowed"
    elif printf '%s\n' "$subject" | grep -Eq '^[A-Za-z][A-Za-z0-9_-]*: .*\.$'; then
        reason="prefixed subject ends with a period"
    elif printf '%s\n' "$subject" | grep -Eqw 'WIP|wip'; then
        reason="subject contains WIP"
    elif printf '%s\n' "$subject" | grep -Eq '^fixup!'; then
        reason="subject starts with fixup!"
    else
        files=$(git diff-tree --no-commit-id --name-only -r --root "$sha")
        if printf '%s\n' "$files" | grep -q '^crates/'; then
            body=$(git log -1 --format=%b "$sha" | tr -d '[:space:]')
            [ -n "$body" ] || reason="diff touches crates/ but the commit has no body explaining the design"
        fi
    fi
    if [ -n "$reason" ]; then
        echo "fail $short: $reason"
        rc=1
    else
        echo "ok $short"
    fi
done <"$list"
exit $rc
