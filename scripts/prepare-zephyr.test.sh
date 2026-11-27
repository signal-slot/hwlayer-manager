#!/bin/bash
# Tests for prepare-zephyr.sh: argument handling and --dry-run only. Nothing is
# downloaded, no network is used and nothing is written outside a mktemp directory.
set -u
here=$(cd "$(dirname "$0")" && pwd)
script=$here/prepare-zephyr.sh
doc=$here/../docs/zephyr-workspace.md
tmp=$(mktemp -d "${TMPDIR:-/tmp}/prepzephyr.XXXXXX") || exit 1
trap 'rm -rf "$tmp"' EXIT
fails=0

check() { # name rc (0 = pass)
    if [ "$2" -eq 0 ]; then echo "pass $1"; else echo "FAIL $1"; fails=$((fails + 1)); fi
}

val() { sed -n "s/^$1=//p" "$script" | head -n1; }
tag=$(val ZEPHYR_TAG)
sdk=$(val SDK_VERSION)
west=$(val WEST_VERSION)
[ -n "$tag" ] && [ -n "$sdk" ]; check "script defines ZEPHYR_TAG and SDK_VERSION" $?

# Usage errors exit 2 and create nothing.
dest=$tmp/dest
for args in "" "--bogus DEST" "--dry-run" "--dry-run DEST DEST.b" "--clean-git-config" "-h" "--help"; do
    real=${args//DEST/$dest}
    # shellcheck disable=SC2086
    sh "$script" $real >/dev/null 2>&1
    rc=$?
    [ "$rc" -eq 2 ]; check "usage error exits 2: '$args'" $?
done
[ ! -e "$dest" ] && [ ! -e "$dest.b" ]; check "usage errors create nothing" $?

if [ "$(uname -s)-$(uname -m)" != Linux-x86_64 ]; then
    echo "skip dry-run cases: the script only supports Linux x86_64 hosts"
else
    # Dry run: the destination must not appear, even with a trailing slash.
    out=$tmp/out.txt
    sh "$script" --dry-run "$dest/" >"$out" 2>"$tmp/err.txt"; rc=$?
    [ "$rc" -eq 0 ]; check "--dry-run exits 0" $?
    [ ! -e "$dest" ]; check "--dry-run writes nothing under the destination" $?
    [ "$(ls -A "$tmp" | grep -vc '^\(out\|err\)\.txt$')" -eq 0 ]; check "--dry-run writes nothing else in the temp dir" $?
    grep -q '^dry run: nothing is created' "$out"; check "dry-run banner printed" $?

    grep -q -- "--mr $tag " "$out"; check "steps mention ZEPHYR_TAG $tag in west init" $?
    grep -q "zephyr-sdk-${sdk}_linux" "$out"; check "steps mention SDK_VERSION $sdk in the bundle name" $?
    grep -q "/download/v$sdk/" "$out"; check "steps mention the SDK release v$sdk URL" $?
    grep -q "west==$west" "$out"; check "steps mention west==$west" $?

    # Export lines: both paths under <dest>/zephyr, no double slash from the trailing slash.
    ws=$(sed -n 's/^export ZEPHYR_WORKSPACE=//p' "$out")
    sd=$(sed -n 's/^export ZEPHYR_SDK_INSTALL_DIR=//p' "$out")
    [ "$ws" = "$dest/zephyr/workspace" ]; check "ZEPHYR_WORKSPACE is <dest>/zephyr/workspace" $?
    [ "$sd" = "$dest/zephyr/sdk/zephyr-sdk-$sdk" ]; check "ZEPHYR_SDK_INSTALL_DIR is <dest>/zephyr/sdk/zephyr-sdk-$sdk" $?

    # Every path under the temp dir in the output is under <dest>/zephyr.
    stray=$(grep -o "$tmp/[^ ]*" "$out" | grep -v "^$dest/zephyr" | grep -v "^$tmp/out.txt" | head -n1)
    [ -z "$stray" ]; check "every printed path under the temp dir is under <dest>/zephyr" $?

    # --clean-git-config is accepted and dry-run still writes nothing.
    sh "$script" --dry-run --clean-git-config "$dest" >/dev/null 2>&1; rc=$?
    [ "$rc" -eq 0 ] && [ ! -e "$dest" ]; check "--clean-git-config accepted, nothing written" $?
fi

# The pins in docs/zephyr-workspace.md match the script.
if [ -r "$doc" ]; then
    grep -q "^| Zephyr | \`$tag\`" "$doc"; check "doc pins Zephyr $tag" $?
    grep -q "^| Zephyr SDK | \`$sdk\`" "$doc"; check "doc pins Zephyr SDK $sdk" $?
    grep -q "^| west | \`$west\`" "$doc"; check "doc pins west $west" $?
    for v in SDK_MINIMAL_SHA256 SDK_ARM_SHA256; do
        grep -q "$(val $v)" "$doc"; check "doc lists $v" $?
    done
else
    check "docs/zephyr-workspace.md readable" 1
fi

[ "$fails" -eq 0 ] && echo "all passed" || echo "$fails failed"
exit $((fails != 0))
