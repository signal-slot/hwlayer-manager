#!/bin/sh
# Self-test for am62p-state-snapshot.sh (fixtures under $TMPDIR).
set -u
here=$(cd "$(dirname "$0")" && pwd)
S=$here/am62p-state-snapshot.sh
T=$(mktemp -d "${TMPDIR:-/tmp}/snap.XXXXXX") || exit 1
trap 'rm -rf "$T"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }

R=$T/root
mkdir -p "$R/sys/class/remoteproc/remoteproc0" "$R/sys/bus/platform/drivers/tidss" \
    "$R/sys/bus/platform/drivers/omap-rproc" "$R/sys/bus/platform/drivers/other" \
    "$R/sys/class/drm/card0-HDMI-A-1" "$R/proc" "$R/etc"
echo r5f >"$R/sys/class/remoteproc/remoteproc0/name"
echo offline >"$R/sys/class/remoteproc/remoteproc0/state"
echo fw.elf >"$R/sys/class/remoteproc/remoteproc0/firmware"
ln -s /x "$R/sys/bus/platform/drivers/tidss/30200000.dss"
ln -s /x "$R/sys/bus/platform/drivers/omap-rproc/78000000.r5f"
ln -s /x "$R/sys/bus/platform/drivers/other/11111111.foo"
echo connected >"$R/sys/class/drm/card0-HDMI-A-1/status"
echo "console=tty0 ostree=/ostree/boot.1/torizon/abc/0 root=x" >"$R/proc/cmdline"
printf 'ID=torizon\nVERSION_ID=7.0\nHOSTNAME=secret\n' >"$R/etc/os-release"
cat >"$T/sysctl" <<'X'
#!/bin/sh
echo "docker.service loaded active running Docker"
echo "weston.service loaded inactive dead Weston"
echo "sshd.service loaded active running SSH"
X
cat >"$T/dock" <<'X'
#!/bin/sh
echo portainer
X
chmod +x "$T/sysctl" "$T/dock"
export STATE_SYSTEMCTL=$T/sysctl STATE_DOCKER=$T/dock

sh "$S" --root "$R" snapshot "$T/a" || fail snapshot
sh "$S" --root "$R" snapshot "$T/b" || fail snapshot2
grep -q 'unit docker.service active' "$T/a" || fail unit
grep -q 'container portainer' "$T/a" || fail container
grep -q 'driver tidss bound dss' "$T/a" || fail driver
if grep -q 'sshd\|11111111\|secret\|30200000' "$T/a"; then fail "leak or unfiltered"; fi
out=$(sh "$S" compare "$T/a" "$T/b") || fail "identical pair"
[ "$out" = SAME ] || fail "expected SAME"

rm "$R/sys/bus/platform/drivers/tidss/30200000.dss"
sh "$S" --root "$R" snapshot "$T/c"
sh "$S" compare "$T/a" "$T/c" >"$T/out"; rc=$?
[ $rc -eq 1 ] || fail "diff exit $rc"
grep -q 'driver tidss bound' "$T/out" || fail "diff content"
echo PASS
