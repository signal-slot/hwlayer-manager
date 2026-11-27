# Driver pipe protocol (xcheck v1 cross-check, C side)

This describes how `liftoff-xcheck-driver` (built by `../build-liftoff.sh`
from `driver.c` and `fake_drm.c`) talks to the program that runs it. The
design is in `docs/libliftoff-crosscheck.md` (sections 2, 5, 6 and 8).

## Assumption for the project owner

The design leaves one decision open: does the Rust side answer libliftoff's
`TEST_ONLY` commits, or is the test-commit model written twice, once in Rust
and once in C? **This driver assumes the design's stated preference: the Rust
side answers every `TEST_ONLY` commit over a pipe from its own model, so the
model exists once.** The C side has no model. It only turns each atomic
request into a line and turns the one-word answer into a return code. If the
owner decides on two models instead, a C model would replace the pipe answer
in `answer_commit` (`driver.c`), and the protocol below would remain as a
trace.

## Invocation

```
liftoff-xcheck-driver [--accept-all | --reject-all] [--request-fd N]
                      [--answer-fd N] [--timeout-ns N] [--verbose] FILE.xcheck
```

- `FILE.xcheck`: one xcheck v1 file (topology and frames).
- Default mode is **pipe**: the driver writes request lines and reads answers.
- `--accept-all` answers every request `ok` itself. `--reject-all` answers
  every request `einval` itself. In both modes the request lines are still
  written and nothing is read. These modes are for tests that run without the
  Rust side.
- `--request-fd N`: write request lines to fd N (default 1, stdout, mixed with
  the result lines). `--answer-fd N`: read answers from fd N (default 0,
  stdin).
- `--timeout-ns N`: libliftoff's `timeout_ns` (an `int64_t`; default 60 s,
  against libliftoff's 1 ms default). The time the answering side takes counts
  against this deadline.
- `--verbose`: libliftoff's debug log goes to stderr. Without it only
  libliftoff errors appear there.

Diagnostics go to stderr only. Exit status: 0 when every frame ran, 1 for a
bad input file (message `PATH:LINE: ...`), 2 for bad usage, 3 for a missing
or unknown answer.

## Lines on stdout

Every line ends with `\n`. Its first word says what it is:

| Keyword | Direction | Meaning |
| --- | --- | --- |
| `test` | driver to responder | one `TEST_ONLY` atomic commit; an answer must follow |
| `result` | driver to runner | the outcome of one frame |

Lines with any other first word may be added later. Readers ignore them.

### Request line (`test`)

One line per `drmModeAtomicCommit` call. libliftoff only commits with
`DRM_MODE_ATOMIC_TEST_ONLY`. A commit without that flag is refused in the
fake with `EINVAL` and is never sent.

```
test frame=<name> seq=<n> flags=0x<hex> planes=<k>{ <plane-state>}
<plane-state> := plane=<id> fb=<id> crtc=<id> src16=<x>,<y>,<w>,<h>
                 dst=<x>,<y>,<w>,<h> alpha=<0..65535>
                 blend=<opaque|premul|coverage|-> rotation=0x<hex> zpos=<i64>
```

The `<plane-state>` is written on one line. It is wrapped above for reading.

- `frame`: the name from the xcheck `frame` record. `seq`: 1, 2, ... within
  that frame. `flags`: the commit flags (always containing `0x100`,
  `DRM_MODE_ATOMIC_TEST_ONLY`).
- `planes=<k>`, then `k` plane states in the file's plane order. A plane is
  listed when the request leaves its `FB_ID` or its `CRTC_ID` non-zero. Every
  plane that is not listed is disabled. The fake starts each commit from "all
  planes disabled", because no real commit is ever made. A listed plane with
  `fb=0` or `crtc=0` is half set up, and the responder should refuse it
  (`einval`), as the kernel does.
- `fb`: the framebuffer id, which is the xcheck `buffer` id. `crtc`: the CRTC
  id, which is the xcheck `output` id.
- `src16`: `SRC_X,SRC_Y,SRC_W,SRC_H` exactly as requested, in 16.16 fixed
  point. Divide by 65536 for pixels. The driver sets whole pixels shifted by
  16.
- `dst`: `CRTC_X,CRTC_Y` (signed) and `CRTC_W,CRTC_H`.
- `alpha`: the requested value. It is `65535` when the plane has no `alpha`
  property, because libliftoff skips 65535 on such planes.
- `blend`: the `pixel blend mode` as an xcheck word. DRM values are
  `None`=0=`opaque`, `Pre-multiplied`=1=`premul` and `Coverage`=2=`coverage`.
  When the request does not set it, this is the plane's default
  (pre-multiplied if offered). It is `-` when the plane has no such property.
- `rotation`: the DRM bitmask: `0x1` rotate-0, `0x2` rotate-90, `0x4`
  rotate-180, `0x8` rotate-270, `0x10` reflect-x (xcheck `flip_h`), `0x20`
  reflect-y (`flip_v`). It is `0x1` when the plane has no `rotation`
  property.
