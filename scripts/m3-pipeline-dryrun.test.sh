#!/bin/sh
# Host dry run of the M3 report pipeline. No board, no hardware claim.
# Run: sh scripts/m3-pipeline-dryrun.test.sh
#
# host-run.log -> am62p-scenes.py summary -> a report built on the structure
# of docs/m3-am62p-report-template.md (placeholder facts, the fixture photo
# for all 8 scenes) -> check-m3-report.sh, which must accept it.
# The report is a host dry run, lives in a temporary directory ($TMPDIR, or
# the ignored target/ dir of the repo) and is removed on exit; never commit it.
# A disagreement between a tool's output and the checker fails the test and
# is printed as a MISMATCH.

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/.." && pwd)
log=$here/testdata/am62p-scenes/host-run.log
photo=$here/testdata/m3-report/scene1.png
template=$repo/docs/m3-am62p-report-template.md

base=${TMPDIR:-$repo/target}
mkdir -p "$base" || exit 2
work=$(mktemp -d "$base/m3-pipeline-dryrun.XXXXXX") || exit 2
trap 'rm -rf "$work"' EXIT HUP INT TERM

die() {
    echo "FAIL: $1" >&2
    exit 1
}

for f in "$log" "$photo" "$template"; do
    [ -f "$f" ] || die "missing input $f"
done

# 1. summary of the host console log
python3 -I "$here/am62p-scenes.py" summary "$log" >"$work/summary.md" ||
    die "am62p-scenes.py summary failed on the host log"
grep -q '^RESULT PASS' "$work/summary.md" || die "summary has no RESULT PASS"
echo "ok: summary"

# 2. the report: the template's sections, with dry-run content
cp "$photo" "$work/scene1.png"
rep=$work/report.md
fence='```'
{
    cat <<EOF
# M3 AM62P report: HOST DRY RUN

**This is a host dry run of the report pipeline, not a board run.
It holds no hardware observation and is never committed.**
Structure: docs/m3-am62p-report-template.md.

## Date

Date: 2000-01-01 (dry run)

## Board

Board: placeholder board (dry run)

## BSP

BSP: placeholder BSP (dry run)

## Kernel

Kernel: placeholder kernel (dry run)

## Panel

Panel: placeholder panel (dry run)

## Console (host log)

$fence
EOF
    grep -E '^(SCENE [1-8] |RESULT )' "$log"
    cat <<EOF
$fence

## Summary of the console

EOF
    cat "$work/summary.md"
    cat <<EOF

## Photographs and camera-check

The fixture image is reused for every scene (dry run, no camera).

EOF
    n=1
    while [ "$n" -le 8 ]; do
        printf '![scene %s](scene1.png)\n\n' "$n"
        printf -- '- Scene %s: camera-check SKIP (host dry run, fixture image)\n\n' "$n"
        n=$((n + 1))
    done
    cat <<EOF
## Restore and Torizon confirmation

Date: 2000-01-01 (dry run)

- Nothing was stopped: host dry run. Torizon not touched.
EOF
} >"$rep"
echo "ok: report built ($(wc -l <"$rep") lines)"

# 3. the structural check must accept it
if out=$(sh "$here/check-m3-report.sh" "$rep" 2>&1); then
    echo "ok: check-m3-report accepts the dry-run report"
else
    echo "MISMATCH: check-m3-report.sh rejects a report built from the tools' output:" >&2
    printf '%s\n' "$out" | sed 's/^/  /' >&2
    die "tool output and checker disagree"
fi

# 4. the checker must still notice damage (a missing scene line)
grep -v '^SCENE 5 ' "$rep" >"$work/bad.md"
if sh "$here/check-m3-report.sh" "$work/bad.md" >/dev/null 2>&1; then
    die "checker accepted a report without SCENE 5"
fi
echo "ok: checker rejects a report without SCENE 5"
echo "m3-pipeline-dryrun: all ok"
