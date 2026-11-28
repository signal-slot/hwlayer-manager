#!/bin/sh
# Tests for measure-ffi-staticlib-size-armv7r.sh with stub rustup, cargo and
# size on PATH: skip, output format, build failure. Nothing is built or
# installed.
set -u
here=$(cd "$(dirname "$0")" && pwd)
script=$here/measure-ffi-staticlib-size-armv7r.sh
tmp=$(mktemp -d "${TMPDIR:-/tmp}/measure7r.XXXXXX") || exit 1
trap 'rm -rf "$tmp"' EXIT
fails=0

ok() { echo "pass $1"; }
bad() { echo "FAIL $1"; fails=$((fails + 1)); }
has() { case $1 in *"$2"*) return 0 ;; *) return 1 ;; esac; }

sys=$tmp/sys
mkdir -p "$sys"
for t in sh dirname grep cat awk date tail mkdir; do
	p=$(command -v "$t") && ln -s "$p" "$sys/$t"
done
stubs=$tmp/stubs
mkdir -p "$stubs"
cat >"$stubs/rustup" <<'EOS'
#!/bin/sh
printf '%s\n' $STUB_TARGETS
EOS
cat >"$stubs/cargo" <<'EOS'
#!/bin/sh
echo "cargo $*" >>"$STUB_LOG"
[ -z "$STUB_CARGO_FAIL" ] || exit 101
d=$CARGO_TARGET_DIR/armv7r-none-eabihf/release
mkdir -p "$d"
: >"$d/libcomposition_engine.a"
EOS
cat >"$stubs/size" <<'EOS'
#!/bin/sh
echo "   text	   data	    bss	    dec	    hex	filename"
echo "    100	     10	      1	    111	     6f	lib.a(a.o)"
echo "    200	     20	      2	    222	     de	lib.a(b.o)"
echo "      5	      0	      0	      5	      5	lib.a(c.o)"
EOS
chmod +x "$stubs/rustup" "$stubs/cargo" "$stubs/size"
sh_bin=$(command -v sh)
CARGO_TARGET_DIR=$tmp/target export CARGO_TARGET_DIR
STUB_CARGO_FAIL= export STUB_CARGO_FAIL

run() { # args...; sets rc, out, log
	: >"$tmp/log"
	out=$(STUB_LOG=$tmp/log PATH=$stubs:$sys "$sh_bin" "$script" "$@" 2>&1)
	rc=$?
	log=$(cat "$tmp/log")
}

# skip: target missing (a similar name must not match)
STUB_TARGETS="thumbv7em-none-eabihf armv7r-none-eabi" export STUB_TARGETS
run
if [ $rc -eq 0 ] && [ "$out" = "SKIP: target not installed" ]; then ok skip; else bad "skip ($rc: $out)"; fi
if has "$log" cargo; then bad skip-ran-cargo; else ok skip-no-cargo; fi

# measured: sums and the dated host-build line
STUB_TARGETS="armv7r-none-eabihf" export STUB_TARGETS
run
if [ $rc -eq 0 ] && has "$out" "text=305 data=30 bss=3"; then ok sums; else bad "sums ($rc: $out)"; fi
if has "$log" "--no-default-features --features panic-handler --target armv7r-none-eabihf --release"; then ok cargo-args; else bad "cargo-args ($log)"; fi
last=$(printf '%s\n' "$out" | tail -n 1)
case $last in
[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]": host build, not measured on hardware") ok dated-line ;;
*) bad "dated-line ($last)" ;;
esac

# build failure
STUB_CARGO_FAIL=1 export STUB_CARGO_FAIL
run
if [ $rc -eq 1 ] && has "$out" "FAIL"; then ok build-fail; else bad "build-fail ($rc: $out)"; fi
STUB_CARGO_FAIL= export STUB_CARGO_FAIL

# bad usage
run extra
if [ $rc -eq 2 ]; then ok usage; else bad "usage ($rc)"; fi

[ $fails -eq 0 ] || { echo "$fails failed"; exit 1; }
echo "all passed"
