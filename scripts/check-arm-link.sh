#!/usr/bin/env bash
# Compile the twoplane C backend for Cortex-M7 and Cortex-R5, link each
# object into a minimal freestanding image, and check the images.
#
# Usage: scripts/check-arm-link.sh [OUT_DIR]     (default: target/arm-link)
#
# Host only: nothing is run, on a device or in an emulator.
#
# Toolchain: arm-none-eabi-gcc when it is on PATH (CC_KIND=gcc), else clang
# with a lld (ld.lld, or rust-lld of the Rust toolchain) (CC_KIND=clang).
# Set CC_KIND to force one. The engine symbols come from
# composition-ffi-staticlib built for the Rust target (cargo, rustup).
#
# Per target: compile ce_twoplane.c with -Wall -Wextra -Werror, build the
# static library, link ce_twoplane.o + start.c + main.c + the library with
# -nostdlib and the linker script, then require in the image (nm):
#   - __aeabi_memclr4 is defined (and which input provides it, from the map),
#   - no undefined symbol.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
out=${1:-$root/target/arm-link}
arm=$root/backends/twoplane/arm
inc=(-I"$root/crates/composition-ffi/include" -I"$root/backends/twoplane")
mkdir -p "$out"

kind=${CC_KIND:-}
if [ -z "$kind" ]; then
	if command -v arm-none-eabi-gcc >/dev/null 2>&1; then kind=gcc; else kind=clang; fi
fi

# name | rust target | cpu flags
targets=(
	"cortex-m7|thumbv7em-none-eabihf|-mcpu=cortex-m7 -mfpu=fpv5-d16 -mfloat-abi=hard -mthumb"
	"cortex-r5|armv7r-none-eabihf|-mcpu=cortex-r5 -mfloat-abi=hard"
)

case $kind in
gcc)
	nm=arm-none-eabi-nm
	# -lgcc is the runtime library, after the engine library.
	link() { # target-flags ld-script map out objs...
		local flags=$1 ld=$2 map=$3 img=$4
		shift 4
		# shellcheck disable=SC2086
		arm-none-eabi-gcc $flags -nostdlib -nostartfiles -T "$ld" -Wl,--gc-sections \
			-Wl,-Map="$map" -o "$img" "$@" -lgcc
	}
	;;
clang)
	nm=${NM:-}
	if [ -z "$nm" ]; then
		for n in llvm-nm nm; do command -v "$n" >/dev/null 2>&1 && { nm=$n; break; }; done
	fi
	lld=$(command -v ld.lld || true)
	if [ -z "$lld" ]; then
		sysroot=$(rustc --print sysroot)
		lld=$(ls "$sysroot"/lib/rustlib/*/bin/rust-lld 2>/dev/null | head -n1 || true)
		[ -n "$lld" ] && lld="$lld -flavor gnu"
	fi
	[ -n "$lld" ] || { echo "fail: no ld.lld or rust-lld" >&2; exit 2; }
	link() {
		local ld=$2 map=$3 img=$4
		shift 4
		# shellcheck disable=SC2086
		$lld -nostdlib --gc-sections -T "$ld" -Map="$map" -o "$img" "$@"
	}
	;;
*) echo "fail: CC_KIND must be gcc or clang" >&2; exit 2 ;;
esac
echo "toolchain: $kind ($nm)"

rc=0
for entry in "${targets[@]}"; do
	IFS='|' read -r name rt flags <<<"$entry"
	d=$out/$name
	mkdir -p "$d"
	echo "== $name ($rt)"
	rustup target add "$rt" >/dev/null 2>&1 || true
	(cd "$root" && cargo build --locked -p composition-ffi-staticlib --no-default-features \
		--features panic-handler --target "$rt" --release)
	lib=$root/target/$rt/release/libcomposition_engine.a

	if [ "$kind" = gcc ]; then
		cflags=($flags -ffreestanding -O2 -Wall -Wextra -Werror "${inc[@]}")
		# shellcheck disable=SC2068
		arm-none-eabi-gcc "${cflags[@]}" -c "$root/backends/twoplane/ce_twoplane.c" -o "$d/ce_twoplane.o"
		# start.c and main.c: no loop-to-memset rewriting in the start-up loops.
		arm-none-eabi-gcc "${cflags[@]}" -fno-builtin -fno-tree-loop-distribute-patterns -c "$arm/start.c" -o "$d/start.o"
		arm-none-eabi-gcc "${cflags[@]}" -c "$arm/main.c" -o "$d/main.o"
	else
		# -nostdinc: only the compiler's own headers, as a freestanding build has.
		res=$(clang -print-resource-dir)/include
		cflags=($flags -ffreestanding -nostdinc -isystem "$res" -O2 -Wall -Wextra -Werror "${inc[@]}")
		# shellcheck disable=SC2086
		clang --target="$rt" "${cflags[@]}" -c "$root/backends/twoplane/ce_twoplane.c" -o "$d/ce_twoplane.o"
		clang --target="$rt" "${cflags[@]}" -fno-builtin -c "$arm/start.c" -o "$d/start.o"
		clang --target="$rt" "${cflags[@]}" -c "$arm/main.c" -o "$d/main.o"
	fi

	echo "-- undefined symbols of the backend object:"
	$nm "$d/ce_twoplane.o" | grep -E ' U ' || true

	# The engine library comes before -lgcc (gcc), so the definition in its
	# compiler_builtins is the one the linker takes; the map says which.
	link "$flags" "$arm/$name.ld" "$d/image.map" "$d/image.elf" \
		"$d/start.o" "$d/main.o" "$d/ce_twoplane.o" "$lib"

	$nm "$d/image.elf" >"$d/image.nm"
	# clang emits __aeabi_memclr4 for the backend; GCC emits memset instead
	# (seen with Arm GCC 14.2.1). Require what the object really calls.
	if $nm "$d/ce_twoplane.o" | grep -E ' U __aeabi_memclr4$' >/dev/null; then
		if grep -E '^[0-9a-f]* [TtWw] __aeabi_memclr4$' "$d/image.nm" >/dev/null; then
			echo "ok: __aeabi_memclr4 defined in the image:"
			grep ' __aeabi_memclr4$' "$d/image.nm"
			echo "-- map lines naming it (the input that provides it):"
			grep -n -B1 -E '__aeabi_memclr4' "$d/image.map" | head -n8 || true
		else
			echo "fail: __aeabi_memclr4 is not defined in $name image" >&2
			rc=1
		fi
	else
		echo "ok: the object does not call __aeabi_memclr4; checking what it does call:"
		for sym in $($nm "$d/ce_twoplane.o" | awk '$1=="U"{print $2}' | grep -v '^ce_' || true); do
			if grep -E "^[0-9a-f]* [TtWw] $sym\$" "$d/image.nm" >/dev/null; then
				echo "  $sym defined in the image"
			else
				echo "fail: $sym is not defined in $name image" >&2
				rc=1
			fi
		done
	fi
	if $nm "$d/image.elf" | grep -E '^ +U ' >/dev/null; then
		echo "fail: undefined symbols in $name image:" >&2
		$nm "$d/image.elf" | grep -E '^ +U ' >&2
		rc=1
	else
		echo "ok: no undefined symbols in $name image"
	fi
done
exit $rc
