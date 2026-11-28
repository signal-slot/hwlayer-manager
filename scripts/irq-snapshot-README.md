# irq-snapshot.sh

Filters and diffs the display-related lines of two `/proc/interrupts`
captures (DPU, irqsteer, display, LVDS/LDB, seerisr, DSI, dc), for the M2-09
read of the A55 interrupt lines (names from `docs/imx95-m7-dpu-irq.md`).

**The tool only parses text.** It does not touch a board. Capturing on the
board is `cat /proc/interrupts` (read-only; it never touches remoteproc or
the M7). Optionally put a `# t=<seconds>` line (e.g. from `date +%s`) at the
top of each capture to get the interval and a per-second rate.

```sh
# on the board, a few seconds apart while the display is running:
{ echo "# t=$(date +%s)"; cat /proc/interrupts; } > before.txt
sleep 5
{ echo "# t=$(date +%s)"; cat /proc/interrupts; } > after.txt

sh scripts/irq-snapshot.sh [--match REGEX] before.txt [after.txt]
```

* `--match REGEX`: case-insensitive extended regex applied to the whole line
  (default: `irqsteer|dpu|display|lvds|ldb|seerisr|dsi|dc` as a word).
* One file: `IRQ SUM CHIP HWIRQ NAME`. Two files: `IRQ SUM DELTA CHIP HWIRQ
  NAME`, `SUM` being the per-CPU count sum of the second file, `DELTA` the
  difference (`new` if the line is absent from the first file), plus
  `(<n>/s)` and `# interval_s=` when both files carry `# t=`.
* Lines without a chip column (`IPI0:`, `Err:`) are handled; their name is the
  label plus the description and CHIP/HWIRQ are `-`.
* Ends with `IRQ-LINES n=<count>` and `IRQSTEER present|absent` (the latter
  looks at the whole last capture, not only the `--match` result).
* Exit 2 on usage errors or unreadable files.

Test: `sh scripts/irq-snapshot.test.sh` (synthetic, invented fixtures written
under `$TMPDIR`). Passing it says nothing about any hardware interrupt.
