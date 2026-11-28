#!/bin/sh
# Fake-board dry run of the AM62P restore sequence (board session step 4):
# snapshot before -> stop a service, unbind a driver -> compare reports a
# difference -> restore + "reboot" to the Torizon state -> compare is SAME
# and the Torizon check passes. Host only; no board is touched.
#
# Uses scripts/am62p-state-snapshot.sh (--root fake tree, STATE_SYSTEMCTL and
# STATE_DOCKER stubs) and scripts/am62p-torizon-check.sh (SSH stub that
# answers from the fake tree). No existing script is modified.
# Findings on the existing scripts: none (no bug seen).
# If am62p-torizon-check.sh is absent, only the snapshot path is tested.
set -u
here=$(cd "$(dirname "$0")" && pwd)
S=$here/am62p-state-snapshot.sh
C=$here/am62p-torizon-check.sh
[ -f "$S" ] || { echo "FAIL: $S missing" >&2; exit 1; }
HAVE_C=1
[ -f "$C" ] || HAVE_C=0
T=$(mktemp -d "${TMPDIR:-/tmp}/restore-dryrun.XXXXXX") || exit 1
trap 'rm -rf "$T"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }

R=$T/root
D=$R/sys/bus/platform/drivers
mkdir -p "$R/sys/class/remoteproc/remoteproc0" "$D/tidss" "$D/ti-k3-r5-remoteproc" \
    "$R/sys/class/drm/card0-HDMI-A-1" "$R/proc" "$R/etc" "$T/state"

# Torizon state of the fake board: used at first boot and as the "reboot".
torizon_state() {
    echo r5f >"$R/sys/class/remoteproc/remoteproc0/name"
    echo offline >"$R/sys/class/remoteproc/remoteproc0/state"
    echo am62p-mcu-r5f0_0-fw >"$R/sys/class/remoteproc/remoteproc0/firmware"
    rm -f "$D"/tidss/*.dss "$D"/ti-k3-r5-remoteproc/*.r5f
    ln -s /x "$D/tidss/30200000.dss"
    ln -s /x "$D/ti-k3-r5-remoteproc/79000000.r5f"
    echo connected >"$R/sys/class/drm/card0-HDMI-A-1/status"
    echo "console=tty0 ostree=/ostree/boot.1/torizon/abc/0 root=x" >"$R/proc/cmdline"
    printf 'ID=torizon\nNAME="Torizon OS"\nVERSION_ID=7.0.0\nBUILD_ID=1\n' >"$R/etc/os-release"
    echo active >"$T/state/weston"
    echo weston >"$T/state/containers"
}
torizon_state

cat >"$T/sysctl" <<EOS
#!/bin/sh
echo "docker.service loaded active running Docker"
echo "weston.service loaded \$(cat "$T/state/weston") running Weston"
echo "sshd.service loaded active running SSH"
EOS
cat >"$T/dock" <<EOS
#!/bin/sh
cat "$T/state/containers"
EOS
# Fake ssh: answers like the check's remote command, from the fake tree.
cat >"$T/ssh" <<EOS
#!/bin/sh
[ -f "$T/state/ssh-down" ] && exit 255
echo "DATE 2026-01-01T00:00:00Z"
echo "OS_ID \$(sed -n 's/^ID=//p' "$R/etc/os-release")"
echo "OS_NAME Torizon OS"
echo "KERNEL 6.6.0-fake"
echo "UPTIME up"
echo "SYSTEMD running"
echo "REMOTEPROC remoteproc0 name r5f state \$(cat "$R/sys/class/remoteproc/remoteproc0/state") firmware x"
echo "END"
EOS
chmod +x "$T/sysctl" "$T/dock" "$T/ssh"
export STATE_SYSTEMCTL=$T/sysctl STATE_DOCKER=$T/dock

snap() { sh "$S" --root "$R" snapshot "$1" || fail "snapshot $1"; }
cmp_() { sh "$S" compare "$1" "$2" >"$T/cmp.out" 2>&1; }
tcheck() { SSH=$T/ssh AM62P_SSH_DEST=fake sh "$C" >"$T/tc.out" 2>&1; }

# 1. before
snap "$T/before"
grep -q 'unit weston.service active' "$T/before" || fail "before: weston not active"
grep -q 'driver tidss bound dss' "$T/before" || fail "before: tidss not bound"
grep -q 'remoteproc name=r5f state=offline' "$T/before" || fail "before: remoteproc"

# 2. board session: stop weston and the container, unbind tidss, boot the R5F
echo inactive >"$T/state/weston"
: >"$T/state/containers"
rm "$D/tidss/30200000.dss"
echo running >"$R/sys/class/remoteproc/remoteproc0/state"
snap "$T/during"
cmp_ "$T/before" "$T/during"
rc=$?
[ $rc -eq 1 ] || fail "compare during: exit $rc, expected 1"
grep -q 'weston.service' "$T/cmp.out" || fail "diff lacks the stopped service"
grep -q 'driver tidss bound' "$T/cmp.out" || fail "diff lacks the unbound driver"
grep -q 'state=running' "$T/cmp.out" || fail "diff lacks the remoteproc state"

# 3. a partial restore must still differ
echo active >"$T/state/weston"
snap "$T/partial"
cmp_ "$T/before" "$T/partial"
rc=$?
[ $rc -eq 1 ] || fail "partial restore compared equal (exit $rc)"

# 4. full restore + reboot into Torizon
if [ $HAVE_C = 1 ]; then
    : >"$T/state/ssh-down"      # ssh is down while rebooting
    tcheck
    rc=$?
    [ $rc -eq 1 ] || fail "torizon check while ssh down: exit $rc"
    grep -q '^TORIZON NOT-UP' "$T/tc.out" || fail "no NOT-UP verdict"
fi
torizon_state
rm -f "$T/state/ssh-down"
snap "$T/after"
cmp_ "$T/before" "$T/after"
rc=$?
[ $rc -eq 0 ] || fail "compare after restore: exit $rc"
[ "$(cat "$T/cmp.out")" = SAME ] || fail "compare after restore: not SAME"
if [ $HAVE_C = 1 ]; then
    tcheck
    rc=$?
    [ $rc -eq 0 ] || fail "torizon check after reboot: exit $rc"
    grep -q '^TORIZON UP' "$T/tc.out" || fail "no TORIZON UP verdict"
else
    echo "note: am62p-torizon-check.sh absent; snapshot path only"
fi
echo PASS
