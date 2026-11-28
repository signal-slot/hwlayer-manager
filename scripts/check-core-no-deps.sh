#!/bin/sh
# Project rule: composition-core has no dependencies.
# Usage: check-core-no-deps.sh [path/to/Cargo.toml]
# Fails if [dependencies], [build-dependencies] or a [target.*.dependencies] /
# [target.*.build-dependencies] table lists anything. [dev-dependencies] are
# reported but allowed. Uses python3 -I with tomllib, else an awk fallback.
# cargo metadata --no-deps is cross-checked only for the repository manifest.
set -u
here=$(cd "$(dirname "$0")" && pwd)
default=$here/../crates/composition-core/Cargo.toml
man=${1:-$default}
[ -f "$man" ] || { echo "check-core-no-deps: no manifest: $man" >&2; exit 2; }

py='
import sys, tomllib
with open(sys.argv[1], "rb") as f:
    d = tomllib.load(f)
bad, dev = [], []
for k in ("dependencies", "build-dependencies"):
    bad += [k + "." + n for n in d.get(k, {})]
dev += ["dev-dependencies." + n for n in d.get("dev-dependencies", {})]
for t, tv in d.get("target", {}).items():
    for k in ("dependencies", "build-dependencies"):
        bad += ["target." + t + "." + k + "." + n for n in tv.get(k, {})]
    dev += ["target." + t + ".dev-dependencies." + n for n in tv.get("dev-dependencies", {})]
for x in dev: print("note: dev " + x)
for x in bad: print("DEP " + x)
'
awkprog='
function key(l) { sub(/^[ \t]+/, "", l); return l }
/^[ \t]*\[/ {
  h = key($0); sub(/[ \t]*(#.*)?$/, "", h); gsub(/^\[+|\]+$/, "", h); sec = ""
  if (h ~ /(^|\.)(build-)?dependencies$/ && h !~ /^(package|workspace)/) sec = "b"
  else if (h ~ /(^|\.)(build-)?dependencies\./) { print "DEP " h; next }
  else if (h ~ /(^|\.)dev-dependencies(\.|$)/) { print "note: dev " h; sec = "d" }
  next
}
{ l = key($0); if (l == "" || l ~ /^#/) next
  if (sec == "b" && l ~ /=/) { n = l; sub(/[ \t]*=.*/, "", n); print "DEP " n }
  else if (sec == "d" && l ~ /=/) { n = l; sub(/[ \t]*=.*/, "", n); print "note: dev " n }
  else if (l ~ /^(build-)?dependencies[ \t]*[.=]/) print "DEP " l
}'

if command -v python3 >/dev/null 2>&1 && python3 -I -c 'import tomllib' 2>/dev/null; then
    out=$(python3 -I -c "$py" "$man") || { echo "check-core-no-deps: cannot parse $man" >&2; exit 2; }
else
    out=$(awk "$awkprog" "$man") || exit 2
fi

printf '%s\n' "$out" | grep '^note:' >&2
deps=$(printf '%s\n' "$out" | grep '^DEP ')
rc=0
if [ -n "$deps" ]; then
    printf '%s\n' "$deps" | sed 's/^DEP /FAIL: composition-core lists a dependency: /' >&2
    rc=1
fi

# Cross-check with cargo for the repository's own manifest.
if [ -z "${1:-}" ] && command -v cargo >/dev/null 2>&1 && command -v python3 >/dev/null 2>&1; then
    md=$(cargo metadata --no-deps --format-version 1 --manifest-path "$man" 2>/dev/null)
    if [ -n "$md" ]; then
        cd=$(printf '%s' "$md" | python3 -I -c '
import json, sys
m = json.load(sys.stdin)
for p in m["packages"]:
    if p["name"] == "composition-core":
        for d in p["dependencies"]:
            if d.get("kind") != "dev":
                print(d["name"])
')
        if [ -n "$cd" ]; then
            echo "FAIL: cargo metadata lists dependencies: $cd" >&2
            rc=1
        fi
    fi
fi
[ $rc -eq 0 ] && echo "ok: composition-core has no dependencies"
exit $rc
