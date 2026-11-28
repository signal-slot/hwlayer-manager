# i.MX95 Zephyr (M2) evidence

This directory holds evidence for SoW milestone M2 (Zephyr on the i.MX95 M7).

- `observed-2026-11-27.txt`: a dated, sanitised, read-only snapshot of the
  Verdin iMX95 (Mallow carrier, LVDS panel, Toradex Reference Multimedia Image
  7.8.0, kernel 6.6.157-7.8.0) taken over ssh. The date is the board's UTC
  date. Everything in it is tagged **observed**. It covers:
  - (a) remoteproc devices, their state and firmware names, and the
    `firmware_class` search path;
  - (b) device-tree nodes for the display pipeline (DPU, pixel link, LDB,
    DSI, panel), the M7 remoteproc node, reserved memory and SCMI;
  - (c) dmesg lines for SCMI/System Manager, remoteproc, DPU, LDB and DRM, with
    the SM/SCMI version;
  - (d) clock and power-domain state for display and M7 entries;
  - (e) the `/lib/firmware` content (no M7 image is installed);
  - (f) `uname`, `os-release` and the state of Weston before and after.

The session only read files and wrote nothing on the board. Weston was active
at the start and at the end. The host name, addresses, serial numbers, MAC
addresses and tty paths are removed. The file's header says what was elided
and what could not be read.

- `observed-2026-11-28.txt`: a second read-only snapshot, run on 2026-11-27
  (board clock 23:00 UTC; the file name carries the planner's date so it does
  not collide with the first). Everything in it is **observed 2026-11-27**.
  It adds, for the open M2 questions:
  - (a) the `/proc/interrupts` lines for the DPU (`4b400000.display-controller`
    on an irqsteer domain, with counts), the pixel interleaver bridge, the
    mailbox/MU controllers and the GPU. The LDB and the remoteproc have no
    interrupt line (**not observed**);
  - (b) the `panel-lvds-native` node with its min/typ/max `panel-timing`
    (decoded in the file header: 1280x800, typ pixel clock 71100000 Hz) and
    the LDB, LVDS PHY, pixel-link and DPU nodes;
  - (c) `/sys/devices/soc0` (family, machine, revision 2.0, soc_id) and the
    dmesg SoC lines; the serial number is dropped;
  - (d) the SM facts Linux shows: SCMI v2.1 `NXP:IMX`, firmware version 0x23e,
    MISC 1.1 and BBM 1.0, and the SCMI drivers bound. There is no
    `/sys/kernel/debug/scmi*` and no sysfs entry for the SM configuration name,
    LM names or states: **not observed**;
  - (e) the remoteproc state and firmware names again (both offline), and an
    empty `firmware_class` path.

  The session wrote nothing on the board; Weston printed `active` before and
  after.

A snapshot says what the board does today. It does not say who could own the
DPU under a different System Manager configuration; that comes from source
reading and is in the feasibility document, not here.

## How these are made and read

Tools that make and read the snapshots:

- [`scripts/imx95-observe.sh`](../../scripts/imx95-observe.sh): the read-only
  board snapshot script that produces files of this kind.
- [`scripts/imx95-observe-summary.py`](../../scripts/imx95-observe-summary.py):
  turns a snapshot into a short ownership table.
- [`docs/imx95-m7-runbook.md`](../imx95-m7-runbook.md): how a board session
  uses the scripts.
- [`docs/imx95-zephyr-feasibility.md`](../imx95-zephyr-feasibility.md): the
  feasibility document that reads the snapshots together with the source
  studies.

Docs that cite these snapshots:

- [`docs/glossary.md`](../glossary.md)
- [`docs/imx95-display-power.md`](../imx95-display-power.md)
- [`docs/imx95-m7-fb-memory.md`](../imx95-m7-fb-memory.md)
- [`docs/imx95-m7-ldb-driver.md`](../imx95-m7-ldb-driver.md)
- [`docs/imx95-m7-reduced-scope.md`](../imx95-m7-reduced-scope.md)
- [`docs/imx95-m7-rpmsg-log.md`](../imx95-m7-rpmsg-log.md)
- [`docs/imx95-m7-rproc-load.md`](../imx95-m7-rproc-load.md)
- [`docs/open-issues.md`](../open-issues.md)
- [`docs/sow-traceability.md`](../sow-traceability.md)
- [`docs/wp5-evidence.md`](../wp5-evidence.md)

No `summary-*.md` file exists in this directory on this tip, so none is linked.
