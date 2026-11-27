# Trace line format for engine events (X-03)

**Status: implemented, format version 1, in the crate
`crates/composition-trace` (2026-11-28). Nothing in this file is
measured.** The crate (`no_std`, no `unsafe`, depending only on
`composition-core`) has the formatter, a strict parser and a `Trace` sink;
section 10 lists the choices the implementation made where this document
left one open, each one the owner may revisit. No producer emits the lines
yet: no board log in the repository uses the format, and the Linux harness
and the Zephyr applications do not attach the sink. The example lines in
section 6 are invented to show the grammar; their numbers are not results
from any device. The crate's tests parse every one of them and format it
back to the same text. First written 2026-11-28 as a proposal; status
changed the same day.

Tags used below: **read from source** (code or docs read in this
repository, cited as `file:line` at the commit this was written on),
**proposed** (a decision this document asks for and the crate does not
settle), **decided** (a choice the crate implements, section 10),
**inferred**. There is no **observed** statement here.

## 1. Why

`docs/open-issues.md` X-03 notes that the core has structured traces but no
fixed text format a board-log parser can rely on, on the Linux harness or on
a Zephyr application. `docs/m2-comparison-plan.md` §4.2 and §3 want the same
event vocabulary (`validate`, `commit`, `latched`, `released`, `dropped`) on
both sides, with timestamps that are raw cycle counts converted on the host
so the M7 does no formatting in the measured path, and a trace checker
shared by Linux and Zephyr (its §10). SoW §10 asks for "structured traces
for `validate`, `commit`, `latched`, `released`, dropped frames, and
resource assignments".

## 2. What the core has now (read from source)

`crates/composition-core/src/trace.rs`:

- `Trace` trait, `trace.rs:106`: `enabled(&self) -> bool` (`:111`) and
  `record(&mut self, at: u64, event: TraceEvent<'_>)` (`:117`).
  `at` is "the time last given to `Engine::advance_time`, 0 before the first
  call" (`:115-116`). `advance_time(now)` takes "ticks of the client's
  choice (milliseconds, vsync counts, ...)", non-decreasing
  (`engine.rs:168-173`). The engine has no clock of its own.
- `NoTrace`, `trace.rs:122`: `enabled()` is false, `record` does nothing.
  `impl Trace for &mut T` at `trace.rs:136`.
- Ordering rule (`trace.rs:13-17`): lifecycle events are traced when the
  engine records them, not when the client polls them; an operation's own
  entry (`Validate`, `Commit`) is traced when it returns, after the entries
  of what happened during it.
- The slices in an event borrow from the engine and live only for the
  `record` call (`trace.rs:35-36`), so a sink must copy or format at once.

### 2.1 `TraceEvent<'a>` (`trace.rs:37-103`), every variant

| Variant | Line | Fields (type) |
| --- | --- | --- |
| `Check` | 41 | `output: OutputId`; `assignments: &[Assignment]`; `result: Result<(), BackendError>` (one entry per candidate the planner tried; `Rejected` makes the planner try the next, any other error ends the search) |
| `Validate` | 51 | `output: OutputId`; `layers: usize`; `result: Result<&[Assignment], Error>` |
| `Commit` | 60 | `output: OutputId`; `result: Result<SubmissionId, Error>` |
| `Latched` | 67 | `output: OutputId`; `submission: SubmissionId` |
| `Released` | 74 | `buffer: BufferId`; `submission: Option<SubmissionId>` (`None` when the device was lost: every buffer is released at once) |
| `Dropped` | 83 | `output: OutputId`; `submission: SubmissionId`; `reason: DropReason` |
| `LatchTimeout` | 93 | `output: OutputId`; `submission: SubmissionId` (the submission stays pending) |
| `DeviceLost` | 100 | none |
| `CapabilitiesChanged` | 102 | none |

Nine variants. `DropReason` (`trace.rs:26`): `Replaced` (`:28`),
`DeviceLost` (`:30`).

### 2.2 Types the fields carry

