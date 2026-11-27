# picture-hook.sh

Hook for `board-scenarios.sh --hook`: takes a picture a while into each hold
and judges it with `camera-check.py`. It is called as `picture-hook.sh ROW
PHASE`. It contains no device name, address or credential: the capture command
comes from the environment.

```sh
PICTURE_CAPTURE='<capture command from the lease>' \
PICTURE_DIR=out/pictures PICTURE_EXPECT_DIR=out/expect \
  sh scripts/board-scenarios.sh --hook scripts/picture-hook.sh HARNESS
```

## Environment

| Variable | Meaning |
| --- | --- |
| `PICTURE_CAPTURE` | required; run as `$PICTURE_CAPTURE OUTFILE`, must write OUTFILE |
| `PICTURE_DIR` | required; pictures and `pictures.log` go here |
| `PICTURE_DELAY` | seconds into a hold before the picture (default 20) |
| `PICTURE_EXPECT_DIR` | optional; `ROW.json` there is the camera-check EXPECT file for ROW |
| `PICTURE_ON_PHASES` | optional; other phases (space or comma separated) that get an immediate picture |
| `PICTURE_WAIT` | seconds `hold-end` waits for a running capture (default 120) |

## Phases

- `hold-start`: starts a background job that sleeps `PICTURE_DELAY`, captures to
  `PICTURE_DIR/ROW-hold-start-<n>.png` and judges it. Returns 0 at once.
- `hold-end`: if the job has not begun its capture, the hold was shorter than
  the delay: the job is cancelled (it can never fire later), `picture missed`
  is logged, exit 1. Otherwise it waits for the job and returns its status.
- Phases in `PICTURE_ON_PHASES`: immediate picture, `ROW-PHASE-<n>.png`.
- Any other phase: nothing.

## pictures.log

One line per picture, appended: `ROW PHASE UTC-TIME FILE VERDICT`. FILE is `-`
when there is no picture. VERDICT (the rest of the line) is `PASS`, `FAIL`,
`not judged` (no EXPECT file), `error (camera-check status N)`,
`capture failed (status N)` or `picture missed (...)`. The camera-check output
is kept next to the picture as `*.check.txt`.

## Exit status

0: started, or picture taken and PASS or not judged. 1: missed, capture failed,
FAIL or camera-check error (board-scenarios.sh only notes it). 2: usage or
environment error.

The picture counter is shared across rows in `PICTURE_DIR`, so rows must run one
after another. Test: `sh scripts/picture-hook.test.sh`.
