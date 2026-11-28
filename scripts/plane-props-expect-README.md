# plane-props-expect

Decodes the plane properties in the text output of the `props` example
(`crates/composition-drm/examples/props.rs`) into one verdict line per plane,
for the transform claims of the Rk3588 and HVS5/HVS6 profiles. Python 3,
standard library only.

```
props /dev/dri/cardN > props.txt
python3 scripts/plane-props-expect.py props.txt
bash scripts/plane-props-expect.test.sh
```

Exit status 0 = ok, 2 = input error (unparsable property line, or no
`plane N: possible_crtcs` section). The `probe` example prints no properties, so
its output is rejected; use `props` for this checker.

## Verdict line

`plane <id> <type>; <rotation>; <zpos>; alpha; blend-mode; color-encoding;
color-range; scaling-filter`, separated by `; `.

- Type is the lower-cased name `props` prints for the `type` enum (overlay,
  primary, cursor).
- Rotation bits are decoded at the standard positions (read from source:
  `props.rs`, not from a kernel) and named as composition-core's `Transform`:
  `rotate-0` Normal, `rotate-90` Rotate90, `rotate-180` Rotate180,
  `rotate-270` Rotate270, `reflect-x` FlipH, `reflect-y` FlipV. A plane without
  the property prints `no rotation property`.
- zpos prints the range, `mutable` or `immutable`, and the current value, or
  `no zpos`.
- The other properties print `yes` or `no` for presence only.

## Limit to state in session notes

`props` prints the current value of `rotation`, not the set the plane
supports (the `drm` crate does not return a bitmask's value names). A plane
that shows only `rotate-0` may support more. A verdict is therefore evidence of
the property's presence and current state; supported transforms need a
TEST_ONLY probe. Say so when quoting a line as read from a board.

## Sessions

Run `props` on each board during its session, keep the output with the session
note, and run this checker on it. Tag the verdict lines in the session doc with
the evidence kind and the date (host UTC). Do not copy device paths, addresses or
serial numbers into the note.

- NanoPC-T6 (RK3588 VOP2): feeds the Rk3588 transform claims; expect a
  Cluster/Esmart difference if the kernel exposes one.
- RPi5 (BCM2712 HVS6) and reTerminal CM4 (BCM2711 HVS5): feed the HVS5/HVS6
  transform claims; compare rotation and reflection per plane type.