| Type | Where | Shape |
| --- | --- | --- |
| `OutputId(u32)` | `types.rs:5` | backend-defined output |
| `PlaneId(u32)` | `types.rs:9` | backend-defined plane id |
| `BufferId(u32)` | `types.rs:16` | |
| `SubmissionId(u64)` | `types.rs:20` | increases monotonically |
| `Rect` | `types.rs:40` | `x: i32, y: i32, width: u32, height: u32` |
| `Assignment` | `planner.rs:212` | `layer: usize`, `plane: PlaneId`, `plane_index: usize`, `zpos: i32`, `source: Rect`, `destination: Rect` |
| `BackendError` | `backend.rs:17` | `Rejected`, `Busy`, `UnknownBuffer(BufferId)`, `DeviceLost`, `Platform(i32)` |
| `Error` | `engine.rs:17` | `Plan(PlanError)`, `Present(PresentError)`, `Backend(BackendError)`, `StalePlan`, `PlanMismatch` |
| `PlanError` | `planner.rs:131` | `UnknownOutput(OutputId)`, `TooManyLayers{count}`, `UnknownBuffer{layer,buffer}`, `InvalidSource{layer}`, `InvalidDestination{layer}`, `CannotClip{layer}`, `NoPlane{layer,reasons: RejectReasons}`, `BackendRejected{candidates: u32}`, `LimitExceeded`, `Aborted` |
| `PresentError` | `presentation.rs:83` | `UnknownBuffer(b)`, `DuplicateBuffer(b)`, `BufferTableFull`, `BufferInUse(b)`, `BufferNotReady(b)`, `BufferNotWritable(b)`, `Busy(output)`, `TooManyOutputs`, `TooManyBuffers`, `EventQueueFull`, `UnexpectedLatch{output,submission}` |
| `RejectReasons(u32)` | `planner.rs:34` | bit set of why planes refused a layer |

### 2.3 What the core trace does not carry (read from source)

- No per-event time other than `at`, which only changes when the client
  calls `advance_time`. It cannot give a latch time: the interrupt or the
  page-flip event time is not in `TraceEvent`.
- No duration for `Validate` or `Commit`; the entry is made when the call
  returns.
- No "buffer became ready" or "submit started" entry.
- Resource assignments are carried only by `Check` and by a successful
  `Validate`; `Commit` carries just the submission id.

The proposal therefore puts two time fields on every line (section 4).

## 3. Line format (version 1, implemented)

One event is one line of ASCII text, ending in `\n`, no embedded newline.
Lines that do not start with the prefix are ignored by the parser, so the
format can share a console with other output (**decided**, D1: "start
with the prefix" means the line's first byte; a console time stamp in
front of `CETRACE` makes the line a foreign line, and `CETRACE` must be
followed by a space; the parser also takes a line ending in `\r\n`).

### 3.1 Version and prefix

Every line starts with the token `CETRACE` followed by `v=<n>`. This
format is `v=1`. A parser must reject a line whose version it does not
know. The proposal also said a parser must ignore unknown `key=value`
fields in a known version; the implemented parser does not (**decided**,
D2): it is strict and rejects every key that section 3.3-3.7 does not
allow on that line, so a new field needs a parser change (or a version
bump) before a producer may write it. Removing or re-meaning a field bumps
the version.

### 3.2 Grammar (EBNF)

```
line      = "CETRACE" SP "v=" uint SP kind { SP field } LF ;
kind      = "ev=" name ;
name      = "hdr" | "check" | "validate" | "commit" | "latched" | "released"
          | "dropped" | "latch_timeout" | "device_lost" | "caps_changed" ;
field     = key "=" value ;
key       = lower { lower | digit | "_" } ;
value     = word | quoted ;
word      = 1*( ALPHA | digit | "-" | "_" | "." | ":" | "/" | "," | ";" ) ;
quoted    = DQUOTE *( any char except DQUOTE and LF ) DQUOTE ;   (* hdr only *)
uint      = digit { digit } ;
SP        = " " ;
```

