#!/usr/bin/env bash
# Host test for imx95-observe.sh: runs it on a fake tree under $TMPDIR.
# Run: bash scripts/imx95-observe.test.sh
here=$(cd "$(dirname "$0")" && pwd)
script=$here/imx95-observe.sh
root=$(mktemp -d "${TMPDIR:-/tmp}/observe-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok()  { echo "ok   $1"; }
bad() { echo "FAIL $1"; fail=1; }
has() { grep -Fq -- "$2" "$root/out" && ok "$1" || bad "$1"; }
hasnt() { grep -Fq -- "$2" "$root/out" && bad "$1" || ok "$1"; }

# Fake values assembled from pieces so this file stays clean.
ip="192.168.""7.42"
mac="aa:bb:cc:""dd:ee:ff"
host="fakebox""-77"
ser="SN""1234567"

p=$root/proc
s=$root/sys
dt=$p/device-tree
mkdir -p "$p/sys/kernel" "$s/class/remoteproc/remoteproc0" "$s/kernel/debug/clk" "$s/kernel/debug/pm_genpd"
mkdir -p "$dt/soc/display-controller@4b400000" "$dt/soc/serial@1" "$dt/reserved-memory/m7-vring" "$dt/firmware/scmi"
printf '%s\n' "$host" >"$p/sys/kernel/hostname"
printf '%s\0' "$ser" >"$dt/serial-number"
printf 'fsl,imx95-dpu\0' >"$dt/soc/display-controller@4b400000/compatible"
printf 'okay\0' >"$dt/soc/display-controller@4b400000/status"
printf '\x00\x00\x00\x00\x4b\x40\x00\x00\x00\x00\x00\x00\x00\x10\x00\x00' >"$dt/soc/display-controller@4b400000/reg"
printf 'arm,scmi\0' >"$dt/firmware/scmi/compatible"
printf 'shared-dma-pool\0' >"$dt/reserved-memory/m7-vring/compatible"
printf 'ns16550\0' >"$dt/soc/serial@1/compatible"
printf 'imx-rproc\n' >"$s/class/remoteproc/remoteproc0/name"
printf 'offline\n' >"$s/class/remoteproc/remoteproc0/state"
printf 'hello.elf\n' >"$s/class/remoteproc/remoteproc0/firmware"
printf ' dpu_pixel  1 1 0 148500000 0\n uart  1 1 0 24000000 0\n lvds_ldb 0 0 0 1 0\n' >"$s/kernel/debug/clk/clk_summary"
printf 'dispmix on\nnetcmix on\n' >"$s/kernel/debug/pm_genpd/pm_genpd_summary"
cat >"$root/dmesg.sh" <<EOT
#!/bin/sh
echo "[ 1.0] SCMI: SM version 1.2 from $ip mac $mac"
echo "[ 1.1] imx-rproc: m7 ready on $host"
echo "[ 1.2] Serial Number: $ser"
echo "[ 1.3] DPU probed"
echo "[ 1.4] unrelated system line"
EOT
chmod +x "$root/dmesg.sh"

PROC_ROOT=$p SYSFS_ROOT=$s DMESG_CMD=$root/dmesg.sh sh "$script" >"$root/out" 2>&1
[ $? -eq 0 ] && ok "exit 0" || bad "exit 0"

has "date section" "### date"
has "remoteproc name" "name: imx-rproc"
has "remoteproc state" "state: offline"
has "remoteproc firmware" "firmware: hello.elf"
has "dpu node" "/soc/display-controller@4b400000"
has "compatible" "compatible: fsl,imx95-dpu"
has "status" "status: okay"
has "reg hex" "reg: 00000000 4b400000 00000000 00100000"
has "reserved-memory node" "/reserved-memory/m7-vring"
has "scmi node" "/firmware/scmi"
hasnt "unrelated node skipped" "serial@1"
has "dmesg SCMI" "SCMI: SM version 1.2"
has "dmesg rproc" "imx-rproc: m7 ready"
has "dmesg DPU" "DPU probed"
hasnt "dmesg unrelated skipped" "unrelated system line"
has "clk dpu line" "dpu_pixel"
has "clk ldb line" "lvds_ldb"
hasnt "clk uart skipped" "uart"
has "genpd line" "dispmix on"
hasnt "genpd other skipped" "netcmix"
has "redaction marker" "<redacted>"
hasnt "ipv4 redacted" "$ip"
hasnt "mac redacted" "$mac"
hasnt "hostname redacted" "$host"
hasnt "serial redacted" "$ser"

# unreadable debugfs and dmesg
rm -f "$s/kernel/debug/clk/clk_summary"
PROC_ROOT=$p SYSFS_ROOT=$s DMESG_CMD=false sh "$script" >"$root/out" 2>&1
has "not readable reported" "not readable"

# the script must not redirect output to any file
if grep -v '^[[:space:]]*#' "$script" | sed 's/2>&1//g' | grep -q '>'; then
    bad "no file redirection"
else
    ok "no file redirection"
fi

[ "$fail" -eq 0 ] && echo "all passed" || echo "FAILURES"
exit "$fail"