- `zpos`: the plane's zpos. libliftoff never writes zpos, so this is the
  fixed value or `zpos_now` from the file.

Before the request is sent, the fake makes the kernel's own early refusals
itself, and those never reach the pipe. Writing an immutable property (`type`,
`IN_FORMATS`, a fixed `zpos`) gives `EINVAL`. A property id or object id that
the fake does not know gives `ENOENT`, which libliftoff treats as fatal.
libliftoff also checks property ranges, enums, bitmasks, `possible_crtcs` and
`IN_FORMATS` itself before it commits (design section 2).

### Answer line

One line per request, read before the driver goes on. Only its first word
counts. Text after it is ignored, so it may hold a reason for a log:

| Word | Returned to libliftoff |
| --- | --- |
| `ok` | 0 (the test commit passes) |
| `einval` | `-EINVAL` |
| `erange` | `-ERANGE` |
| `enospc` | `-ENOSPC` |

Any other word, or end of file, stops the driver with exit status 3.
libliftoff treats all three errors as "try another allocation" (design
section 2).

### Result line (`result`)

One line per frame, in file order, after all of its requests:

```
result frame=<name> output=<id> layers=<n> apply=<ok|error:<errno>|skipped:<why>>
       placed=<m> planes=<p1>,<p2>,... deadline=<0|1> tests=<t> origin=<origin text>
```

This is also one line, wrapped here for reading.

- `layers`: the number of layers in the frame. `placed`: how many of them
  libliftoff put on a plane.
- `planes`: one entry per layer, in frame order. The entry is the plane id,
  `composition` (`liftoff_layer_needs_composition`), or `none` for a layer
  libliftoff treats as invisible (alpha 0). With no layers it is `-`.
- `apply`: `ok`, or `error:<n>` with the positive errno from
  `liftoff_output_apply`. On error every layer is reported as `composition`
  and `placed=0`. `skipped:unknown-output` or `skipped:unknown-buffer` means
  that the frame names an output or buffer the file lacks. Such a frame is not
  run (no requests, `planes=-`, `tests=0`), the way core refuses it before
  planning.
- `deadline=1` when libliftoff logged that its deadline passed, or when
  `liftoff_output_apply` ran for `timeout_ns` or longer.
- `tests`: the number of request lines written for this frame.
- `origin`: the file's `origin` line after the word `origin`, verbatim, up to
  the end of the line (it holds spaces). It is always the last field. A
  result line keeps the basis of its input, as design section 5 requires.

## How the driver maps the file

- One fake CRTC per `output` line. The CRTC id is the output id, and the CRTC
  index (the `possible_crtcs` bit) is the order of the `output` lines, so the
  plane's `outputs=` mask is used unchanged as `possible_crtcs`.
- Plane properties: `type` (immutable enum), `FB_ID`, `CRTC_ID`,
  `CRTC_X/Y` (signed range), `CRTC_W/H`, `SRC_X/Y/W/H` (ranges), `zpos`
  (range `N..M` with value `zpos_now`; `fixed:N` is an immutable range
  `N..N`), `IN_FORMATS` (immutable blob, kernel layout, when the plane has
  `format` lines), `alpha` (range 0..65535) when `alpha=1`, `pixel blend
  mode` (enum of the listed modes) when `blend` is not `-`, and `rotation`
  (bitmask of the listed transforms plus rotate-0) when `transforms` is not
  `-`. `scaling`, `max_source`, `full_output`, `profile` and `group` are not
  KMS properties. The driver ignores them, and the answering model applies
  them.
- Framebuffers: each `buffer` line, readable with `drmModeGetFB2`, with
  `DRM_MODE_FB_MODIFIERS` set so that libliftoff's format check is active.
- Per frame: a fresh libliftoff device (on a `/dev/null` fd, which the fake
  never uses), `liftoff_device_register_all_planes`, one output, and one layer
  per `layer` line. Each layer gets `FB_ID`, `SRC_*` (pixels << 16),
  `CRTC_*`, `zpos`, `alpha`, `pixel blend mode` and `rotation`. The layer
  `zpos` is the rank of a stable sort by `z`, so equal `z` keeps frame order
  with the later layer on top, as composition-core stacks ties. No
  composition layer is set. `liftoff_output_apply(output, req, 0, &opts)`
  runs with the long timeout.
- Layer defaults when a field is missing: `alpha=65535`, `blend=premul`,
  `transform=normal`. `buffer`, `src`, `dst` and `z` are required. A
  transform may join names with `+` (`rot90+flip_h`). A reflection alone
  means rotate-0 plus that reflection.

`profile` is read but has no effect on the C side. A file that the Rust reader
refuses (for example one over the engine's limits) should not be given to the
driver. The driver checks only what it needs to build the fake device.