A `word` may contain `;` and `,` so the `assign=` list (3.6) is one value.
Field order after `ev=` is as in 3.4 so a plain text tool can cut columns,
but a parser must not depend on it. A value holds no space (so a split on
SP, then on the first `=`, is a complete parser), except the `quoted` form,
used only on `hdr`. `v=` and `ev=` are always the first two fields, as the
grammar says; the parser enforces it.

### 3.3 Common fields

| Key | Type | Meaning |
| --- | --- | --- |
| `seq` | uint | Per-run line counter starting at 0, in the order the sink received the events. A gap means lines were lost (ring overflow, console loss) and the checker reports it. |
| `t` | uint | Raw timestamp taken by the sink inside `Trace::record` (section 5). Unit and clock are given by the `hdr` line. |
| `at` | uint | The `at` argument of `record`: the engine time last given to `advance_time`. Same clock as `t` if the client passes the platform stamp to `advance_time`; the `hdr` line says so (`at_unit`). |
| `tev` | uint, optional | Time of the underlying hardware event, when the platform knows it, in the same unit as `t`: the DRM page-flip event timestamp for `latched` on Linux, the cycle counter read in the DPU interrupt handler on Zephyr. Absent when unknown. A tool must prefer `tev` over `t` for `latched`. |
| `dur` | uint, optional | Ticks spent in the operation (for `validate`, `commit`). The core does not provide it (section 2.3); the platform wrapper that calls the engine measures it with the same clock. Absent when not measured. |
| `out` | uint | `OutputId` |
| `sub` | uint | `SubmissionId`, or `-` where the core has `None` |
| `buf` | uint | `BufferId` |

### 3.4 Events

Fields after `ev=NAME seq= t= at=`:

| `ev=` | From | Further fields |
| --- | --- | --- |
| `hdr` | not a `TraceEvent`; written once at start by the platform | see 3.5 (carries `seq`, but not `t` or `at`) |
| `check` | `Check` | `out`, `n` (number of assignments), `assign=` (3.6), `result=ok` or `result=<code>` (3.7) |
| `validate` | `Validate` | `out`, `layers`, then `result=ok` with `n` and `assign=`, or `result=<code>` and the code's extra fields; optional `dur` |
| `commit` | `Commit` | `out`, then `result=ok sub=<id>` or `result=<code>` and extras; optional `dur` |
| `latched` | `Latched` | `out`, `sub`; optional `tev` |
| `released` | `Released` | `buf`, `sub` (`-` for device loss) |
| `dropped` | `Dropped` | `out`, `sub`, `reason=replaced` or `reason=device_lost` |
| `latch_timeout` | `LatchTimeout` | `out`, `sub` |
| `device_lost` | `DeviceLost` | none |
| `caps_changed` | `CapabilitiesChanged` | none |

### 3.5 The `hdr` line

Written once per run, before the first event, by the platform code (not by
the core). It carries what the host needs to convert and to attribute the
log:

| Key | Meaning |
| --- | --- |
| `src` | `linux` or `zephyr` |
| `clock` | `monotonic_ns` (Linux), `cyc32` (Zephyr `k_cycle_get_32`), `cyc64` (a 64-bit cycle counter if the application has one), `uptime_ticks` (Zephyr `k_uptime_ticks`) |
| `freq_hz` | ticks per second of that clock, **read from the running system** (Zephyr `sys_clock_hw_cycles_per_sec()` or `CONFIG_SYS_CLOCK_TICKS_PER_SEC`; `1000000000` for `monotonic_ns`), never assumed |
| `t0` | the clock value at the time of the header, the run's zero |
| `at_unit` | `same` if `at` and `t` share the clock, otherwise a free-text unit, e.g. `ms` |
| `build` | quoted string naming the source revision of the producer |
| `board` | quoted, free text such as the SoC name; no address, host name, user or serial path |

### 3.6 Resource assignments: `assign=`

A list of `;`-joined items, one per `Assignment` (`planner.rs:212`), each
`layer:plane:index:zpos:src:dst` where `src` and `dst` are
`x,y,w,h` (`Rect`, `types.rs:40`):

```
assign=0:31:0:0:0,0,1920,1200:0,0,1920,1200;1:35:2:1:0,0,256,256:100,80,256,256
```

