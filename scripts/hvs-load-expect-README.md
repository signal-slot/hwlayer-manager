# hvs-load-expect

Judges the `STEP` lines printed by the `hvs_load_probe` example: the first
rung of the load ladder where the display drops frames or counts HVS
underruns while atomic `TEST_ONLY` still passes (`docs/design/hvs-backend.md`
§11 Q2). Python 3, standard library only. The input line formats below are
read from source: [main.rs](../crates/composition-drm/examples/hvs_load_probe/main.rs)
and [debugfs.rs](../crates/composition-drm/examples/hvs_load_probe/debugfs.rs).

This is a judge of text. It never opens a device. A log from a host run or a
vkms VM run exercises the tool and the probe's control flow only: no hardware
result comes from it. Only a log taken on the board itself, with debugfs
readable, says anything about the HVS.

```sh
python3 -I scripts/hvs-load-expect.py load.log
sudo ./hvs_load_probe /dev/dri/card0 --frames 120 | \
    python3 -I scripts/hvs-load-expect.py -
```

Several files are read as one log; with no file, or `-`, stdin is read.

## Input

One `STEP` line per ladder rung (one line in the log):

```
STEP n=3 layers=2 size=720x1280 src=1440x2560 scale=0.5000 test_only=ok commit=ok latched=120/120 late=1 missing=0 underrun_before=5 underrun_after=7 dlist_words=21 dlist=0:21,1:off,2:off
```

- `size` is each layer's destination size, `src` its source size, `scale`
  destination over source (0.5 = a 2x downscale).
- `test_only` is `ok`, `err:<errno>` or `skipped:<why>` (a buffer could not
  be made).
- `commit` is the step's first real commit: `ok`, `err:<errno>` or `not-run`.
- `latched=a/b`: page flips that arrived of the `b` frames asked for; `late`:
  flips more than 1.5 refresh periods after the previous one (kernel
  timestamps); `missing`: frames whose commit was refused or whose flip did
  not come within one second.
- `underrun_before` / `underrun_after`: debugfs `hvs_underrun` around the
  step, `na` when debugfs was unreadable. `dlist_words` / `dlist`: words of
  the active display lists from `hvs_dlists`, summed and per channel.

The log must end with `VERDICT hvs_load_probe: DONE`; without it the run is
treated as incomplete. Other probe lines (`DATE`, `PLANE`, `LADDER`,
`DEBUGFS`, `NOTE`, `RESTORE`) are ignored; `SUMMARY` is cross-checked.

## What it judges

A step **drops** when `test_only=ok` and at least one of:

- `underrun_after > underrun_before` (`underrun:+<d>`),
- `late > 0` (`late:<n>`),
- `missing > 0` (`missing:<n>`),
- `commit=err:<errno>` (`commit:err:<errno>`: the kernel refused for real a
  state it accepted in `TEST_ONLY`).

A step rejected by `TEST_ONLY` or skipped never drops, whatever its counters
say. Output:

```
STEPS total=4 test_only_ok=3 test_only_err=1 skipped=0 drop_steps=2
DLIST max_words=21
FIRST_REJECT step=2 layers=1 size=360x640 src=180x320 scale=2.0000 errno=22
DROP step=3 layers=2 size=720x1280 src=1440x2560 scale=0.5000 reasons=underrun:+2,late:1
DROP step=4 layers=3 size=720x1280 src=1440x2560 scale=0.5000 reasons=missing:120,commit:err:22
FOUND first_drop step=3 layers=2 size=720x1280 src=1440x2560 scale=0.5000 reasons=underrun:+2,late:1
```

or `NOT_FOUND` as the last line when no step dropped.

- `UNDERRUN unavailable (...)` is printed when no step has both underrun
  counts. A `NOT_FOUND` then rests on the flip counts only; say so when
  quoting it.
- `FIRST_REJECT none` when `TEST_ONLY` accepted every step tried.
- If the probe's `SUMMARY first_drop=` differs from the judged first drop, a
  `NOTE probe SUMMARY first_drop=<x>, judged <y>` line is printed; it does not
  change the result.
- `hvs_underrun` is one number for the whole HVS, not one per CRTC, so with
  a second output active an underrun on that output would count too
  (inferred from the file's format; not checked on a board).

## Exit status

| exit | meaning |
|---|---|
| 0 | judged: `FOUND` or `NOT_FOUND` |
| 2 | usage or input error: unreadable file, malformed `STEP` line, missing key, duplicate step number, no `STEP` line, or no `VERDICT hvs_load_probe: DONE` line |

Self-test: `bash scripts/hvs-load-expect.test.sh` (synthetic logs inline,
no board data). The probe's own parser is tested on the host by
`cargo test -p composition-drm --test hvs_load_parse`, against the recorded
BCM2711 debugfs dumps in `crates/composition-drm/tests/data/bcm2711/`.
