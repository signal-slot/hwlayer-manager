#!/bin/sh
# Self-test for kms-dump-diff.py. Run: bash scripts/kms-dump-diff.test.sh

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/kms-dump-diff.py
fx=$here/testdata/kms-dump-diff
root=$(mktemp -d "${TMPDIR:-.}/kdd-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
py() { python3 -I "$tool" "$@"; }

# expect NAME RC FILE-WITH-PATTERNS ARGS...: run, check exit code and that each
# line of the pattern file occurs as a fixed string in stdout.
expect() {
    name=$1; rc_want=$2; pats=$3; shift 3
    py "$@" > "$root/out" 2> "$root/err"
    rc=$?
    [ "$rc" -eq "$rc_want" ] || { bad "$name: exit $rc, want $rc_want"; return; }
    while IFS= read -r p; do
        [ -n "$p" ] || continue
        grep -qF -- "$p" "$root/out" || { bad "$name: missing '$p'"; cat "$root/out"; return; }
    done < "$pats"
    ok "$name"
}

cat > "$root/p1" <<'T'
plane 39 removed
plane 51 added
plane 33: type primary -> overlay
plane 33: scaling property SCALING_FILTER removed
plane 33: rotation: mutable atomic -> mutable atomic enum [rotate-0=1, rotate-90=2]
plane 33: format AR24:0x0 removed
plane 33: format NV12:0x0 added
plane 45: outputs 0b1 -> 0b11
plane 45: zpos range (property) 0..=7 -> 0..=3
connector 160 added
crtc 88 removed
unparsed: 1
T
expect probe-diff 1 "$root/p1" "$fx/probe-old.txt" "$fx/probe-new.txt"

cat > "$root/p2" <<'T'
plane 33: possible_crtcs 0b1 [0] -> 0b11 [0,1]
plane 33: zpos range (property) 0..=7 -> 0..=3
plane 33: scaling property scaling filter added
plane 33: scaling property SCALING_FILTER: enum {Default=0, Nearest Neighbor=1} [atomic] -> enum {Default=0} [atomic]
plane 33: format AR24:0x0 removed
crtc 88 added
T
expect props-diff 1 "$root/p2" "$fx/props-old.txt" "$fx/props-new.txt"

cat > "$root/p3" <<'T'
# kms-dump-diff (props)
- plane 39 removed
- crtc 88 added
T
expect markdown 1 "$root/p3" --markdown "$fx/props-old.txt" "$fx/props-new.txt"

# Sorted output: removed planes come before added planes, plane details by id.
py "$fx/props-old.txt" "$fx/props-new.txt" > "$root/o1"
py "$fx/props-old.txt" "$fx/props-new.txt" > "$root/o2"
if cmp -s "$root/o1" "$root/o2" && [ "$(sed -n 1p "$root/o1")" = "plane 39 removed" ]; then
    ok "stable sorted order"
else bad "order"; fi

: > "$root/none"
expect identical-probe 0 "$root/none" "$fx/probe-old.txt" "$fx/probe-old.txt"
expect identical-props 0 "$root/none" "$fx/props-old.txt" "$fx/props-old.txt"

# Usage errors
for args in "" "$fx/probe-old.txt" "--bogus a b" "$fx/probe-old.txt $fx/props-old.txt" "$fx/probe-old.txt $root/missing" "--kind"; do
    # shellcheck disable=SC2086
    py $args > /dev/null 2>&1
    rc=$?
    [ "$rc" -eq 2 ] && ok "usage error exit 2 ($args)" || bad "usage exit $rc for '$args'"
done

[ $fail -eq 0 ] && echo "all ok"
exit $fail
