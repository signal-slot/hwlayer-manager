#!/usr/bin/env bash
# Host test for rk3588-session-run.sh: bash -n and --dry-run plan only.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
s=$here/rk3588-session-run.sh
t=${TMPDIR:-/tmp}/rk3588-run-test.$$
trap 'rm -rf "$t"' EXIT
mkdir -p "$t/bin"
fail() { echo "FAIL: $*" >&2; exit 1; }

bash -n "$s"
bash -n "$0"
for n in probe props kms_sweep pending_probe; do
	printf '#!/bin/sh\nexit 0\n' > "$t/bin/$n"
	chmod +x "$t/bin/$n"
done
printf '# comment\n\nprops /dev/dri/card0\n' > "$t/steps"

marker=fake-board-marker-xyz
out=$(BOARD_SSH="login-$marker" "$s" --dry-run --bin-dir "$t/bin" --out "$t/out" \
	--steps "$t/steps" --step "kms_sweep combos /dev/dri/card0 --sets 32" \
	--step "pending_probe /dev/dri/card0")
case $out in *"display-guard.sh stop"*) ;; *) fail "no display-guard stop" ;; esac
case $out in *"display-guard.sh restore"*) ;; *) fail "no display-guard restore" ;; esac
case $out in *"rm -rf"*) ;; *) fail "no cleanup" ;; esac
case $out in *"./kms_sweep combos /dev/dri/card0 --sets 32"*) ;; *) fail "args lost" ;; esac
case $out in *"$marker"*) fail "BOARD_SSH leaked into plan" ;; esac
[ "$(printf '%s\n' "$out" | grep -c '^step ')" = 3 ] || fail "expected 3 steps"
[ ! -e "$t/out" ] || fail "dry-run wrote output"
for w in reboot modprobe insmod udevadm; do
	case $out in *"$w"*) fail "forbidden command $w" ;; esac
done

if "$s" --dry-run --bin-dir "$t/bin" --out "$t/out" --step "nosuch x" 2>/dev/null; then fail "unknown example accepted"; fi
if "$s" --dry-run --bin-dir "$t/bin" --out "$t/out" --step "scale_probe" 2>/dev/null; then fail "missing binary accepted"; fi
if env -u BOARD_SSH "$s" --bin-dir "$t/bin" --out "$t/out" --step props 2>/dev/null; then fail "ran without BOARD_SSH"; fi

if grep -rIl "$marker" "$t" 2>/dev/null; then fail "secret written to a file"; fi
echo "rk3588-session-run.test.sh: PASS"
