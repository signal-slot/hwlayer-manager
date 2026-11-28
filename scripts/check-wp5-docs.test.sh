#!/bin/bash
# Tests for check-wp5-docs.sh using fake repositories in a temporary directory.
set -u
here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-wp5-docs.sh
tmp=$(mktemp -d "${TMPDIR:-$here}/wp5docs.XXXXXX") || exit 1
trap 'rm -rf "$tmp"' EXIT
fails=0

expect() { # name want_rc pattern root
    local out rc
    out=$(sh "$script" "$4" 2>&1); rc=$?
    if [ "$rc" -ne "$2" ] || ! printf '%s\n' "$out" | grep -qE -- "$3"; then
        echo "FAIL $1 (rc=$rc): $out"
        fails=$((fails + 1))
    else
        echo "pass $1"
    fi
}

mk() { # dir proposal-body demo-body (empty arg = file absent)
    mkdir -p "$1/docs"
    [ -n "$2" ] && printf '%s\n' "$2" > "$1/docs/wp5-proposal.md"
    [ -n "$3" ] && printf '%s\n' "$3" > "$1/docs/wp5-demo.md"
    return 0
}

good_p='# Proposal
Date: 2026-11-28
Offer is read from source. Acceptance is not guaranteed.'
good_d='# Demo
Date: 2026-11-28
## Walk-through
Result: not measured.'

mk "$tmp/pass" "$good_p" "$good_d"
expect pass 0 '^ok$' "$tmp/pass"

mk "$tmp/skipboth" "" ""
expect skip-both 0 'skip docs/wp5-proposal.md' "$tmp/skipboth"
out=$(sh "$script" "$tmp/skipboth")
case $out in *"skip docs/wp5-demo.md"*) ;; *) echo "FAIL skip demo"; fails=$((fails + 1)) ;; esac
mk "$tmp/skipone" "$good_p" ""
expect skip-one 0 'skip docs/wp5-demo.md' "$tmp/skipone"

mk "$tmp/nodate" '# Proposal
text
inferred' ""
expect no-date 1 'wp5-proposal.md: no date line' "$tmp/nodate"
mk "$tmp/latedate" '# P
1
2
3
4
5
6
7
8
9
10
2026-11-28 inferred' ""
expect date-late 1 'no date line' "$tmp/latedate"

mk "$tmp/notag" '# Proposal
2026-11-28
plain text' ""
expect no-tag 1 'no evidence tag' "$tmp/notag"
mk "$tmp/obsundated" '# Proposal
2026-11-28
observed without a date' ""
expect observed-needs-date 1 'no evidence tag' "$tmp/obsundated"
mk "$tmp/obsdated" '# Proposal
2026-11-28
observed 2026-11-27 on the host' ""
expect observed-dated 0 '^ok$' "$tmp/obsdated"

mk "$tmp/promise" '# Proposal
2026-11-28
inferred
The patch will be accepted by upstream.' ""
expect promise 1 'wp5-proposal.md:4: promises upstream acceptance' "$tmp/promise"
mk "$tmp/promise2" '# Proposal
2026-11-28
inferred
Merge is Guaranteed.
Already accepted upstream.' ""
expect promise-two 1 'proposal.md:5: promises' "$tmp/promise2"
mk "$tmp/negated" '# Proposal
2026-11-28
inferred
This is not guaranteed and no one says it will be accepted.' ""
expect promise-negated 0 '^ok$' "$tmp/negated"
# the acceptance check applies to the proposal only
mk "$tmp/demoguar" "$good_p" '# Demo
2026-11-28
## Walk-through
host only, guaranteed to run.'
expect demo-not-checked 0 '^ok$' "$tmp/demoguar"

mk "$tmp/nowalk" "" '# Demo
2026-11-28
host only
## Steps'
expect no-walkthrough 1 "wp5-demo.md: no 'Walk-through' heading" "$tmp/nowalk"
mk "$tmp/walkh3" "" '# Demo
2026-11-28
host only
### Walk-through (2026-11-28)'
expect walkthrough-h3 0 '^ok$' "$tmp/walkh3"

expect no-root 2 'not found' "$tmp/does-not-exist"

if [ "$fails" -ne 0 ]; then
    echo "$fails failure(s)"
    exit 1
fi
echo "all passed"
