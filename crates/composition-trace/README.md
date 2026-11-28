# composition-trace

A formatter and a strict parser for the text line format `CETRACE v=1`, which
carries the structured traces of `composition-core` (`Trace`, `TraceEvent`,
`DropReason`). The format is specified in [docs/trace-format.md](../../docs/trace-format.md).
Example line (**read from source**, `src/lib.rs`):

```text
CETRACE v=1 ev=commit seq=3 t=1000300000 at=1000200000 out=0 result=ok sub=1
```

Tags used below: **read from source** (code read in this repository at the
commit that holds this file), **observed** (a command run and its output seen).

## What it does

**Read from source** (`src/lib.rs`, `Cargo.toml`): the crate is `no_std`,
allocation-free, has `#![forbid(unsafe_code)]`, and depends only on
`composition-core` (`composition-fake` is a dev-dependency). One trace entry is
one ASCII line. The crate formats entries into lines, parses lines back into
typed records, and refuses anything the format does not allow.

## Public API

**Read from source** (`src/lib.rs`, `src/format.rs`, `src/parse.rs`, `src/sink.rs`):

| Item | What it is |
|---|---|
| `PREFIX` (`"CETRACE"`), `VERSION` (`1`) | The token every line starts with; the only version written and parsed. |
| `write_header(w, seq, &Header)` | Writes the `hdr` line into any `core::fmt::Write`. Fails without writing if `freq_hz` is 0, `at_unit` is not one word, or `build`/`board` hold `"` or non-printable-ASCII. |
| `write_event(w, &Stamp, &TraceEvent)` | Writes the line of one entry. Fails without writing if `tev` is set on an event other than `Latched`, or `dur` on one other than `Validate`/`Commit`. |
| `parse_line(&str) -> Result<Option<Record>, ParseError>` | Parses one line (optional trailing `\n` or `\r\n`). |
| `Record::{Header, Event}` | A parsed line: the `hdr` line (`seq`, `Header`) or an entry (`Stamp`, `Event`). |
| `Event` | Owned form of `TraceEvent`: `Check`, `Validate`, `Commit`, `Latched`, `Released`, `Dropped`, `LatchTimeout`, `DeviceLost`, `CapabilitiesChanged`; `as_trace_event()` borrows it back as the core's type. |
| `Header`, `Stamp` | The `hdr` fields (`src`, `clock`, `freq_hz`, `t0`, `at_unit`, `build`, `board`) and the common fields (`seq`, `t`, `at`, optional `tev`, `dur`). |
| `Source` (`Linux`, `Zephyr`), `Clock` | The `src=` and `clock=` values, each with `as_str()`. |
| `Assignments` | `FixedVec<Assignment, MAX_LAYERS>`, the owned assignments of a parsed line. |
| `ParseError { kind, offset }`, `ErrorKind` | Why a line was refused and the byte offset: `Syntax`, `TrailingGarbage`, `UnknownVersion`, `UnknownEvent`, `UnknownKey`, `UnexpectedKey`, `DuplicateKey`, `MissingField`, `BadNumber`, `BadValue`, `CountMismatch`, `TooManyAssignments`. |
| `LineTrace<W, C>` | A `composition_core::Trace` sink that formats every entry into a writer; `C: FnMut() -> u64` supplies `t`. Methods: `new`, `header`, `seq`, `failed`, `writer`, `writer_mut`, `into_inner`. Attach with `Engine::with_trace`. It cannot fill `tev` or `dur`, since the core's entries carry neither. |

`LineTrace` counts every line it hands to the writer in `seq`, including the
header and lines that failed, so a failed write shows as a gap in `seq`;
`failed()` counts them.

## Parsing a board log

**Read from source** (`parse_line` docs, `tools/drm-harness/src/trace.rs`
`count`): a log may mix trace lines with other console output. The consumer
should:

1. decode the bytes lossily (for example `String::from_utf8_lossy`), so stray
   non-UTF-8 console bytes do not stop the run;
2. call `parse_line` on each line;
3. treat `Ok(None)` as "not a trace line" (it does not start with `CETRACE`
   followed by a space or the end of the line) and skip it;
4. treat `Err(ParseError)` as a malformed trace line, a failure to report
   (with `kind` and `offset`);
5. treat `Ok(Some(record))` as a valid line and use it (count, compare).

The parser is strict: it accepts the order of fields after `ev=` in any
order, but nothing else the format does not allow (two spaces, trailing
space, duplicate or unknown keys, leading zeros, a wrong `n=`, and so on).

## Use by the harness and the board script

**Read from source** (`tools/drm-harness/src/main.rs`, `tools/drm-harness/src/trace.rs`,
`scripts/board-scenarios.sh`):

- `drm-harness --trace FILE` creates FILE, writes the `hdr` line
  (`src=linux`, `clock=monotonic_ns`, `at_unit=ms`) and attaches a
  `LineTrace<String, _>` through a process-wide sink to every engine of the
  run. Lines are buffered and appended to FILE when the buffer is large, after
  each scenario and at exit (also after SIGTERM). Without `--trace` the sink
  reports itself disabled and the engine prepares no entry. A
  `trace lines=N failed=M` record ends the harness output.
- `drm-harness trace-check FILE` parses FILE (a trace, or a log holding trace
  lines) with `parse_line`, skipping non-trace lines, and prints
  `TRACE <type> <count>` lines (lines, hdr, check, validate, commit,
  commit_ok, commit_err, latched, released, dropped and its two reasons,
  latch_timeout, device_lost, caps_changed). It exits 1 on a malformed line or
  when there is no trace line, 2 if the file is unreadable.
- `scripts/board-scenarios.sh --trace` passes `--trace` to each harness run
  (FILE next to the run's log, `.trace` for `.log`), runs `trace-check` on it
  and compares the counts with the harness's own records of the same run
  (commit, latched, released, dropped, device_lost, depending on the row). A
  parse failure or a mismatch fails the row; it also checks that
  `lines` equals the file's line count and `failed` is 0 when the log has the
  `trace lines= failed=` record.

## Build status

- **Observed** (read-only check by another item of this plan, 2026-11-28): a
  host build of this crate for `thumbv7em-none-eabihf` succeeded.
- **Read from source**: nothing in CI builds the crate for that target
  (CI is not changed by this file), and its flash cost is not measured.

## Tests

**Read from source** (`tests/`): `round_trip.rs`, `malformed.rs`,
`parse_fuzz.rs`, `scenario_lines.rs` (fake-engine scenario sequences) and
`sink.rs`. Run them with `cargo test -p composition-trace`.
