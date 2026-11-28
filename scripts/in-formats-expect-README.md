# in-formats-expect

Host checker that judges the raw `IN_FORMATS` list of every plane from the text
the `probe` or `props` example prints (RK3588 design Q1: does the cap of
composition-core hide AFBC pairs on Cluster planes, and is linear listed
first). Python 3, standard library only. It reads text and touches no device.

```
python3 scripts/in-formats-expect.py [--max-formats N] [FILE ...]   # no FILE: stdin
bash scripts/in-formats-expect.test.sh
```

## Input

Either example works; they list the pairs in the same order (modifier entry by
modifier entry, format bit by format bit), so the verdicts agree.

- `probe`: `plane <id> type <kind> possible crtcs ...`, then
  `IN_FORMATS (<n>): XR24:0x0 AR24:0x800000000000001 ...`. A plane without the
  property prints `formats (<n>): linear ...`; those count as linear pairs.
- `props`: `plane <id>: possible_crtcs ...`, then
  `IN_FORMATS: <F> formats, <M> modifiers` and one `modifier 0x...: FOURCC ...`
  line per modifier. A plane whose property is absent or has blob 0 prints no
  such line and gets the verdict `pairs none`.

Run the example on the target with the usual display-guard rules, save its
stdout to a file, and judge the file on the host:

```
python3 scripts/in-formats-expect.py probe-output.txt props-output.txt
```

Several files are concatenated; plane ids repeat if both examples are given, so
judge them one file at a time when you want a cross-check of the two.

## Output

One line per plane, in input order:

```
plane <id> pairs <n> over-max <yes|no> (max <M>) linear-first <yes|no|absent> afbc-beyond-max <k>
```

- `pairs`: number of format/modifier pairs listed.
- `over-max`: `yes` when `pairs` is greater than MAX_FORMATS. `M` is read from
  `pub const MAX_FORMATS` in `crates/composition-core/src/limits.rs` at run
  time; `--max-formats N` overrides it (the test uses this).
- `linear-first`: `yes` when `DRM_FORMAT_MOD_LINEAR` (0) is listed before any
  non-linear modifier, `no` when a non-linear modifier comes first, `absent`
  when linear is not listed at all. Plain linear-only lists are `yes`.
- `afbc-beyond-max`: pairs at index MAX_FORMATS or later whose modifier has the
  ARM vendor byte (0x08 in bits 63:56), i.e. AFBC pairs the capped list in
  composition-core would drop. Non-zero means the cap hides AFBC on that plane.

Reading it for Q1: a plane with `over-max yes` and `afbc-beyond-max` above 0
loses AFBC pairs under the cap; `linear-first no` means even the retained head
may be vendor-only, so a client that takes the first entries sees no linear.

## Exit status

0 when every plane parsed. 2 for input it cannot parse: no plane lines, a
malformed `FOURCC:0xMOD` pair, an announced pair or modifier count that does
not match the lines, an `unreadable` or `too short` IN_FORMATS line, a
truncated modifier entry, or an unreadable file. The reason goes to stderr.

## Notes

- The verdict only judges text. Record the example's own output next to the
  verdict in the session document, with the date (host UTC) as observed.
- A verdict of `afbc-beyond-max 0` on one plane says nothing about the others;
  each plane has its own line.
