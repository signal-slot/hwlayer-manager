# Trace sample logs

Fixed inputs for the strict trace parser and the board-scenarios trace-count
check (X-03). **Synthetic**: written by hand to follow the grammar of
`docs/trace-format.md` §3 and the engine's behaviour. Nothing here is a board
measurement; the ids, times and `build="sample"` are invented. No file names a
board, host, address or serial.

Tags: **read from source** (cited `file:line` at the commit these were
written on), **inferred** (a decision of this item, which the parser's owner
may revisit). The harness lines (`harness`, `session`, `VERDICT`, ...) are
ordinary harness output (`tools/drm-harness/README.md`); the parser must
ignore any line that does not start with `CETRACE` (`trace-format.md` §3).
`scenario=` and `--trace` in them are the planned names, not yet implemented.

Counts are over `commit result=ok`, `latched`, `released`, `dropped` lines.
A `commit` with `result=<error>` is a refused commit and is not counted.

## Relations (R1-R5)

| Id | Relation | Source |
| --- | --- | --- |
| R1 | Each submission shows at most `layers` buffers, so released lines <= sum of `layers` of the committed frames. Stronger: a `released sub=S` needs an earlier `commit ok sub=S`. (`sub=-` only follows device loss.) | **read from source**: a latch releases the buffers of the frame it takes off screen, `presentation.rs:403-425`; a replaced submission releases its unused buffers, `presentation.rs:389-395`; reset releases each in-use buffer once, `presentation.rs:435-466`. The `layers`-sum form is **inferred** (the commit line carries no buffer ids). |
| R2 | Every `commit ok` sub ends in exactly one of `latched` or `dropped`, or is still pending at the end of the log; a `released sub=S` (not `-`) needs S latched-and-replaced or dropped. | **read from source**: replacing a pending submission emits `Dropped`, `presentation.rs:389-394`; a latch needs the pending id, `presentation.rs:411-413`; trace attribution `engine.rs:472-504`. "Pending at the end allowed" is `trace-format.md` §7 (**read from source**, a proposal). |
| R3 | `ev=` is one of `hdr check validate commit latched released dropped latch_timeout device_lost caps_changed`. | **read from source**: nine `TraceEvent` variants, `trace.rs:37-103`, plus `hdr` from `trace-format.md` §3.2. |
| R4 | A line starting with `CETRACE` matches the grammar (`key=value` words, numeric fields are `uint`). | `trace-format.md` §3.2 (proposal). |
| R5 | Only known versions are accepted. | `trace-format.md` §3.1 (proposal). |

Other **read from source** facts the consistent logs follow: a device loss
emits `Dropped` for pending submissions, then `Released` for every buffer in
use with no submission, then `DeviceLost` (`presentation.rs:435-466`,
`engine.rs:513-520`); a commit after the loss fails with
`BackendError::DeviceLost` (`engine.rs:314`, `:370`); a capability change
makes an earlier plan `StalePlan` (`engine.rs:296-300`, `:528-532`) and the
`Latched`-time release is attributed to the submission taken off screen
(`engine.rs:454-461`). The first latch on an output releases nothing.

## Files

| File | Scenario | commit | latched | released | dropped | Verdict | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `hold.log` | hold (A1) | 1 | 1 | 0 | 0 | accept | frame stays on screen, so no release (**read from source**: `presentation.rs:420-424`). Carries `future_field=1` on one line: an unknown `key=value` is ignored within a known version (`trace-format.md` §3.1, **read from source**). |
| `stop.log` | stop (A3) | 3 | 2 | 1 | 0 | accept | sub 3 pending when SIGTERM arrives; allowed at the end (**inferred** from §7). The harness lines show the intended post-fix output (`resources`, `VERDICT`). |
| `revoke.log` | revoke (A4) | 3 | 2 | 3 | 1 | accept | 1 release by latch + 2 `sub=-` by loss; one `dropped reason=device_lost`; one refused commit. |
| `modeset.log` | modeset (A5) | 2 | 2 | 1 | 0 | accept | `caps_changed` then a `stale_plan` refusal then re-validate; the stale commit is not counted. The exact sequence a real modeset emits is **inferred**. |
| `recovery.log` | recovery (A6) | 3 | 2 | 1 | 1 | accept | two runs; a second `hdr` (`seq=0`) starts run 2, so state (pending, on-screen, `seq`) restarts. Per run: run 1 = 2/1/1/1, run 2 = 1/1/0/0. Treating `hdr` as a run boundary is **inferred**. |
| `bad-released-exceeds-commits.log` | - | 2 | 2 | 3 | 0 | reject | breaks R1: 1-layer frames, 3 releases > 2 commits. |
| `bad-missing-latch.log` | - | 2 | 1 | 1 | 0 | reject | breaks R2: sub 1 neither latched nor dropped, yet released. |
| `bad-unknown-event.log` | - | 1 | 1 | 0 | 0 | reject | breaks R3 (`ev=frobnicate`); counts are those if the line were skipped. |
| `bad-malformed-line.log` | - | 0 | 1 | 0 | 0 | reject | breaks R4 (`t=abc`); counts if the line were skipped (the commit is lost). |
| `bad-unknown-version.log` | - | 0 | 1 | 0 | 0 | reject | breaks R5 (`v=2`); counts if skipped. Added beyond the four requested because §3.1 requires it. |

A strict parser must reject each `bad-*` file as a whole (exit non-zero, name
the line for R3-R5), not count around the bad line; the "if skipped" counts
are only for a test that checks the parser did not silently do so.

Not covered (listed for the parser's tests, no sample): a `seq` gap
(`trace-format.md` §3.3), `latch_timeout` (`trace.rs:93`), `cyc32` wrap.
