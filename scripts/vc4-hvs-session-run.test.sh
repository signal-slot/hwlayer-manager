#!/bin/bash
# Host test for vc4-hvs-session-run.sh: bash -n, --dry-run checks and a run
# against a fake ssh. Nothing touches a board.
set -u
here=$(cd "$(dirname "$0")" && pwd)
run=$here/vc4-hvs-session-run.sh
work=$(mktemp -d "${TMPDIR:-/tmp}/hvs-run-test.XXXXXX") || exit 1
trap 'rm -rf "$work"' EXIT
fail=0
ok() { echo "ok   $*"; }
bad() { echo "FAIL $*"; fail=1; }
has() { if grep -q -e "$2" <<<"$1"; then ok "$3"; else bad "$3"; fi; }
hasnt() { if grep -q -e "$2" <<<"$1"; then bad "$3"; else ok "$3"; fi; }

if bash -n "$run"; then ok "bash -n"; else bad "bash -n"; fi
if bash -n "$0"; then ok "bash -n test"; else bad "bash -n test"; fi

bins=$work/bins
mkdir "$bins"
for b in probe props scale_probe pending_probe mode_test; do
	printf '#!/bin/sh\necho fake\n' > "$bins/$b"
done

# --- dry run (no BOARD_SSH needed, no ssh run)
out=$(env -u BOARD_SSH "$run" --dry-run --bin-dir "$bins" --out "$work/o" \
	--step 'p:probe --raw /dev/dri/card0' --step 's:scale_probe /dev/dri/card0 --max-factor 64' 2>&1)
rc=$?
if [ $rc -eq 0 ]; then ok "dry-run exit 0"; else bad "dry-run exit $rc"; fi
has "$out" 'mktemp -d /tmp/hvs-session' "dry-run: fresh /tmp dir"
has "$out" 'display-guard.sh stop' "dry-run: guard stop"
has "$out" 'display-guard.sh restore' "dry-run: guard restore"
has "$out" 'uname -r' "dry-run: uname"
has "$out" 'rm -rf /tmp/hvs-session' "dry-run: cleanup"
has "$out" 'probe --raw /dev/dri/card0' "dry-run: step 1"
has "$out" 'scale_probe /dev/dri/card0 --max-factor 64' "dry-run: step 2"
hasnt "$out" 'downscale_probe' "dry-run: absent binary not copied"
if [ ! -e "$work/o" ]; then ok "dry-run creates no output dir"; else bad "dry-run created output dir"; fi
for w in reboot modprobe insmod udevadm flash; do
	hasnt "$out" "$w" "dry-run: no $w"
done

# --- argument refusals
for bad_step in 'x:reboot' 'x:probe; reboot' 'x:probe $(id)' 'noname' 'x:probe > /etc/x'; do
	"$run" --dry-run --bin-dir "$bins" --out "$work/o" --step "$bad_step" > /dev/null 2>&1
	rc=$?
	if [ $rc -eq 2 ]; then ok "refuses step '$bad_step'"; else bad "accepts step '$bad_step'"; fi
done

# --- fake ssh run
cat > "$work/ssh" <<'FAKE'
#!/bin/sh
for last; do :; done
echo "$last" >> "$FAKE_LOG"
case $last in
mktemp*) echo /tmp/hvs-session.FAKE01 ;;
tar*) cat > /dev/null ;;
'uname -r') echo 6.12.99-fake ;;
*display-guard.sh*stop*) echo "DISPLAY-GUARD stopped display-manager.service" ;;
*display-guard.sh*restore*) echo "DISPLAY-GUARD restored display-manager.service active"; exit "${FAKE_RESTORE_RC:-0}" ;;
*display-guard.sh*detect*) echo display-manager.service ;;
'rm -rf'*) touch "$FAKE_GONE" ;;
'ls -d'*) [ -e "$FAKE_GONE" ] && exit 1; exit 0 ;;
cd*) echo "out from secretuser at secrethost"; echo "err secrethost" >&2; exit "${FAKE_STEP_RC:-0}" ;;
esac
FAKE
chmod +x "$work/ssh"
export SSH_BIN=$work/ssh FAKE_LOG=$work/log FAKE_GONE=$work/gone
export BOARD_SSH='-o BatchMode=yes secretuser@secrethost'

o=$work/run1
out=$("$run" --bin-dir "$bins" --out "$o" --step 'p:probe --raw /dev/dri/card0' --step 'm:mode_test' 2>&1)
rc=$?
if [ $rc -eq 0 ]; then ok "run exit 0"; else bad "run exit $rc"; fi
has "$out" 'kernel=6.12.99-fake' "summary has uname"
has "$out" 'DISPLAY-MANAGER: active again' "reports display manager active"
has "$out" 'CLEANUP: board work directory removed' "reports cleanup"
if [ "$(cat "$o/01-p.exit")" = 0 ]; then ok "exit code captured"; else bad "exit code"; fi
if [ -s "$o/01-p.stdout" ] && [ -s "$o/01-p.stderr" ]; then ok "stdout and stderr captured"; else bad "stdout/stderr"; fi
if grep -q 'KERNEL 6.12.99-fake' "$o/01-p.header" && grep -q 'host UTC' "$o/01-p.header"; then ok "header has UTC and kernel"; else bad "header"; fi
if grep -q '<board>' "$o/01-p.stdout"; then ok "board login scrubbed from stdout"; else bad "scrub"; fi
if grep -rq -e secretuser -e secrethost "$o"; then bad "login found in outputs"; else ok "no login in outputs"; fi
if grep -q 'rm -rf /tmp/hvs-session.FAKE01' "$work/log"; then ok "rm -rf issued"; else bad "rm"; fi

# failing step is recorded, exit 1, still cleaned up
rm -f "$work/gone"
FAKE_STEP_RC=7 "$run" --bin-dir "$bins" --out "$work/run2" --step 'p:probe' > /dev/null 2>&1
rc=$?
if [ $rc -eq 1 ] && [ "$(cat "$work/run2/01-p.exit")" = 7 ]; then ok "step failure recorded, exit 1"; else bad "step failure (rc=$rc)"; fi
if [ -e "$work/gone" ]; then ok "cleanup after failing step"; else bad "no cleanup after failure"; fi

# restore failure is not hidden
rm -f "$work/gone"
FAKE_RESTORE_RC=1 "$run" --bin-dir "$bins" --out "$work/run3" --step 'p:probe' > "$work/run3.txt" 2>&1
rc=$?
if [ $rc -eq 3 ] && grep -q 'NOT confirmed active' "$work/run3.txt"; then ok "restore failure exits 3"; else bad "restore failure (rc=$rc)"; fi

if [ $fail -eq 0 ]; then echo "PASS vc4-hvs-session-run.test.sh"; else echo FAILED; exit 1; fi
