#!/bin/sh
# Regenerate the host-run AM62P console and diff it against the fixture
# (header line ignored); check am62p-scenes.py summary accepts it.
# Skips with exit 0 when cargo is missing.
here=$(cd "$(dirname "$0")" && pwd) || exit 2
fix=$here/testdata/am62p-console/host-run.txt
command -v cargo >/dev/null 2>&1 || { echo "skip: cargo missing"; exit 0; }
tmp=$(mktemp -d "${TMPDIR:-/tmp}/am62p-console.XXXXXX") || exit 2
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
cd "$here/.." || exit 2
cargo test -q -p composition-twoplane board_scenes -- --nocapture 2>/dev/null |
    grep -E '^(SCENE|RESULT) ' >"$tmp/new" || { echo "FAIL: no console from cargo"; exit 1; }
tail -n +2 "$fix" >"$tmp/old"
diff -u "$tmp/old" "$tmp/new" || { echo "FAIL: fixture differs from a fresh host run"; exit 1; }
python3 "$here/am62p-scenes.py" summary "$fix" >/dev/null ||
    { echo "FAIL: am62p-scenes.py summary"; exit 1; }
echo "ok: am62p-host-console"
