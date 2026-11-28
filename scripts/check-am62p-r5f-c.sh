#!/usr/bin/env bash
# Compile the C of the AM62P R5F board test for armv7r-none-eabihf and check
# the objects' symbols.
#
# Usage: scripts/check-am62p-r5f-c.sh [OUT_DIR]   (default: target/am62p-r5f-c)
#
# Host only: nothing is linked or run. Not part of CI or check-all.sh.
#
# Compiles tests/board/am62p_r5f/src/scenes.c and backends/twoplane/ce_twoplane.c
# with clang --target=armv7r-none-eabihf -mcpu=cortex-r5 -mfpu=vfpv3-d16
# -mfloat-abi=hard -std=c11 -Wall -Wextra -Werror -pedantic -ffreestanding
# -nostdinc -O2 and clang's own headers. Then:
#   - the undefined symbols of both objects must be ce_* (engine or backend)
#     or __aeabi_memclr4, memcpy, memset; anything else fails;
#   - when rustup has armv7r-none-eabihf, libcomposition_engine.a is built for
#     it and must define every symbol the two objects leave undefined between
#     them (the ce_* ones and the runtime ones they use).
# Without clang, an Arm-capable clang or an nm, it prints "skipped" and exits 0.
# Without the rust target the library check is skipped, the rest still runs.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
out=${1:-$root/target/am62p-r5f-c}
rt=armv7r-none-eabihf
app=$root/tests/board/am62p_r5f/src
tp=$root/backends/twoplane

skip() { echo "skipped: $1"; exit 0; }

command -v clang >/dev/null 2>&1 || skip "clang is not on PATH"
nm=${NM:-}
if [ -z "$nm" ]; then
	for n in llvm-nm nm; do command -v "$n" >/dev/null 2>&1 && { nm=$n; break; }; done
fi
[ -n "$nm" ] || skip "no llvm-nm or nm"

res=$(clang -print-resource-dir)/include
flags=(--target=$rt -mcpu=cortex-r5 -mfpu=vfpv3-d16 -mfloat-abi=hard -std=c11
	-Wall -Wextra -Werror -pedantic -ffreestanding -nostdinc -isystem "$res" -O2
	-I"$root/crates/composition-ffi/include" -I"$tp" -I"$app")

mkdir -p "$out"
# A clang without the ARM backend cannot do this: say skipped, not failed.
echo 'int ce_probe;' >"$out/probe.c"
if ! clang "${flags[@]}" -c "$out/probe.c" -o "$out/probe.o" 2>"$out/probe.err"; then
	skip "clang cannot compile for $rt ($(head -n1 "$out/probe.err"))"
fi

echo "toolchain: clang ($nm)"
clang "${flags[@]}" -c "$app/scenes.c" -o "$out/scenes.o"
clang "${flags[@]}" -c "$tp/ce_twoplane.c" -o "$out/ce_twoplane.o"
echo "ok: scenes.c and ce_twoplane.c compile for $rt"

rc=0
undef() { $nm "$1" | awk '$1=="U"{print $2}'; }
defd() { $nm "$1" | awk '$2 ~ /^[TtWwDdBbRr]$/ {print $3}'; }

all_undef=$( (undef "$out/scenes.o"; undef "$out/ce_twoplane.o") | sort -u)
echo "-- undefined symbols of the objects:"
echo "$all_undef" | sed 's/^/  /'
bad=$(echo "$all_undef" | grep -v -E '^(ce_.*|__aeabi_memclr4|memcpy|memset)$' | grep . || true)
if [ -n "$bad" ]; then
	echo "fail: unexpected undefined symbols:" >&2
	echo "$bad" | sed 's/^/  /' >&2
	rc=1
else
	echo "ok: undefined symbols are only ce_*, __aeabi_memclr4, memcpy, memset"
fi

# What the objects need from outside themselves.
own=$( (defd "$out/scenes.o"; defd "$out/ce_twoplane.o") | sort -u)
need=$(echo "$all_undef" | grep -v -x -F -f <(echo "$own"; echo __none__) | grep . || true)

if command -v rustup >/dev/null 2>&1 && rustup target list --installed 2>/dev/null | grep -qx "$rt"; then
	(cd "$root" && cargo build --locked -p composition-ffi-staticlib --no-default-features \
		--features panic-handler --target "$rt" --release)
	lib=$root/target/$rt/release/libcomposition_engine.a
	libdef=$(defd "$lib" 2>/dev/null | sort -u)
	missing=$(echo "$need" | grep -v -x -F -f <(echo "$libdef"; echo __none__) | grep . || true)
	if [ -n "$missing" ]; then
		echo "fail: libcomposition_engine.a does not define:" >&2
		echo "$missing" | sed 's/^/  /' >&2
		rc=1
	else
		echo "ok: libcomposition_engine.a defines all $(echo "$need" | grep -c .) symbols the objects need"
	fi
else
	echo "skipped: library check, rustup has no $rt (rustup target add $rt)"
fi
exit $rc
