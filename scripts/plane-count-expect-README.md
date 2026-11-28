# plane-count-expect

`scripts/plane-count-expect.py` does the plane-type and `possible_crtcs`
arithmetic on the text output of `cargo run -p composition-drm --example probe
-- --raw` (read from source: `crates/composition-drm/examples/probe.rs`, the
`crtcs` and `planes` sections). It uses only the Python standard library and
never touches a board; the probe output is the input.

```
scripts/plane-count-expect.py [--expect-planes N] [--expect-crtcs N] [--limits FILE] [PROBE_OUTPUT|-]
```

## What it prints

- the totals of CRTCs and planes, and planes per type (primary, overlay,
  cursor, plus other/none if the kernel reports them);
- per CRTC: the planes whose `possible_crtcs` includes it, split by type, and
  how many planes lie beyond primary plus overlays (that is, cursor and other),
  labelled "all cursor", "not all cursor" or "none";
- the planes shared by several CRTCs;
- one line "extra planes are cursors: yes/no". This is the check for
  "are the extra per-CRTC planes cursors" (HVS Q5, BCM2712) and, from the
  shared-plane list and the type counts, for "which hardware windows show up
  as KMS planes" (RK Q8);
- one line comparing the total and the largest per-CRTC count with
  `MAX_PLANES` (read from `crates/composition-core/src/limits.rs`, or from
  `--limits FILE`): "caps would hide planes" or "caps would not hide planes".

The CRTC set of a plane is taken from the id list in parentheses in the probe
line; if that list is empty, the bit mask is decoded against the CRTC listing
order.

## Exit codes

- 0: output printed, no expectation failed;
- 1: `--expect-planes` or `--expect-crtcs` did not match (a `MISMATCH:` line
  says which);
- 2: usage error or parse error (no `crtcs` or `planes` section, unknown plane
  type, a plane naming an unknown CRTC, unreadable file).

## Test

`bash scripts/plane-count-expect.test.sh` runs inline fixtures: an
HVS6-like layout with a cursor plane per CRTC, an RK-like layout with shared
planes above the cap, a non-cursor extra, a count mismatch and a parse error.
The fixtures are synthetic, not board recordings: the layouts are inferred
and make no claim about any board.
