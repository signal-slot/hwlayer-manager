#!/usr/bin/env bash
# Host test: `m2scene --dry-run` output through trace-to-metrics.py.
#
# Documented behaviour (docs/m2-linux-scene.md): the dry run opens no device
# and prints the RUN line, scene and plan only. It emits no CETRACE lines and
# no '# date:' comment, and trace-to-metrics.py takes a file (no stdin) and
# rejects a log with no `ev=hdr` line (exit 2). So the dry run cannot be
# converted. This test pins that exact state. If the example starts emitting
# trace events, the first branch below checks the conversion for a RUN line
# and at least one METRIC line instead.
# Nothing here is a measurement. Written 2026-11-28.
set -u
cd "$(dirname "$0")/.." || exit 1
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
tmp=$(mktemp -d "${TMPDIR:-/tmp}/m2st.XXXXXX") || exit 1
trap 'rm -rf "$tmp"' EXIT
T="python3 -I scripts/trace-to-metrics.py"

if cargo run -q -p composition-drm --example m2scene -- --dry-run >"$tmp/dry.log" 2>"$tmp/dry.err"; then
	ok "m2scene --dry-run exits 0"
else
	bad "m2scene --dry-run failed"
fi
grep -q '^RUN .* side=linux stage=B .* mode=dry-run' "$tmp/dry.log" && ok "dry-run RUN line" || bad "dry-run RUN line"
grep -q '^VERDICT m2scene: DONE' "$tmp/dry.log" && ok "dry-run verdict DONE" || bad "dry-run verdict"

# The converter reads a file argument; add the date comment real logs carry.
(echo "# date: $(date -u +%F)"; cat "$tmp/dry.log") >"$tmp/dated.log"

if grep -q '^CETRACE ' "$tmp/dry.log"; then
	if $T "$tmp/dated.log" >"$tmp/out.txt" 2>"$tmp/out.err"; then ok "conversion succeeds"; else bad "conversion failed: $(cat "$tmp/out.err")"; fi
	grep -q '^RUN ' "$tmp/out.txt" && ok "RUN line" || bad "no RUN line"
	grep -q '^METRIC ' "$tmp/out.txt" && ok "METRIC line" || bad "no METRIC line"
else
	ok "dry-run emits no CETRACE events (documented)"
	# Mismatch pinned: no hdr line, so the converter rejects the log.
	$T "$tmp/dated.log" >"$tmp/out.txt" 2>"$tmp/out.err"
	rc=$?
	[ "$rc" -eq 2 ] && ok "converter exits 2 on an event-less log" || bad "converter exit $rc, expected 2"
	grep -q "no CETRACE ev=hdr line" "$tmp/out.err" && ok "converter says: no CETRACE ev=hdr line" || bad "converter message: $(cat "$tmp/out.err")"
	[ ! -s "$tmp/out.txt" ] && ok "no RUN/METRIC output" || bad "unexpected converter output"
fi

if [ "$fail" -eq 0 ]; then echo PASS; else echo FAILED; fi
exit "$fail"
