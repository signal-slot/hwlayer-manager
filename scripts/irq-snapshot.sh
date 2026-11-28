#!/bin/sh
# irq-snapshot.sh - filter and diff display/DPU/irqsteer lines of
# /proc/interrupts captures.
#
# Only parses text files; it never reads the board itself (capture with
# 'cat /proc/interrupts > file'). See scripts/irq-snapshot-README.md.
#
# Usage: sh scripts/irq-snapshot.sh [--match REGEX] BEFORE.txt [AFTER.txt]
set -eu

# Default: case-insensitive ERE for the display-related names of
# docs/imx95-m7-dpu-irq.md. 'dc' only as a whole word (dc0, dc-xx).
DEFAULT_RE='irqsteer|dpu|display|lvds|ldb|seerisr|dsi|(^|[^a-z])dc([^a-z]|$)'
RE=$DEFAULT_RE

usage() {
    echo "usage: sh $0 [--match REGEX] BEFORE.txt [AFTER.txt]" >&2
    exit 2
}

while [ $# -gt 0 ]; do
    case $1 in
        --match) [ $# -ge 2 ] || usage; RE=$2; shift 2 ;;
        --match=*) RE=${1#--match=}; shift ;;
        -h|--help) usage ;;
        --) shift; break ;;
        -*) usage ;;
        *) break ;;
    esac
done
[ $# -ge 1 ] && [ $# -le 2 ] || usage
for f in "$@"; do
    [ -r "$f" ] || { echo "irq-snapshot: cannot read $f" >&2; exit 2; }
done

IRQ_RE=$RE awk '
function parse(file, idx,    line, f, nf, st, ncpu, i, lab, sum, rest, p, name, chip, hw, t) {
    ncpu = 0
    while ((getline line < file) > 0) {
        if (line ~ /^#/) {
            if (line ~ /^# *t=/) {
                t = line; sub(/^# *t=/, "", t); sub(/[ \t].*$/, "", t)
                T[idx] = t + 0; HAST[idx] = 1
            }
            continue
        }
        nf = split(line, f, /[ \t]+/)
        st = (f[1] == "") ? 2 : 1
        if (f[st] ~ /^CPU[0-9]+$/) {
            ncpu = 0
            for (i = st; i <= nf; i++) if (f[i] ~ /^CPU[0-9]+$/) ncpu++
            continue
        }
        lab = f[st]
        if (ncpu == 0 || lab !~ /:$/ || f[st + 1] !~ /^[0-9]+$/) continue
        sub(/:$/, "", lab)
        sum = 0
        for (i = 1; i <= ncpu; i++) {
            if (f[st + i] !~ /^[0-9]+$/) break
            sum += f[st + i]
        }
        if (tolower(line) !~ RE_) continue
        rest = ""
        for (i = st + 1 + ncpu; i <= nf; i++) rest = rest (rest == "" ? "" : " ") f[i]
        chip = "-"; hw = "-"
        if (lab ~ /^[0-9]+$/) {
            # N: counts chip hwirq [type] name
            split(rest, p, " ")
            if (p[1] != "") chip = p[1]
            if (p[2] != "") hw = p[2]
            name = ""
            for (i = 3; i in p; i++) {
                if (i == 3 && p[i] ~ /^(Level|Edge|Fasteoi|Bad)$/) continue
                name = name (name == "" ? "" : " ") p[i]
            }
            if (name == "") name = "-"
        } else {
            # IPI0:, Err: ... have no chip column
            name = lab (rest == "" ? "" : " " rest)
        }
        CNT[idx, lab] = sum
        if (idx == last && !(lab in ORD)) {
            ORD[lab] = 1; order[++no] = lab
            CHIP[lab] = chip; HW[lab] = hw; NAME[lab] = name
        }
    }
    close(file)
}
BEGIN {
    RE_ = tolower(ENVIRON["IRQ_RE"])
    last = ARGC - 1
    for (n = 1; n <= last; n++) parse(ARGV[n], n)
    dt = ""
    if (last == 2 && HAST[1] && HAST[2]) dt = T[2] - T[1]
    if (last == 2) printf "# interval_s=%s\n", (dt == "" ? "unknown" : dt)
    if (last == 1) print "IRQ SUM CHIP HWIRQ NAME"
    else print "IRQ SUM DELTA CHIP HWIRQ NAME"
    for (k = 1; k <= no; k++) {
        key = order[k]
        if (last == 1) {
            printf "%s %d %s %s %s\n", key, CNT[1, key], CHIP[key], HW[key], NAME[key]
        } else {
            if ((1, key) in CNT) d = CNT[2, key] - CNT[1, key]; else d = "new"
            printf "%s %d %s %s %s %s", key, CNT[2, key], d, CHIP[key], HW[key], NAME[key]
            if (dt != "" && dt > 0 && d != "new") printf " (%.2f/s)", d / dt
            printf "\n"
        }
    }
    printf "IRQ-LINES n=%d\n", no
    st = 0
    while ((getline l < ARGV[last]) > 0)
        if (l !~ /^#/ && tolower(l) ~ /irqsteer/) st = 1
    print (st ? "IRQSTEER present" : "IRQSTEER absent")
    exit
}' "$@"