`plane` is the `PlaneId` (backend-defined), `index` is `plane_index`.
The plane's kind, format and modifier are not in `Assignment`; the Linux
harness's `commit` lines (`m2-comparison-plan.md` §4.1 M-5) may add
`kind=`, `fmt=`, `mod=` fields for them, but that is outside this format
and the core cannot supply them; the strict parser (D2) rejects them today.

An empty list (a `check` or `validate ok` with no assignment) is written
`n=0 assign=-`, since a `word` cannot be empty (**decided**, D4).

### 3.7 Errors: `result=<code>`

`result=ok` or a code from this table, plus the extras. Codes are the
lower-case snake form of the variant; a nested error is joined by `.`.

| Source variant | Code | Extras |
| --- | --- | --- |
| `Error::StalePlan` | `stale_plan` | |
| `Error::PlanMismatch` | `plan_mismatch` | |
| `Error::Backend(e)` | `backend.<e>` | for `unknown_buffer` `buf=`; for `platform` `errno=<int>` |
| `Error::Plan(e)` | `plan.<e>` | `unknown_output` `err_out=`; `too_many_layers` `count=`; `unknown_buffer` `layer=` `buf=`; `invalid_source`, `invalid_destination`, `cannot_clip` `layer=`; `no_plane` `layer=` `reasons=0x<hex>`; `backend_rejected` `candidates=`; `limit_exceeded`; `aborted` |
| `Error::Present(e)` | `present.<e>` | `unknown_buffer`, `duplicate_buffer`, `buffer_in_use`, `buffer_not_ready`, `buffer_not_writable` `buf=`; `busy` `err_out=`; `unexpected_latch` `err_out=` `err_sub=`; `buffer_table_full`, `too_many_outputs`, `too_many_buffers`, `event_queue_full` none |
| `BackendError` (in `check`) | `rejected`, `busy`, `unknown_buffer`, `device_lost`, `platform` | as above |

`reasons=` is the raw `RejectReasons` bit set; its bit meaning is defined
in `planner.rs:32-` and is not repeated here.

The proposal wrote the output of `plan.unknown_output` and `present.busy`
as `out=`, and the output and submission of `present.unexpected_latch` as
`out=` `sub=`. On a `validate` or `commit` line that collides with the
line's own `out=`, so the implementation writes them `err_out=` and
`err_sub=` (**decided**, D5).

## 4. Two time fields, and why

`at` alone cannot serve M-1 and M-2a of `m2-comparison-plan.md` (§3, §4):
it only changes when the client calls `advance_time`, and a `Latched`
entry is made when the engine records the backend's event, which may be
later than the interrupt. Hence:

- `t` is stamped by the sink when `record` runs. It orders events and
  gives the `commit` return time (the `Commit` entry is made on return,
  `trace.rs:15-17`).
- `tev` is the hardware event time, supplied by the backend or platform
  glue to the sink for `latched`. How the sink learns it (a side channel
  next to `record`, or a field the backend sets) is **not decided**; the
  core's `TraceEvent::Latched` has no such field (`trace.rs:67-72`), so
  this needs a core change or a platform-side table keyed by
  `(out, sub)`. Listed as an open question, section 9.

## 5. Timestamp source and conversion

| Platform | Source (proposed) | `clock` | Notes |
| --- | --- | --- | --- |
| Linux | `clock_gettime(CLOCK_MONOTONIC)` as integer nanoseconds. For `tev` on `latched`, the page-flip event's own timestamp, requested on the monotonic clock (`m2-comparison-plan.md` §4.1) | `monotonic_ns` | `freq_hz=1000000000` |
| Zephyr | `k_cycle_get_32()` (or a DWT `CYCCNT` / 64-bit counter if the application uses one) read in the sink and, for `tev`, in the DPU interrupt handler | `cyc32` / `cyc64` | frequency read from the running system, recorded in `hdr` |
| Zephyr, coarse | `k_uptime_ticks()` | `uptime_ticks` | resolution is one system tick; only for logs where cycle resolution is not needed. Say so in the report |

