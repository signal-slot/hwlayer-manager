#!/bin/sh
# Test for am62p-board-info.sh against a fake, generic tree in $TMPDIR.
here=$(cd "$(dirname "$0")" && pwd)
T=$(mktemp -d "${TMPDIR:-/tmp}/am62p-info.XXXXXX") || exit 1
trap 'rm -rf "$T"' EXIT
R=$T/root
D=$R/sys/class/drm/card0-DPI-1
mkdir -p "$R/etc" "$R/proc/sys/kernel" "$R/proc/device-tree" "$D" \
  "$R/sys/class/drm/card0-HDMI-A-1" \
  "$R/sys/class/remoteproc/remoteproc0" "$R/sys/class/remoteproc/remoteproc1" \
  "$R/lib/firmware"
printf 'fakehost01\n' > "$R/etc/hostname"
printf 'fakehost01\n' > "$R/proc/sys/kernel/hostname"
cat > "$R/etc/os-release" <<X
NAME="Fake OS"
VERSION="1.2.3+build.4"
VERSION_ID=1.2.3
PRETTY_NAME="Fake OS 1.2.3"
BUILD_ID=20990101
MACHINE_ID=0123456789abcdef0123456789abcdef
X
printf '9.9.9-fake\n' > "$R/proc/sys/kernel/osrelease"
printf '#1 SMP Fri Jan 1 00:00:00 UTC 2100 fakehost01\n' > "$R/proc/sys/kernel/version"
printf 'Fake Board Model\0' > "$R/proc/device-tree/model"
printf 'fake,board\0fake,soc\0' > "$R/proc/device-tree/compatible"
printf 'FAKESERIAL0042\0' > "$R/proc/device-tree/serial-number"
printf '0123456789abcdef0123456789abcdef\n' > "$R/etc/machine-id"
printf 'connected\n' > "$D/status"
printf '800x480\n640x480\n' > "$D/modes"
# EDID-like blob: bytes 0-53 filler, then a name descriptor (0xFC) and a
# serial descriptor (0xFF), as in a real base block
{
  head -c 54 /dev/zero
  printf '\000\000\000\374\000FakePanel\n   '
  printf '\000\000\000\377\000SN-FAKE-77\n   '
  head -c 20 /dev/zero
} > "$D/edid"
printf 'disconnected\n' > "$R/sys/class/drm/card0-HDMI-A-1/status"
: > "$R/sys/class/drm/card0-HDMI-A-1/modes"
printf 'r5f-fake\n' > "$R/sys/class/remoteproc/remoteproc0/name"
printf 'offline\n' > "$R/sys/class/remoteproc/remoteproc0/state"
printf 'fake-fw.elf\n' > "$R/sys/class/remoteproc/remoteproc0/firmware"
printf 'dsp-fake\n' > "$R/sys/class/remoteproc/remoteproc1/name"
printf 'running\n' > "$R/sys/class/remoteproc/remoteproc1/state"
ln -s /opt/fake/image.elf "$R/lib/firmware/fake-fw.elf"

out=$T/out
sh "$here/am62p-board-info.sh" --root "$R" > "$out" || { echo "FAIL: exit"; exit 1; }
fail=0
has() { grep -F -q -- "$1" "$out" || { echo "FAIL: missing '$1'"; fail=1; }; }
hasnt() { if grep -F -q -- "$1" "$out"; then echo "FAIL: leaked '$1'"; fail=1; fi; }
has '| Fake OS 1.2.3 |'
has '9.9.9-fake'
has 'Fake Board Model'
has 'fake,board; fake,soc'
has 'FakePanel'
has 'status: connected; modes: 800x480 640x480'
has 'name: r5f-fake; state: offline'
has 'fake-fw.elf -> /opt/fake/image.elf'
has 'name: dsp-fake; state: running'
hasnt 'FAKESERIAL0042'
hasnt 'SN-FAKE-77'
hasnt 'fakehost01'
hasnt '0123456789abcdef0123456789abcdef'
has '<redacted>'
if sh "$here/am62p-board-info.sh" --bogus >/dev/null 2>&1; then
  echo "FAIL: bad option accepted"; fail=1
fi
# POSIX-only awk: busybox awk, mawk, or gawk --posix (no strtonum there)
PA=
if command -v busybox >/dev/null 2>&1 && busybox awk 'BEGIN{exit 0}' 2>/dev/null; then
  PA="busybox awk"
elif command -v mawk >/dev/null 2>&1; then
  PA=mawk
elif command -v original-awk >/dev/null 2>&1; then
  PA=original-awk
elif command -v gawk >/dev/null 2>&1; then
  PA="gawk --posix"
fi
if [ -z "$PA" ]; then
  echo "SKIP: no POSIX-only awk (busybox, mawk, original-awk, gawk) on this host"
else
  mkdir -p "$T/bin"
  printf '#!/bin/sh\nexec %s "$@"\n' "$PA" > "$T/bin/awk"
  chmod +x "$T/bin/awk"
  PATH="$T/bin:$PATH" sh "$here/am62p-board-info.sh" --root "$R" > "$T/out2" \
    || { echo "FAIL: exit under $PA"; fail=1; }
  grep -F -q 'edid name: FakePanel' "$T/out2" \
    || { echo "FAIL: panel name under $PA"; fail=1; }
  if grep -F -q 'SN-FAKE-77' "$T/out2"; then echo "FAIL: serial under $PA"; fail=1; fi
  echo "posix awk case ran with: $PA"
fi
[ $fail = 0 ] && echo "am62p-board-info.test.sh: ok"
exit $fail
