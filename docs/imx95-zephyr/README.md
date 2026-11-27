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

A snapshot says what the board does today. It does not say who could own the
DPU under a different System Manager configuration; that comes from source
reading and is in the feasibility document, not here.