Conversion, on the host only, in integer arithmetic with a wide
intermediate (128-bit):

```
ns(t) = (unwrap(t) - unwrap(t0)) * 1_000_000_000 / freq_hz
```

`unwrap` extends a `cyc32` counter: when `t` is smaller than the previous
value of the same field kind, add 2^32. This is only valid if consecutive
lines are less than one wrap apart; the checker treats a `seq` gap or an
ambiguous wrap as an error. Whether a wrap period is long enough depends on
the counter frequency, which is not assumed here. A 64-bit counter or
`uptime_ticks` needs no unwrap.

The M7 writes only raw `t`, `at`, `tev` and ids; no division or formatting
is needed in the measured path (`m2-comparison-plan.md` §4.2). Producing
the text may be deferred to after the run from a binary ring buffer; the
line format is then what the dump writes. If the console is used during the
run the report says so.

Linux and Zephyr clocks have no common origin: the proposal supports
intervals within one log only. No cross-log absolute time is defined.

## 6. Examples (illustrative, invented, not measured)

Linux:

```
CETRACE v=1 ev=hdr seq=0 src=linux clock=monotonic_ns freq_hz=1000000000 t0=1000000000 at_unit=same build="example" board="example"
CETRACE v=1 ev=check seq=1 t=1000200000 at=1000200000 out=0 n=1 assign=0:31:0:0:0,0,1920,1200:0,0,1920,1200 result=ok
CETRACE v=1 ev=validate seq=2 t=1000210000 at=1000200000 out=0 layers=1 n=1 assign=0:31:0:0:0,0,1920,1200:0,0,1920,1200 result=ok dur=15000
CETRACE v=1 ev=commit seq=3 t=1000300000 at=1000200000 out=0 result=ok sub=1 dur=80000
CETRACE v=1 ev=latched seq=4 t=1016900000 at=1016000000 out=0 sub=1 tev=1016667000
CETRACE v=1 ev=commit seq=5 t=1017000000 at=1017000000 out=0 result=ok sub=2 dur=70000
CETRACE v=1 ev=latched seq=6 t=1033600000 at=1033000000 out=0 sub=2 tev=1033334000
CETRACE v=1 ev=released seq=7 t=1033600000 at=1033000000 buf=7 sub=1
```

