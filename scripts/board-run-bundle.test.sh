#!/bin/sh
# Self-test for board-run-bundle.sh. Run: sh scripts/board-run-bundle.test.sh
here=$(cd "$(dirname "$0")" && pwd)
t=$(mktemp -d "${TMPDIR:-/tmp}/brb-test.XXXXXX") || exit 2
trap 'rm -rf "$t"' EXIT HUP INT TERM
fail=0
ok() { echo "ok   $1"; }
bad() { echo "FAIL $1"; fail=1; }
export BOARD_RUN_BUNDLE_DATE=2026-11-28 BOARD_RUN_BUNDLE_UNAME=6.18.test-fake

cp "$here/testdata/board-run-bundle/fake-cmd.sh" "$t/fake-cmd.sh"
mkdir -p "$t/out"
sh "$here/board-run-bundle.sh" --soc rk3588 --kind sweep --out "$t/out" \
    -- sh "$t/fake-cmd.sh" 10.9.8.7 root@boardhost --console=/dev/ttyUSB3 --planes=8 \
    >"$t/stdout" 2>"$t/stderr"
rc=$?
f=$t/out/sweep-2026-11-28-rk3588.txt
if [ "$rc" = 0 ]; then ok exit-0; else bad "exit $rc"; cat "$t/stderr"; fi
[ -f "$f" ] && ok file-name || bad file-name
[ "$(sed -n 1p "$f")" = "# observed" ] && ok header-observed || bad header-observed
sed -n 2p "$f" | grep -Fxq '# date: 2026-11-28' && ok header-date || bad header-date
sed -n 3p "$f" | grep -Fxq '# kernel: 6.18.test-fake' && ok header-kernel || bad header-kernel
if sed -n 4p "$f" | grep -Fxq "# command: sh $t/fake-cmd.sh <host> <host> --console=<host> --planes=8"; then
    ok header-command
else
    bad header-command; sed -n 4p "$f"
fi
grep -q 'plane 33 TEST_ONLY ok' "$f" && ok output-kept || bad output-kept
grep -q 'stderr line' "$f" && ok stderr-captured || bad stderr-captured
if grep -Eq '10\.9\.8\.7|ttyUSB|SERIALXYZ|aa:bb:cc:dd:ee:ff|boardhost' "$f"; then bad scrubbed; else ok scrubbed; fi
grep -q 'suggested README line: - sweep-2026-11-28-rk3588.txt' "$t/stdout" && ok readme-suggestion || bad readme-suggestion
[ ! -e "$t/out/README.md" ] && ok no-readme-written || bad no-readme-written
[ "$(ls "$t/out" | wc -l)" = 1 ] && ok one-file || bad one-file

# check-test-data.sh accepts the result.
mkdir -p "$t/data/rk3588"
cp "$f" "$t/data/rk3588/"
echo "- sweep-2026-11-28-rk3588.txt: sweep" >"$t/data/rk3588/README.md"
printf '#!/bin/sh\nexit 0\n' >"$t/nosecrets.sh"
if CHECK_NO_BOARD_SECRETS=$t/nosecrets.sh bash "$here/check-test-data.sh" "$t/data" >/dev/null 2>&1; then
    ok check-test-data
else
    bad check-test-data
fi

# The real secrets check passes on the result (throwaway repo inside).
BOARD_RUN_BUNDLE_SECRETS=$here/check-no-board-secrets.sh sh "$here/board-run-bundle.sh" \
    --soc bcm2712 --kind caps --out "$t/out" -- sh "$t/fake-cmd.sh" >/dev/null 2>&1
[ $? = 0 ] && ok real-secrets-check || bad real-secrets-check

# The real checker catches an address the scrub step let through.
printf 'import sys\nsys.stdout.write(open(sys.argv[-1]).read())\n' >"$t/pass.py"
BOARD_RUN_BUNDLE_SCRUB=$t/pass.py BOARD_RUN_BUNDLE_SECRETS=$here/check-no-board-secrets.sh \
    sh "$here/board-run-bundle.sh" --soc bcm2712 --kind verify --name leak --out "$t/out" \
    -- sh "$t/fake-cmd.sh" >/dev/null 2>&1
[ $? = 1 ] && ok real-secrets-catches-ip || bad real-secrets-catches-ip

# Dotted host names in CMD are redacted; --hostname redacts plain names
# in the header and the output.
sh "$here/board-run-bundle.sh" --soc rk3588 --kind caps --name hosts --hostname nanopc --out "$t/out" \
    -- sh "$t/fake-cmd.sh" board.lan nanopc-t6.local nanopc >/dev/null 2>&1
h=$t/out/caps-2026-11-28-hosts.txt
if grep -Eq 'board\.lan|nanopc' "$h"; then bad hostnames-redacted; grep -n nanopc "$h"; else ok hostnames-redacted; fi
sed -n 4p "$h" | grep -q '<host> <host> <host>' && ok hostnames-header || bad hostnames-header

# A failing secrets check makes the wrapper fail.
printf '#!/bin/sh\necho "ipv4: x:1: leak" >&2\nexit 1\n' >"$t/leaky.sh"
BOARD_RUN_BUNDLE_SECRETS=$t/leaky.sh sh "$here/board-run-bundle.sh" --soc bcm2712 --kind verify --out "$t/out" \
    -- true >/dev/null 2>&1
[ $? = 1 ] && ok secrets-fail-exit-1 || bad secrets-fail-exit-1

# A failing scrub makes the wrapper fail and writes no file.
printf '#!/bin/sh\nexit 2\n' >"$t/badscrub.py"
BOARD_RUN_BUNDLE_SCRUB=$t/badscrub.py sh "$here/board-run-bundle.sh" --soc bcm2711 --kind verify --out "$t/out" \
    -- true >/dev/null 2>&1
rc=$?
[ "$rc" = 1 ] && [ ! -e "$t/out/verify-2026-11-28-bcm2711.txt" ] && ok scrub-fail || bad scrub-fail

# A failing command exits 3 but is still bundled.
sh "$here/board-run-bundle.sh" --soc bcm2711 --kind caps --name x --out "$t/out" -- false >/dev/null 2>&1
rc=$?
[ "$rc" = 3 ] && [ -f "$t/out/caps-2026-11-28-x.txt" ] && ok cmd-fail-exit-3 || bad cmd-fail-exit-3

# Usage errors.
sh "$here/board-run-bundle.sh" --soc imx95 --kind caps --out "$t/out" -- true >/dev/null 2>&1
[ $? = 2 ] && ok bad-soc || bad bad-soc
sh "$here/board-run-bundle.sh" --soc rk3588 --kind caps --out "$t/out" >/dev/null 2>&1
[ $? = 2 ] && ok no-cmd || bad no-cmd

exit $fail
