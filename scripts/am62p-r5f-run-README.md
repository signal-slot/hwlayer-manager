# am62p-r5f-run.sh

Runs an image on the Verdin AM62P MCU R5F through remoteproc and reads the
core's console from its trace buffer (`debugfs
remoteproc/remoteprocN/trace0`). It runs on the board itself, over ssh.

```
am62p-r5f-run.sh [--timeout SECS] [--run-secs SECS] [--interval SECS]
                 [--log FILE] IMAGE REMOTEPROC [STOPFILE]
am62p-r5f-run.sh restore STATEFILE
```

- `IMAGE` — the image to load. It must be a regular file under `/tmp`
  (anything else is refused). It is never copied or installed: it is loaded
  by pointing `firmware_class` path at its directory and writing its file
  name to the remoteproc `firmware` attribute.
- `REMOTEPROC` — index (e.g. `1`) or the contents of the remoteproc `name`
  file. The trace buffer used is `.../remoteprocN/trace0` of that core.
- `STOPFILE` — lines `stop UNIT` and `unbind DRIVER DEVICE`, `#` starts a
  comment. What the bring-up documentation lists as the things to stop or
  unbind is not in this repository, so it is passed as this file and never
  hard-coded here.
- `--run-secs` seconds to keep copying the trace buffer (default 60,
  `0` = no time limit), `--interval` seconds between copies (default 1),
  `--timeout` seconds to wait for `running` (default 10), `--log` the log
  file under `/tmp` (default `<state dir>/log`, overwritten per run).
  The capture ends early once a line starting with `RESULT` appears.

The start flow refuses an image outside `/tmp`, a malformed stop file, and a
core that is not offline — it never stops the core. It records the active
state of every listed unit and the bound state of every listed binding in a
state file (`/tmp/am62p-r5f-run/state` by default), stops and unbinds them,
sets the firmware name and firmware_class path, starts the core, copies new
trace-buffer lines into the log every `--interval` seconds (also printing
them as `trace: ...`, and new kernel lines as `dmesg: ...`), then restores
the firmware_class path and — when the core is offline — the firmware name.
The core is left running on purpose: a second stop fails on this board and
a second image needs a reboot, so when the core is still running the
firmware name is reported as `kept` instead of restored.

`restore STATEFILE` rebinds what the state file recorded as bound and starts
what it recorded as active, then removes the state file. Run it before
rebooting the board back to its normal boot chain; it is the only moment
units and bindings are touched again.

The script writes only under `/tmp` (state file, log, scratch), under
`${ROOT}/sys` (bind and unbind files, remoteproc `state` and `firmware`,
`firmware_class` path) and the units it was told to stop. It never touches
the U-Boot environment, the eMMC boot areas or packages, and needs no board
addresses or credentials.

Exit status: 0 ok (a capture that ran out of time without a `RESULT` line
is a normal end and prints `note: no RESULT line ...`); 1 failure (a stop,
unbind, start or trace failure; the core restore is still attempted);
2 usage or refused input; 3 refused, core not offline; 4 core restore
failed (firmware name or firmware_class path could not be restored while
the core was offline); 5 previous run not restored (run `restore` on the
state file first).

Test hooks (environment): `ROOT` (sysfs and debugfs prefix),
`RPROC_FW_ROOT` (allowed directory for image, log and state, default `/tmp`),
`AM62P_STATE_DIR`, `AM62P_TRACEFS`, `RPROC_POLL` and `RPROC_POLLS_PER_SEC`
(start-wait poll), `AM62P_TRACE_POLL` and `AM62P_TRACE_POLLS` (trace copy
interval and count), `SYSTEMCTL`, `DMESG`.

Test: `sh scripts/am62p-r5f-run.test.sh` (fake sysfs/debugfs tree and fake
systemctl under `$TMPDIR`; no board access).

## Trust checks

The state directory must be a real directory owned by the caller (it is set to
mode 700); a symlinked state directory, state file or log is refused. `restore`
refuses a state file or directory that is a symlink or not owned by the caller,
accepts driver paths only under `${ROOT}/sys/bus/*/drivers/*`, and accepts only
path-free unit and device names.
