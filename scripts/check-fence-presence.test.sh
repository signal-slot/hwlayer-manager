#!/bin/sh
# Self-test for check-fence-presence.sh.
# Run: sh scripts/check-fence-presence.test.sh
# Copies the shipped table to a temp directory under the repository, breaks it
# in several ways and expects the check to notice.

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
script=$here/check-fence-presence.sh
doc=$root/docs/fences/property-presence.md
tmp=$(mktemp -d "$root/fp-test.XXXXXX") || exit 2
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
fail=0

expect() { # name want-rc doc
    sh "$script" "$3" > "$tmp/out" 2>&1
    rc=$?
    if [ "$rc" -ne "$2" ]; then
        echo "FAIL $1: rc $rc, want $2"; cat "$tmp/out"; fail=1
    else
        echo "ok   $1"
    fi
}

expect shipped-table 0 "$doc"

# A plane count that disagrees with the data.
sed 's#| 2/2: 32 35 | 2/2: 34 37 | observed 2026-11-28 (recorded probe) |$#| 1/2: 32 | 2/2: 34 37 | observed 2026-11-28 (recorded probe) |#' \
    "$doc" > "$tmp/planes.md"
expect wrong-plane-count 1 "$tmp/planes.md"
sh "$script" "$tmp/planes.md" 2>&1 | grep -q 'planes: table' || { echo "FAIL wrong-plane-count message"; fail=1; }

# A CRTC id list that disagrees with the data.
sed 's#| 2/2: 81 82 |#| 2/2: 81 83 |#' "$doc" > "$tmp/crtc.md"
expect wrong-crtc-id 1 "$tmp/crtc.md"

# A date that is in neither the file nor its README.
sed 's#| 2026-11-28 | 7/7#| 2026-01-01 | 7/7#' "$doc" > "$tmp/date.md"
expect wrong-date 1 "$tmp/date.md"

# A recorded file with no row.
grep -v 'imx95-probe-2026-11-28.txt' "$doc" > "$tmp/missing.md"
expect missing-row 1 "$tmp/missing.md"

# A row for a file that does not exist.
{ cat "$doc"; echo '| X | Y | `docs/observed/nope.txt` | 2026-11-28 | 0/0 | 0/0 | t |'; } > "$tmp/nofile.md"
expect nonexistent-file 1 "$tmp/nofile.md"

# A row for a recorded file that lists no planes or CRTCs.
{ cat "$doc"; echo '| X | Y | `docs/observed/imx8mp-x2-2026-11-28/probe-card0.txt` | 2026-11-28 | 0/0 | 0/0 | t |'; } > "$tmp/nolist.md"
expect row-without-listing 1 "$tmp/nolist.md"

expect missing-doc 2 "$tmp/absent.md"

[ $fail -eq 0 ] && echo "all passed"
exit $fail
