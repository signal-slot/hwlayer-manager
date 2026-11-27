# format-cut-report

`scripts/format-cut-report.py` shows, per plane, which `(format, modifier)`
pairs of a full `IN_FORMATS` listing survive the backend's `MAX_FORMATS` cut
and which are dropped. Python 3, standard library only; run it with `-I`.

## Use

```
python3 -I scripts/format-cut-report.py [--max N] [--quiet] [FILE...]
```

No file: reads stdin. `--max` overrides the limit (default 32, the value of
`composition_core::limits::MAX_FORMATS`, copied into the script, so update both
if the constant changes). `--quiet` prints only the summary lines.

Per plane it prints the total pair count, the pairs kept and the pairs
dropped, then one summary line:

```
FORMAT-CUT plane=<id> total=<n> kept=<k> dropped=<d> linear_first=<yes|no>
```

Exit code: 0 nothing dropped on any plane, 1 something dropped, 2 parse error
(also unreadable file, or no listing found).

This answers RK3588 Q1 and HVS Q4 once the full per-plane dumps exist: the
`probe` summary prints at most 32 formats per plane, the full list is in the
raw `IN_FORMATS` line or the `props` example.

## The order the backend uses

`DrmBackend` (`crates/composition-drm/src/lib.rs`, where it fills
`PlaneCaps::formats`) takes the pairs in blob order, as `parse_in_formats`
returns them: modifier entries in blob order, and within one entry the formats
in mask-bit order. It then does a stable
`pairs.sort_by_key(|p| p.modifier != Modifier::LINEAR)` and
`take(MAX_FORMATS)`. So: every linear (modifier 0) pair first, in blob order,
then the non-linear pairs in blob order, and everything after the 32nd pair is
dropped. The script reproduces exactly this, without removing duplicates (the
backend does not). A plane without `IN_FORMATS` is treated as all-linear.

`linear_first=yes` means the listing already had at least one linear pair and
sorting did not change its order; `no` means the sort moved pairs, or the
plane has no linear pair at all.

## Accepted input

Lines are recognised on their own, so extra text, other sections and other
tools' output are ignored. A line `plane <id>` (any text after the id) sets the
current plane. Then:

- `probe --raw`: `  IN_FORMATS (<n>): FOURCC:0xMOD ...`. `<n>` must equal the
  number of pairs listed.
- `probe`, plane without `IN_FORMATS`: `  formats (<n>): linear FOURCC ...`
  (all linear).
- `props`: `IN_FORMATS: <f> formats, <m> modifiers` followed by `<m>` lines
  `modifier 0x<hex>: FOURCC FOURCC ...`.

The probe's derived (post-cut) `formats (k): ...` lines do not start with
`linear` and are ignored. Newer `kms_sweep` or caps dumps are accepted if they
use one of these line shapes; anything else is skipped. The non-listing `IN_FORMATS` lines the examples print are skipped:
the `probe --raw` property line (`IN_FORMATS: immutable atomic blob, <n> bytes
value <id>`), the props property line (`IN_FORMATS (id N): ...`),
`IN_FORMATS: unreadable ...` and `IN_FORMATS blob too short (...)`. Any other
line starting with `IN_FORMATS`, a wrong count, a plane listed twice,
a listing before any `plane` line, or a file with no listing at all is a parse
error (exit 2).

## Test

```
bash scripts/format-cut-report.test.sh
```

Uses synthetic data in `scripts/testdata/format-cut-report/` (45 pairs with
AFBC-style modifiers, a small plane, a props-style plane, a malformed file).
No board data.