Ordering follows section 2: a `released` after a `latched` belongs to the
submission that latch took off screen (`previous`, `engine.rs:467-471`), so
the first latch on an output releases nothing; a `released` after a
`dropped` belongs to the dropped submission; entries made during a `commit`
(the replacement's `dropped`, `released`) precede that `commit` line.

Zephyr (the clock frequency is an example value only):

```
CETRACE v=1 ev=hdr seq=0 src=zephyr clock=cyc32 freq_hz=800000000 t0=12000 at_unit=same build="example" board="example"
CETRACE v=1 ev=commit seq=1 t=500000 at=500000 out=0 result=ok sub=1
CETRACE v=1 ev=dropped seq=2 t=510000 at=510000 out=0 sub=1 reason=replaced
CETRACE v=1 ev=released seq=3 t=510000 at=510000 buf=3 sub=1
CETRACE v=1 ev=commit seq=4 t=510000 at=510000 out=0 result=ok sub=2
CETRACE v=1 ev=latched seq=5 t=13300000 at=13300000 out=0 sub=2 tev=13299100
```

Refusals and loss:

```
CETRACE v=1 ev=validate seq=9 t=2000 at=2000 out=0 layers=3 result=plan.no_plane layer=2 reasons=0x4
CETRACE v=1 ev=commit seq=10 t=2100 at=2000 out=0 result=backend.busy
CETRACE v=1 ev=latch_timeout seq=11 t=9000 at=9000 out=0 sub=4
CETRACE v=1 ev=device_lost seq=12 t=9500 at=9500
CETRACE v=1 ev=dropped seq=13 t=9500 at=9500 out=0 sub=4 reason=device_lost
CETRACE v=1 ev=released seq=14 t=9500 at=9500 buf=5 sub=-
```

## 7. Dropped frames and assignments

- **Dropped frames.** A `dropped` line has the submission id and a reason
  (`replaced`: a newer submission replaced it before latch;
  `device_lost`). Per output, a checker evaluates
  count of `commit result=ok` `= latched + dropped` (plus pending at the
  end of the log), as `m2-comparison-plan.md` M-3 requires. A
  `latch_timeout` is not a drop: the submission stays pending, so a later
  `latched` or `dropped` for the same `(out, sub)` still ends it. A
  missing interrupt is never turned into a `latched` line.
- **Resource assignments.** `validate result=ok` carries the final
  `assign=` list (layer to plane, `zpos`, clipped source and destination);
  `check` lines show each candidate the planner tried and the backend's
  answer. A refused frame has no assignment, only a `plan.*` code. The
  assignment a hardware commit actually used is the last `validate ok`
  before the `commit ok` for that output (**inferred**: the core does not
  link the two by id; a `plan=<n>` correlation field would need a core
  change).

## 8. Which SoW §10 metric each event lets one compute

SoW §10 (`SoW.md:318-325`) performance list, with the events needed. All are
*computable from a log in this format*; none has been computed.

| SoW §10 metric | Events and fields | Limit |
| --- | --- | --- |
| Plane assignment time | `validate dur=`; or `t` differences if the platform brackets the call | `dur` is not in the core; needs the platform wrapper |
| Time from ready buffer to latch | `commit t` (its return) to `latched tev` per `sub`; M-2a "submit-to-latch" | "buffer ready" itself is not an event; only submit-to-latch is available |
| End-to-end presentation latency | `commit` to `latched tev` only; latch-to-light needs a camera or photodiode (`m2-comparison-plan.md` M-2b) | not computable from this log |
| Dropped / late frames | `dropped`, `latch_timeout`, `latched tev` intervals (M-1, M-3) | late = interval above the plan's threshold, applied on the host |
| Structured traces (validate, commit, latched, released, dropped, assignments) | all events | a format only |
| CPU and memory | none | not from engine events (`/proc`, thread analyser per M-4, M-7) |
| DMA/display bandwidth, power | none | needs an instrument; record "not available" otherwise |

Additional checks the lines allow: a buffer must not be `released` before
the latch of the next submission on that output, or the drop of its own
submission; every `sub` appears
in exactly one of `latched`/`dropped` before the end; `seq` has no gap
(the M-6 trace checker, `m2-comparison-plan.md` §4.1-4.2).

## 9. Open questions and non-goals

- Where `tev` comes from for `latched` (section 4): needs a decision by
  the owner of `crates/composition-core/src/**`. The crate's formatter
  takes `tev` and `dur` in its `Stamp` argument, so a platform wrapper that
  knows them can write them with `write_event`; the `LineTrace` sink cannot,
  because `Trace::record` gives it neither, and leaves both out.
- `RejectReasons` (`planner.rs:34`) has `bits()` but no constructor from
  raw bits, so the parser rebuilds a parsed `reasons=` from the eleven
  constants the core defines (`planner.rs:40-60`) and rejects any other bit
  (D6). If the core adds a reason, the crate's table has to follow; a
  `RejectReasons::from_bits` in the core would remove that coupling. Not
  changed here: `crates/composition-core/src/**` is held by another plan.
- Strictness (D2) against extensibility: the `kind=`, `fmt=`, `mod=`
  fields of 3.6 or any other harness-specific field need either a format
  change recorded here and in the parser, or a lenient parser mode. The
  owner may prefer the proposal's "ignore unknown keys" rule.
- Whether the C ABI exposes trace to C/Zephyr callers; not examined here.
- The harness's existing lines (`RESULT`, `VERDICT`, `EXPECT`, `GEOMETRY`
  in `crates/composition-drm/examples/twoplane.rs`, seen at lines 53, 74,
  566, 1735, 1815) are proposed to stay as they are next to `CETRACE`;
  they are not part of this format.
- Not in scope of the crate: a log checker (counts, `seq` gaps, the
  section 8 rules), a ring buffer, emission from the DRM harness or a
  Zephyr application, and any hardware run.

