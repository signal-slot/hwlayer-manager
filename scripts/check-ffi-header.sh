#!/usr/bin/env bash
# Check that the C header moves with composition-ffi in the same commit.
#
# Usage: scripts/check-ffi-header.sh [BASE_REF]     (default: origin/main,
#        falling back to main when origin/main does not exist)
#
# For every non-merge commit in BASE_REF..HEAD that changes
# crates/composition-ffi/src/lib.rs, compare the "ABI surface" of the file
# before and after the commit. If the surface differs, the commit must also
# change crates/composition-ffi/include/composition_engine.h.
#
# Heuristic. The ABI surface is the normalised text of:
#   - every item introduced by #[no_mangle] / #[unsafe(no_mangle)] or
#     containing extern "C": from the attribute or first line up to and
#     including the line holding the opening brace or the terminating ';'
#     (so signature changes spread over several lines are seen, bodies are not);
#   - every item introduced by #[repr(C...)]: the attribute, derives and the
#     whole struct/enum/union definition up to its closing brace at column 0
#     (or its ';').
# Comment-only lines (//, ///, //!) are dropped and whitespace is collapsed, so
# edits to comments, function bodies, formatting or unrelated Rust items pass.
# Known limits: any edit inside a repr(C) type counts as a surface change;
# a repr(C) item whose closing brace is indented is read up to the next
# statement-ending ';'. This guards against forgetting the header; it does not
# prove the header is right.
#
# Output: "ok <sha>" or "fail <sha>: <reason>" per commit; exit 1 on any fail,
# 2 when the base ref cannot be resolved.
set -u

LIB=crates/composition-ffi/src/lib.rs
HDR=crates/composition-ffi/include/composition_engine.h
base=${1:-origin/main}

if ! git rev-parse --verify --quiet "$base^{commit}" >/dev/null; then
    if [ $# -eq 0 ] && git rev-parse --verify --quiet "main^{commit}" >/dev/null; then
        base=main
    else
        echo "error: cannot resolve base ref '$base'" >&2
        exit 2
    fi
fi

# surface <rev>: print the ABI surface of lib.rs at <rev> (empty if absent)
surface() {
    git show "$1:$LIB" 2>/dev/null | awk '
        function flush() { if (buf != "") print buf; buf = ""; mode = "" }
        {
            line = $0
            if (line ~ /^[ \t]*\/\//) next
            gsub(/[ \t]+/, " ", line)
            if (mode == "") {
                if (line ~ /no_mangle/ || line ~ /extern "C"/) mode = "fn"
                else if (line ~ /#\[repr\(C/) mode = "ty"
                else next
            }
            buf = buf line "\n"
            attr = (line ~ /^ *#\[/)
            if (mode == "fn" && !attr && (line ~ /[{] *$/ || line ~ /; *$/)) flush()
            else if (mode == "ty" && !attr && (line ~ /^[}]/ || (line ~ /; *$/ && line !~ /^ *(pub )?[a-z_0-9]+: /))) flush()
        }
        END { flush() }'
}

rc=0
while read -r sha; do
    [ -n "$sha" ] || continue
    short=$(git rev-parse --short "$sha")
    files=$(git diff-tree --no-commit-id --name-only -r --root "$sha")
    if ! printf '%s\n' "$files" | grep -qx "$LIB"; then
        echo "ok $short"
        continue
    fi
    if [ "$(surface "$sha^")" = "$(surface "$sha")" ]; then
        echo "ok $short"
    elif printf '%s\n' "$files" | grep -qx "$HDR"; then
        echo "ok $short"
    else
        echo "fail $short: $LIB changes an extern \"C\", no_mangle or repr(C) item but $HDR is not changed in the same commit"
        rc=1
    fi
done < <(git rev-list --no-merges --reverse "$base..HEAD")
exit $rc
