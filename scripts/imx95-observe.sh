#!/bin/sh
# Read-only snapshot of an i.MX95 board: remoteproc, device tree, dmesg, clk/genpd.
# Run it ON the board (e.g. `ssh <board> sh -s < scripts/imx95-observe.sh`); it
# writes one text report to stdout and changes nothing: no file is written, no
# service touched, no device node opened.  Address-like values, the hostname and
# serial numbers are replaced by <redacted> before anything is printed.
#
# Roots can be overridden so the host test can run it on a fake tree:
#   PROC_ROOT  (default /proc)   SYSFS_ROOT (default /sys)
#   DMESG_CMD  (default dmesg)
# debugfs is read from $SYSFS_ROOT/kernel/debug.

PROC_ROOT=${PROC_ROOT:-/proc}
SYSFS_ROOT=${SYSFS_ROOT:-/sys}
DMESG_CMD=${DMESG_CMD:-dmesg}

DT_RE='display|dpu|ldb|lvds|dsi|remoteproc|imx_rproc|reserved-memory|scmi'
# "sm" only as a word, otherwise it matches "system" and the like.
DMESG_RE='(^|[^a-z])sm([^a-z]|$)|scmi|rproc|dpu'
CLK_RE='disp|dpu|ldb|lvds|dsi|pix'

# Print a DT property as text (NUL -> space), empty when absent.
dt_text() {
    [ -r "$1" ] || return 0
    tr '\000' ' ' <"$1"
}

# Print a DT property as hex, grouped in 32-bit cells.
dt_hex() {
    [ -r "$1" ] || return 0
    od -An -v -tx1 <"$1" | tr -d ' \n' | sed 's/\([0-9a-f]\{8\}\)/\1 /g; s/ $//'
    echo
}

report() {
    echo "# imx95-observe: read-only snapshot"
    echo "### date"
    date -u
    echo "### uname -r"
    uname -r

    echo "### remoteproc"
    found=0
    for d in "$SYSFS_ROOT"/class/remoteproc/*; do
        [ -d "$d" ] || continue
        found=1
        echo "$d"
        for a in name state firmware; do
            v=
            [ -r "$d/$a" ] && v=$(cat "$d/$a")
            echo "  $a: $v"
        done
    done
    [ "$found" = 1 ] || echo "none"

    echo "### device tree"
    dt=$PROC_ROOT/device-tree
    if [ -d "$dt" ]; then
        find "$dt/" -type d | sort | while IFS= read -r n; do
            n=${n%/}
            rel=${n#$dt}
            comp=$(dt_text "$n/compatible")
            if printf '%s %s\n' "$rel" "$comp" | grep -Eiq "$DT_RE"; then
                echo "$rel"
                echo "  compatible: $comp"
                echo "  status: $(dt_text "$n/status")"
                echo "  reg: $(dt_hex "$n/reg")"
            fi
        done
    else
        echo "not readable"
    fi

    echo "### dmesg (SM/SCMI/rproc/DPU)"
    if out=$($DMESG_CMD 2>&1); then
        printf '%s\n' "$out" | grep -Ei "$DMESG_RE"
    else
        echo "not readable"
    fi

    for f in clk/clk_summary pm_genpd/pm_genpd_summary; do
        echo "### debugfs $f (disp/dpu/ldb/lvds/dsi/pix lines)"
        p=$SYSFS_ROOT/kernel/debug/$f
        if [ -r "$p" ] && out=$(cat "$p" 2>&1); then
            printf '%s\n' "$out" | grep -Ei "$CLK_RE"
        else
            echo "not readable"
        fi
    done
}

# Values to redact besides the address patterns.
host=
[ -r "$PROC_ROOT/sys/kernel/hostname" ] && host=$(cat "$PROC_ROOT/sys/kernel/hostname")
[ -n "$host" ] || host=$(uname -n)
serial=$(dt_text "$PROC_ROOT/device-tree/serial-number" | tr -d ' ')

esc() { printf '%s' "$1" | sed 's/[][\.*^$/]/\\&/g'; }

# Literal values (hostname, serial) are redacted in a second sed pass.
literals() {
    expr=
    for v in "$host" "$serial"; do
        [ -n "$v" ] && expr="$expr -e s/$(esc "$v")/<redacted>/g"
    done
    if [ -n "$expr" ]; then
        # shellcheck disable=SC2086
        sed $expr
    else
        cat
    fi
}

filter() {
    sed -e 's/[0-9]\{1,3\}\(\.[0-9]\{1,3\}\)\{3\}/<redacted>/g' \
        -e 's/[0-9A-Fa-f]\{2\}\(:[0-9A-Fa-f]\{2\}\)\{5\}/<redacted>/g' \
        -e 's/[0-9A-Fa-f]\{2\}\(-[0-9A-Fa-f]\{2\}\)\{5\}/<redacted>/g' \
        -e 's/\([Ss]erial[ _-]*\([Nn]umber\|[Nn]o\)\{0,1\}[ ]*[:=][ ]*\)[^ ]*/\1<redacted>/g' |
        literals
}

report 2>&1 | filter