## 10. Implementation and decisions

Crate `crates/composition-trace` (**read from source**, at the commit that
adds it):

- `write_header(w, seq, &Header)` and `write_event(w, &Stamp, &TraceEvent)`
  write one line, ending in `\n`, into any `core::fmt::Write`, in the field
  order of section 6.
- `parse_line(&str) -> Result<Option<Record>, ParseError>`: `Ok(None)` for
  a foreign line, `Record::Header` or `Record::Event` (an owned copy of the
  `TraceEvent`, convertible back with `Event::as_trace_event`) for a trace
  line, and a `ParseError { kind, offset }` otherwise. The kinds are
  `Syntax`, `TrailingGarbage`, `UnknownVersion`, `UnknownEvent`,
  `UnknownKey`, `UnexpectedKey`, `DuplicateKey`, `MissingField`,
  `BadNumber`, `BadValue`, `CountMismatch` and `TooManyAssignments`.
- `LineTrace::new(writer, clock)` implements `Trace`: each `record` writes
  one line with the next `seq` and `t = clock()`. `LineTrace::header`
  writes the `hdr` line. Attach it, or `&mut` it, with
  `Engine::with_trace`.
- Tests: every `TraceEvent` variant, every `DropReason` and every error
  code round-trip through format and parse and format back to the same
  text; every line of section 6 parses and formats back unchanged; each
  error kind has malformed-line cases; an engine on the fake backend
  writes through `LineTrace` and every line parses with no `seq` gap.

Decisions where this document left a choice open. Each one is the
implementer's (2026-11-28), recorded so the owner may revisit it:

| | Decision |
| --- | --- |
| D1 | A trace line is one whose first bytes are `CETRACE` followed by a space or the end of the line; anything else (a console time stamp in front, `CETRACEX`, lower case) is a foreign line. One trailing `\n` or `\r\n` is accepted; any other `\r` or `\n` in a trace line is a syntax error. |
| D2 | The parser is strict: a key not allowed on that line (unknown to the format, or known but not for that event or `result=`) is an error, as is a duplicate key, a missing field, or a trailing space. This replaces the proposal's rule of ignoring unknown keys. |
| D3 | Numbers have one spelling: decimal, no `+`, no leading zero; `-` only on signed fields (`zpos`, rectangle `x` and `y`, `errno`) and never on 0. Each field is range-checked against its core type (`u32` for `out`, `buf`, plane ids, widths and `candidates`; `u64` for `seq`, `t`, `at`, `tev`, `dur`, `sub`, `freq_hz`, `t0`; `usize` for `layers`, `n`, `layer`, `count`, plane index; `i32` for the signed ones). `reasons=` is `0x` and lower-case hex without leading zero, `0x0` for none. |
| D4 | An empty assignment list is `n=0 assign=-`. `n=` must equal the number of items. More than `MAX_LAYERS` (16) items is an error, since the parser keeps them in a fixed-size table. |
| D5 | Error extras that name an output or submission are `err_out=` and `err_sub=` (section 3.7), so they do not collide with the line's own `out=`. |
| D6 | `reasons=` bits the core does not define are rejected (section 9). |
| D7 | `src=` and `clock=` take only the values of section 3.5. `freq_hz=0` is rejected. `at_unit=` is one `word`. `build=` and `board=` must be quoted and other fields must not; quoted text is printable ASCII (0x20-0x7e) without `"`, with no escape. The formatter refuses to write a header that breaks these rules. |
| D8 | `tev=` is allowed only on `latched` and `dur=` only on `validate` and `commit`; the formatter refuses to write them elsewhere. A `check` line carries `n=` and `assign=` whatever its result; a refused `validate` carries neither. |
| D9 | `v=` and `ev=` must be the first two fields, and the version is checked before anything else on the line, so a line of a later version is refused only for its version. |
| D10 | `LineTrace` advances `seq` for every line it is handed, also when the writer fails, so a lost line shows as a `seq` gap; it counts failures (`failed()`). A writer failing mid-line may leave a partial line, which the parser refuses. The sink does not check that the header comes first. |
