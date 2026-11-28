# pending-probe-expect

`scripts/pending-probe-expect.py` gives a mechanical verdict on a saved
`pending_probe` log (`crates/composition-drm/examples/pending_probe.rs`): flip
and latch counts of an overlay commit (`--flips N` run) and what the kernel
did with a commit sent before the previous one's flip event (`--replace`
run). It serves HVS Q6 and RK3588 Q9 for the RPi5, reTerminal and NanoPC-T6
sessions. Python 3, standard library only.

## Use

```sh
scripts/pending-probe-expect.py LOG [--expect-replace accepted|ebusy] [--expect-flips N]
scripts/pending-probe-expect.test.sh   # self-test on synthetic logs
```

`LOG` is a file or `-` for stdin. Output (read from source: the line formats
come from the header of `pending_probe.rs`):

```text
PENDING commits=<n> flips=<n> latched=<n> missed=<n>
PENDING replace result=<accepted|ebusy|other errno|unknown>
EXPECT ok|fail ...
```

- `commits` is the `submitted` count of the `SUMMARY flips:` line, `flips`
  its `events` count, `missed` its `missed` count.
- `latched` is the number of `FLIP` lines whose commit was accepted and that
  carry an event sequence (a late event counts in `flips` but not here).
- `result` follows the real commit B: `accepted`, `ebusy`, any other errno as
  printed, or `unknown` when A itself was refused.
- `--expect-flips N` is `ok` only with `commits=N`, `flips=N`, `missed=0`.
- `--expect-replace` is `ok` only when `result` equals the given value.

A log with no `SUMMARY` (flips) or `VERDICT replace_pending` line is
truncated: the script counts from the `FLIP` and `A`/`B` lines, prints
`PENDING truncated=yes`, and exits 1 (`EXPECT fail` when an expectation was
given). Exit status: 0 ok, 1 fail or truncated, 2 input error (no
`pending_probe` lines, unreadable file, bad arguments).

## Limits

- The script judges what the probe printed, not the picture; use
  `scripts/camera-check.py` for a screen view.
- A vkms or other virtual log is portability evidence, never a hardware
  result.
- The self-test uses synthetic logs only (inferred from the source header; no
  board log was read).
