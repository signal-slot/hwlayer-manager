#!/bin/sh
# Tests for check-armv7r-build.sh with stub cargo and rustup on PATH:
# skip, strict skip, pass, fail, bad usage. Nothing is built or installed.
set -u
here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-armv7r-build.sh
tmp=$(mktemp -d "${TMPDIR:-/tmp}/armv7r.XXXXXX") || exit 1
trap 'rm -rf "$tmp"' EXIT
fails=0

ok() { echo "pass $1"; }
bad() { echo "FAIL $1"; fails=$((fails + 1)); }
has() { case $1 in *"$2"*) return 0 ;; *) return 1 ;; esac; }

# PATH with only the basic tools; the stubs dir comes first.
sys=$tmp/sys
mkdir -p "$sys"
for t in sh dirname grep cat; do
	p=$(command -v "$t") && ln -s "$p" "$sys/$t"
done
stubs=$tmp/stubs
mkdir -p "$stubs"
cat >"$stubs/rustup" <<'EOS'
#!/bin/sh
echo "rustup $*" >>"$STUB_LOG"
[ "$1 $2 $3" = "target list --installed" ] || exit 9
printf '%s\n' $STUB_TARGETS
EOS
cat >"$stubs/cargo" <<'EOS'
#!/bin/sh
echo "cargo $*" >>"$STUB_LOG"
if [ -n "$STUB_FAIL_PKG" ]; then
	case " $* " in *" $STUB_FAIL_PKG "*) exit 101 ;; esac
fi
exit 0
EOS
chmod +x "$stubs/rustup" "$stubs/cargo"
sh_bin=$(command -v sh)

run() { # args...; sets rc, out, log
	: >"$tmp/log"
	out=$(STUB_LOG=$tmp/log PATH=$stubs:$sys "$sh_bin" "$script" "$@" 2>&1)
	rc=$?
	log=$(cat "$tmp/log")
}

STUB_FAIL_PKG= export STUB_FAIL_PKG

# skip: target missing (a similar name must not match)
STUB_TARGETS="thumbv7em-none-eabihf armv7r-none-eabi" export STUB_TARGETS
run
if [ $rc -eq 0 ] && [ "$out" = "ARMV7R SKIP: target not installed" ]; then ok skip; else bad "skip ($rc: $out)"; fi
if has "$log" "cargo"; then bad skip-ran-cargo; else ok skip-no-cargo; fi
if has "$log" "target add"; then bad skip-installed; else ok skip-no-install; fi

# strict: missing target fails
run --strict
if [ $rc -eq 1 ] && has "$out" "ARMV7R FAIL"; then ok strict-missing; else bad "strict-missing ($rc: $out)"; fi

# pass
STUB_TARGETS="thumbv7em-none-eabihf armv7r-none-eabihf" export STUB_TARGETS
run
if [ $rc -eq 0 ] && has "$out" "ARMV7R PASS"; then ok pass; else bad "pass ($rc: $out)"; fi
if has "$log" "cargo build -p composition-core --target armv7r-none-eabihf"; then ok pass-core; else bad pass-core; fi
if has "$log" "cargo build -p composition-twoplane --target armv7r-none-eabihf"; then ok pass-twoplane; else bad pass-twoplane; fi
run --strict
if [ $rc -eq 0 ] && has "$out" "ARMV7R PASS"; then ok strict-pass; else bad "strict-pass ($rc)"; fi

# fail: core, then twoplane
STUB_FAIL_PKG=composition-core export STUB_FAIL_PKG
run
if [ $rc -eq 1 ] && has "$out" "ARMV7R FAIL: composition-core"; then ok fail-core; else bad "fail-core ($rc: $out)"; fi
STUB_FAIL_PKG=composition-twoplane export STUB_FAIL_PKG
run
if [ $rc -eq 1 ] && has "$out" "ARMV7R FAIL: composition-twoplane"; then ok fail-twoplane; else bad "fail-twoplane ($rc: $out)"; fi
STUB_FAIL_PKG= export STUB_FAIL_PKG

# usage
run --bogus
if [ $rc -eq 2 ]; then ok usage; else bad "usage ($rc)"; fi
run --strict extra
if [ $rc -eq 2 ]; then ok usage-extra; else bad "usage-extra ($rc)"; fi

if [ $fails -ne 0 ]; then echo "$fails failed"; exit 1; fi
echo "all passed"
