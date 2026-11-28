#!/bin/sh
# Self-test for irq-snapshot.sh with SYNTHETIC fixtures (invented numbers and
# names, not a board capture). Run: sh scripts/irq-snapshot.test.sh
set -eu

here=$(cd "$(dirname "$0")" && pwd)
S=$here/irq-snapshot.sh
: "${TMPDIR:?TMPDIR must be set}"
T=$(mktemp -d "$TMPDIR/irqsnap.XXXXXX")
trap 'rm -rf "$T"' EXIT INT TERM

fail=0
check() { # desc, haystack, needle (fixed string)
    case $2 in
        *"$3"*) ;;
        *) echo "FAIL: $1: missing '$3'"; echo "$2"; fail=1 ;;
    esac
}
absent() {
    case $2 in
        *"$3"*) echo "FAIL: $1: unexpected '$3'"; fail=1 ;;
    esac
}

# SYNTHETIC: invented 6-CPU arm64 layout, not from any board.
cat > "$T/a.txt" <<'EOF'
# SYNTHETIC FIXTURE - invented data
# t=100
           CPU0       CPU1       CPU2       CPU3       CPU4       CPU5
 11:       1000       1000       1000       1000       1000       1000     GICv3  27 Level     arch_timer
 40:         10          0          0          0          0          0  irqsteer-demo   3 Level     dpu-demo-event
 41:          5          5          0          0          0          0  irqsteer-demo  70 Edge      display-demo-vsync
 42:          1          0          0          0          0          0     GICv3 300 Level     eth-demo
 43:          7          0          0          0          0          0     GICv3 301 Level     ldb-lvds-demo
IPI0:        20         20         20         20         20         20       Rescheduling interrupts
Err:          0
EOF
cat > "$T/b.txt" <<'EOF'
# SYNTHETIC FIXTURE - invented data
# t=104
           CPU0       CPU1       CPU2       CPU3       CPU4       CPU5
 11:       2000       2000       2000       2000       2000       2000     GICv3  27 Level     arch_timer
 40:         10          0          0          0          0          0  irqsteer-demo   3 Level     dpu-demo-event
 41:        125        125          0          0          0          0  irqsteer-demo  70 Edge      display-demo-vsync
 42:          9          0          0          0          0          0     GICv3 300 Level     eth-demo
 43:          7          0          0          0          0          0     GICv3 301 Level     ldb-lvds-demo
 44:          3          0          0          0          0          0     GICv3 302 Level     seerisr-demo
IPI0:        30         30         30         30         30         30       Rescheduling interrupts
Err:          0
EOF
# Capture without any matching line.
cat > "$T/none.txt" <<'EOF'
# SYNTHETIC FIXTURE - invented data
           CPU0       CPU1
 11:         10         10     GICv3  27 Level     arch_timer
Err:          0
EOF

# 1. single file
out=$(sh "$S" "$T/a.txt")
check single "$out" "40 10 irqsteer-demo 3 dpu-demo-event"
check single "$out" "41 10 irqsteer-demo 70 display-demo-vsync"
check single "$out" "43 7 GICv3 301 ldb-lvds-demo"
check single "$out" "IRQ-LINES n=3"
check single "$out" "IRQSTEER present"
absent single "$out" "arch_timer"
absent single "$out" "eth-demo"

# 2. two files: deltas, interval, rate, new line
out=$(sh "$S" "$T/a.txt" "$T/b.txt")
check diff "$out" "# interval_s=4"
check diff "$out" "40 10 0 irqsteer-demo 3 dpu-demo-event"
check diff "$out" "41 250 240 irqsteer-demo 70 display-demo-vsync (60.00/s)"
check diff "$out" "43 7 0 GICv3 301 ldb-lvds-demo"
check diff "$out" "44 3 new GICv3 302 seerisr-demo"
check diff "$out" "IRQ-LINES n=4"
check diff "$out" "IRQSTEER present"

# 3. custom match, including chip-less lines (IPI, Err)
out=$(sh "$S" --match 'ipi|^err|Err:' "$T/a.txt" "$T/b.txt")
check match "$out" "IPI0 120 60 - - IPI0 Rescheduling interrupts (15.00/s)"
check match "$out" "Err 0 0 - - Err"
check match "$out" "IRQ-LINES n=2"
check match "$out" "IRQSTEER present"   # reported on the whole capture
out=$(sh "$S" --match 'arch_timer' "$T/a.txt" "$T/b.txt")
check match2 "$out" "11 12000 6000 GICv3 27 arch_timer"

# 4. nothing matches
out=$(sh "$S" "$T/none.txt")
check none "$out" "IRQ-LINES n=0"
check none "$out" "IRQSTEER absent"

# 5. no t= comment: interval unknown, no rate
sed '/^# t=/d' "$T/b.txt" > "$T/b2.txt"
out=$(sh "$S" "$T/a.txt" "$T/b2.txt")
check not "$out" "# interval_s=unknown"
absent not "$out" "/s)"

# 6. usage errors
if sh "$S" >/dev/null 2>&1; then echo "FAIL: no args accepted"; fail=1; fi
if sh "$S" "$T/missing.txt" >/dev/null 2>&1; then echo "FAIL: missing file accepted"; fail=1; fi

if [ "$fail" -eq 0 ]; then echo "irq-snapshot.test.sh: ok"; fi
exit "$fail"
