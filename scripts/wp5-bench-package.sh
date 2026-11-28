#!/bin/sh
# Host benchmark package from a clean checkout (WP5). Host only, not a
# hardware result. See scripts/wp5-bench-package-README.md.
set -eu

LABEL='host only, not a hardware result'
usage() {
	echo "usage: sh scripts/wp5-bench-package.sh [--commit REV] [--dry-run] OUTDIR" >&2
}

commit=HEAD
dry=0
out=
while [ $# -gt 0 ]; do
	case $1 in
	--commit) [ $# -ge 2 ] || { usage; exit 2; }; commit=$2; shift 2 ;;
	--dry-run) dry=1; shift ;;
	-h|--help) usage; exit 0 ;;
	-*) echo "unknown option: $1" >&2; usage; exit 2 ;;
	*) [ -z "$out" ] || { usage; exit 2; }; out=$1; shift ;;
	esac
done
[ -n "$out" ] || { usage; exit 2; }

: "${TMPDIR:?TMPDIR must be set (a directory on disk, not /tmp)}"
case $TMPDIR in
/tmp|/tmp/*|/var/tmp|/var/tmp/*) echo "TMPDIR must not be under /tmp or /var/tmp" >&2; exit 2 ;;
esac

repo=$(cd "$(dirname "$0")/.." && git rev-parse --show-toplevel)
rev=$(git -C "$repo" rev-parse --verify "$commit^{commit}") || {
	echo "unknown commit: $commit" >&2; exit 2; }

mkdir -p "$out"
scratch=$(mktemp -d "$TMPDIR/wp5-bench.XXXXXX")
trap 'rm -rf "$scratch"' EXIT INT TERM
git clone -q --no-hardlinks "$repo" "$scratch/src"
git -C "$scratch/src" checkout -q "$rev"

cpu=$(sed -n 's/^model name[[:space:]]*:[[:space:]]*//p' /proc/cpuinfo 2>/dev/null | head -n 1)
[ -n "$cpu" ] || cpu=unknown
date_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)
if command -v rustc >/dev/null 2>&1; then rustc_v=$(rustc -V); else rustc_v='not available'; fi
if command -v cargo >/dev/null 2>&1; then cargo_v=$(cargo -V); else cargo_v='not available'; fi

ran=
if [ "$dry" -eq 0 ]; then
	command -v cargo >/dev/null 2>&1 || { echo "cargo not found (use --dry-run)" >&2; exit 1; }
	export CARGO_TARGET_DIR="$scratch/target"
	(cd "$scratch/src" && cargo fetch --locked)
	for ex in plan_bench plan_time advise_bench; do
		[ -f "$scratch/src/crates/composition-fake/examples/$ex.rs" ] || continue
		{
			echo "# $LABEL"
			echo "# example: composition-fake/$ex (release), commit $rev, $date_utc"
			(cd "$scratch/src" && cargo run --release --locked -q -p composition-fake --example "$ex")
		} >"$out/$ex.txt"
		ran="$ran $ex"
	done
fi
[ -n "$ran" ] || ran=' none'

{
	echo "# Benchmark package manifest ($LABEL)"
	echo "label: $LABEL"
	echo "commit: $rev"
	echo "date: $date_utc"
	echo "rustc: $rustc_v"
	echo "cargo: $cargo_v"
	echo "cpu: $cpu"
	echo "dry_run: $dry"
	echo "examples:$ran"
} >"$out/manifest.txt"
echo "wrote $out/manifest.txt ($LABEL)"
