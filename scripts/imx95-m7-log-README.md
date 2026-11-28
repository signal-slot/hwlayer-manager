# imx95-m7-log.sh

A board-side helper that captures the log of the i.MX95 Cortex-M7 into a file under `/tmp`: the rpmsg tty for a given time, then the remoteproc trace buffer if there is one. It reads the log only; it does not load, start or stop the M7 (see `docs/imx95-m7-runbook.md` for that). It is a POSIX `sh` script that needs only `cat`, `date`, `sleep` and `kill`.

## Run it over ssh

The script is piped to a remote `sh`, so nothing is installed on the board:

```sh
ssh <board> sh -s -- 10 /tmp/m7-log.txt < scripts/imx95-m7-log.sh
```

`<board>` is the ssh destination from the lease. No address, host name or user belongs in this file or in a commit. Fetch the result the same way, for example `ssh <board> cat /tmp/m7-log.txt > m7-log.txt`.

## Usage

```
imx95-m7-log.sh SECS OUTFILE
```

| Argument | Meaning |
| --- | --- |
| `SECS` | Seconds to read the rpmsg tty; a positive integer. |
| `OUTFILE` | File to create. Must start with `/tmp/` and contain no `..` part. An existing non-regular file is refused. |

The tty is found by globbing `ttyRPMSG*` under the device directory and taking the first match, so the script holds no fixed tty path.

## Output

`OUTFILE` is created (or truncated) with, in order:

1. `# date: YYYY-MM-DD`, the first line, as `docs/trace-format.md` requires of a real trace log.
2. The bytes read from the rpmsg tty for `SECS` seconds. Reading stops early if the tty closes.
3. For each remoteproc trace file found under debugfs, a `# remoteproc trace: <name>` comment line followed by its contents. Nothing is added if debugfs has none or is not readable.

stdout gets `OK log <OUTFILE>`. Failures print `FAIL <what>: ...` on stderr. The date line is the board's clock at capture time. The file is a log of what the M7 printed, not a measurement by itself. Dating and tagging it in a doc is the caller's job (see `docs/trace-format.md` and `docs/imx95-m7-rpmsg-log.md`).

| Exit | Meaning |
| --- | --- |
| 0 | Log written. |
| 1 | No `ttyRPMSG*` device found (is the M7 running with an rpmsg tty?). No file is created. |
| 2 | Usage error, bad duration, or refused or unwritable output path. |

## Board rules

- The only thing written is `OUTFILE`, under `/tmp`.
- No module is loaded, and no tty setting, sysfs file or remoteproc state is changed.
- It does not stop Weston, touch the display, or write the rootfs, U-Boot environment or boot partitions.

## Test hooks

`SYSFS_ROOT` (default `/sys`), `DEV_ROOT` (default `/dev`) and `LOG_ROOT` (the allowed output prefix, default `/tmp`) let the host test point the script at a fake tree. Leave them unset on a board.

## Host test

```sh
bash scripts/imx95-m7-log.test.sh
```

It builds a fake sysfs and device tree under `$TMPDIR` and checks:

- a capture succeeds and starts with the date line, and the tty and trace contents are present;
- a missing rpmsg device exits 1 with a clear message and leaves no file;
- an output path outside `/tmp`, one with `..`, and a non-numeric duration all exit 2;
- the script contains no fixed tty path or IP-like address.

It exits non-zero on any failure.

## Limits

- The script has been tested on the host against fake trees only. Real rpmsg tty behaviour (blocking reads, tty line settings) is not shown by the test. Not measured on a board by this README.
- The tty is read with `cat` as the kernel presents it. No baud, framing or line-discipline setting is applied or restored.
- Only the first `ttyRPMSG*` is read. With several rpmsg endpoints the choice follows glob order.
- The duration is counted in whole seconds of `sleep`, so it is approximate. Data the M7 sent before the capture began is not in the file.
- Trace buffers are read once, after the tty capture. They are a snapshot, not a stream.
- It does not parse trace-format lines; use the trace tools described in `docs/trace-format.md` for that.
