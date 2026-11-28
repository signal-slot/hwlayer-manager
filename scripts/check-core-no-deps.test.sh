#!/bin/sh
# Self-test of check-core-no-deps.sh with fake manifests under $TMPDIR.
# Each manifest is checked with python3 (tomllib) and with the awk fallback.
set -u
here=$(cd "$(dirname "$0")" && pwd)
chk=$here/check-core-no-deps.sh
tmp=$(mktemp -d "${TMPDIR:-.}/cnd.XXXXXX") || exit 2
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
rc=0
bad() { echo "FAIL: $*" >&2; rc=1; }

# PATH without python3, for the awk fallback
mkdir "$tmp/nopy"
for t in sh awk grep sed cat; do
    p=$(command -v "$t") && ln -s "$p" "$tmp/nopy/$t"
done

# run NAME EXPECT(0|1) BODY
run() {
    printf '[package]\nname = "x"\nversion = "0.1.0"\n\n%s\n' "$3" >"$tmp/$1.toml"
    sh "$chk" "$tmp/$1.toml" >/dev/null 2>&1
    [ $? -eq "$2" ] || bad "$1: python: expected exit $2"
    PATH=$tmp/nopy sh "$chk" "$tmp/$1.toml" >/dev/null 2>&1
    [ $? -eq "$2" ] || bad "$1: awk: expected exit $2"
}
nl='
'
run empty 0 ''
run lints 0 "[lints]${nl}workspace = true"
run dev 0 "[dev-dependencies]${nl}serde = \"1\""
run dep 1 "[dependencies]${nl}serde = \"1\""
run deptable 1 "[dependencies.serde]${nl}version = \"1\""
run build 1 "[build-dependencies]${nl}cc = \"1\""
run target 1 "[target.'cfg(unix)'.dependencies]${nl}libc = \"0.2\""
run emptydeps 0 "[dependencies]${nl}# nothing"
run ws 1 "[dependencies]${nl}serde.workspace = true"

out=$(sh "$chk" "$tmp/dev.toml" 2>&1)
case $out in *"dev-dependencies.serde"*) ;; *) bad "dev deps not reported" ;; esac
sh "$chk" "$tmp/none.toml" >/dev/null 2>&1
[ $? -eq 2 ] || bad "missing manifest must exit 2"

[ $rc -eq 0 ] && echo "ok: check-core-no-deps tests"
exit $rc
