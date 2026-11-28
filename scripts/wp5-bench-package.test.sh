#!/bin/sh
# Test for wp5-bench-package.sh. Dry run always; one real run if cargo exists.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
S=$here/wp5-bench-package.sh
: "${TMPDIR:?TMPDIR must be set}"
t=$(mktemp -d "$TMPDIR/wp5-bench-test.XXXXXX")
trap 'rm -rf "$t"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }

sh "$S" >/dev/null 2>&1 && fail "no args accepted"
sh "$S" --bogus "$t/o" >/dev/null 2>&1 && fail "bad option accepted"
sh "$S" --dry-run --commit nonexistent-rev "$t/o" >/dev/null 2>&1 && fail "bad commit accepted"
TMPDIR=/tmp sh "$S" --dry-run "$t/o" >/dev/null 2>&1 && fail "/tmp TMPDIR accepted"

check_manifest() {
	m=$1/manifest.txt
	[ -f "$m" ] || fail "no manifest in $1"
	for k in label commit date rustc cargo cpu dry_run examples; do
		grep -q "^$k:" "$m" || fail "manifest lacks $k"
	done
	grep -q 'host only, not a hardware result' "$m" || fail "manifest lacks label"
	grep -Eq '^commit: [0-9a-f]{40}$' "$m" || fail "bad commit"
	hn=$(hostname 2>/dev/null || true)
	if [ -n "$hn" ] && grep -qiF -- "$hn" "$1"/*; then fail "host name in output"; fi
	if grep -Eq '[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+' "$m"; then fail "IP-like text in manifest"; fi
	return 0
}

sh "$S" --dry-run "$t/dry" >/dev/null
check_manifest "$t/dry"
grep -q '^dry_run: 1' "$t/dry/manifest.txt" || fail "dry_run flag"
grep -q '^examples: none' "$t/dry/manifest.txt" || fail "dry run ran examples"

if command -v cargo >/dev/null 2>&1; then
	sh "$S" "$t/real" >/dev/null
	check_manifest "$t/real"
	[ -s "$t/real/plan_time.txt" ] || fail "no plan_time output"
	for f in "$t"/real/*.txt; do
		grep -q 'host only, not a hardware result' "$f" || fail "$f unlabelled"
	done
else
	echo "cargo not available: real run skipped"
fi
echo "PASS"
