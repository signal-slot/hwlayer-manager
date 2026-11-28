#!/bin/sh
# Compile the M3-03 board scenes 9 to 12 (tests/board/am62p_r5f_m3_03) for
# the host, run them behind the fake panel of tests/board/am62p_r5f/host,
# and expect RESULT PASS.
#
# Usage: scripts/check-am62p-m3-03-c.sh [OUT_DIR]   (default: target/am62p-m3-03-c)
#
# Host only: no Zephyr, no board, no showcase checkout. A host PASS checks
# the scenes' sequence and judgement against a fake driver; it is not a
# board result. Not part of CI or check-all.sh.
#
# Builds the engine with `cargo build -p composition-ffi-staticlib` (the
# path of libcomposition_engine.a is read from cargo's JSON messages, so
# CARGO_TARGET_DIR is honoured), then compiles with $CC (default cc),
# -std=c11 -Wall -Wextra -Werror -pedantic:
#   tests/board/am62p_r5f_m3_03/host/m3_03_host.c  (includes fake_panel.c)
#   tests/board/am62p_r5f_m3_03/src/m3_03_scenes.c
#   tests/board/am62p_r5f/src/scenes.c             (fake_panel.c needs it)
#   backends/twoplane/ce_twoplane.c
# A scene the fake panel cannot model prints "SCENE N skipped (host): ...".
# Without the C compiler or cargo it prints "skipped" and exits 0.
# Exit status: 0 = RESULT PASS (or skipped), 1 = anything else.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
out=${1:-$root/target/am62p-m3-03-c}
cc=${CC:-cc}
app=$root/tests/board/am62p_r5f_m3_03
base=$root/tests/board/am62p_r5f
tp=$root/backends/twoplane

skip() { echo "skipped: $1"; exit 0; }
fail() { echo "fail: $1" >&2; exit 1; }

command -v "$cc" >/dev/null 2>&1 || skip "no C compiler ($cc is not on PATH)"
command -v cargo >/dev/null 2>&1 || skip "cargo is not on PATH"

mkdir -p "$out"
(cd "$root" && cargo build --locked -p composition-ffi-staticlib \
	--message-format=json-render-diagnostics) >"$out/cargo.json" ||
	fail "cargo build -p composition-ffi-staticlib"
lib=$(grep '"reason":"compiler-artifact"' "$out/cargo.json" |
	grep -o '"[^"]*libcomposition_engine\.a"' | tail -n 1 | tr -d '"')
[ -n "$lib" ] && [ -f "$lib" ] || fail "cargo reported no libcomposition_engine.a"

"$cc" -std=c11 -Wall -Wextra -Werror -pedantic \
	-I"$root/crates/composition-ffi/include" -I"$tp" -I"$base/src" -I"$base/host" \
	-I"$app/src" \
	"$app/host/m3_03_host.c" "$app/src/m3_03_scenes.c" "$base/src/scenes.c" \
	"$tp/ce_twoplane.c" "$lib" -lpthread -ldl -lm -o "$out/m3_03_host" ||
	fail "the scenes do not compile for the host"
echo "ok: scenes 9-12 compile for the host ($cc)"

rc=0
"$out/m3_03_host" >"$out/console.txt" 2>&1 || rc=$?
cat "$out/console.txt"
if [ "$rc" -ne 0 ]; then
	fail "the host run exited with $rc"
fi
grep -q '^RESULT PASS' "$out/console.txt" || fail "no RESULT PASS line"
if grep -q 'FAIL' "$out/console.txt"; then
	fail "a FAIL line in the console"
fi
echo "ok: RESULT PASS on the host (not a board result)"
