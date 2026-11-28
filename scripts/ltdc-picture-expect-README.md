# ltdc-picture-expect

Host helper for STM32MP257F-DK session C pictures. It turns a plane geometry, a transform and/or an NV12 colour into a
[`camera-check.py`](camera-check-README.md) `EXPECT.json`, the way
[`twoplane-expect.py`](twoplane-expect.py) does for twoplane and plane_commit EXPECT lines, but with a notion of
transforms and YUV colours. Python 3, standard library only. No network, no board access.

**Everything it writes is a prediction (inferred / read from source), not an observation.** The file carries a
`"prediction"` note saying so (camera-check ignores it). A PASS against a picture is what makes a row observed, and
only the session that took the picture can say that.

```
python3 scripts/ltdc-picture-expect.py --mode WxH --plane X,Y,WxH [--src WxH]
        [--transform T] [--nv12 | --yuv Y,Cb,Cr]
        [--encoding bt601|bt709] [--range limited|full]
        [--size F] [--tolerance N] [--rotation N] [--name NAME] [--out DIR]
```

Exit status: 0 = ok, 2 = input error (message on stderr).

## Kinds of expectation

| Invocation | Regions | Basis |
| --- | --- | --- |
| (default) | four, one per buffer quadrant of a `pattern=quadrants` RGB layer, at its position after `--transform` | quadrant colours and mapping read from `crates/composition-drm/examples/plane_commit.rs`; the transform semantics of the LTDC from `docs/stm32mp257-ltdc-transform-source.md`: read from source, the picture is inferred |
| `--nv12` | the same four, for an NV12 `pattern=quadrants` layer: plane_commit's BT.601 limited Y,Cb,Cr of each quadrant colour, converted back with `--encoding`/`--range` (default BT.601 limited, the driver default) | coefficients read from source (`ltdc.c`), formula and rounding inferred (`docs/stm32mp257-ltdc-yuv-source.md`) |
| `--yuv Y,Cb,Cr` | one flat region at the plane centre | same as `--nv12` |

`--transform` takes `0`, `90`, `180`, `270`, `reflect-x`, `reflect-y`, joined with `+`, like plane_commit's
`rotation=` (at most one angle). The mapping is plane_commit's `quadrant_screen_pos`: the buffer is scaled to the
plane rectangle, turned counter-clockwise by the angle, then reflected. 180 gives the same regions as
`reflect-x+reflect-y`. 90 and 270 are accepted for the primary plane only; the script prints a note on stderr, because
overlays refuse them and a committed 90/270 on this board is predicted to reach a null pointer (inferred): do not
commit it.

`--src WxH` records the buffer size. Scaling does not move the quadrant centres (the scaler is independent of the
transform, read from source), so it only adds a note to the file; NV12 sizes must be even. Compare the same
quadrants of a scaled and an unscaled picture for the plane-34 scaling item.

## Options

- `--mode WxH`: the CRTC mode, the panel of the picture (KVM capture, edge to edge). The file uses
  `"panel": {"corners": [[0,0],[1,0],[1,1],[0,1]], "unit": "frac"}` and camera `rotation` 0 (override with
  `--rotation 0|90|180|270`).
- `--plane X,Y,WxH`: the plane's `dst` rectangle; it must lie inside the mode.
- `--size F`: width of each sample rect as a fraction of the mode (default 0.02, as in twoplane-expect).
- `--tolerance N`: RGB distance (default 40). The YUV model is good to about +-2 per channel; the capture chain
  decides the rest, so leave the default or widen it.
- `--encoding`, `--range`: only with `--nv12` or `--yuv`. They stand for the plane properties `COLOR_ENCODING` and
  `COLOR_RANGE` (planes 34 and 42 only); record the values you set.
- `--name NAME` (`[a-z0-9_-]`, default `ltdc`) and `--out DIR` write `DIR/expect-NAME.json`; without `--out` the JSON
  goes to stdout.

## Example

```
python3 scripts/ltdc-picture-expect.py --mode 1920x1080 --plane 100,100,400x200 \
    --transform reflect-x --out /path/to/expect --name plane42-reflect-x
python3 scripts/camera-check.py capture.png /path/to/expect/expect-plane42-reflect-x.json
```

## Model of the YUV conversion (inferred)

`Y' = (Y-16)*255/219` for limited range (`LxPCR_YREN` set), `Y' = Y` for full; `R = Y' + r_cr/256 (Cr-128)`,
`G = Y' - (g_cb/256 (Cb-128) + g_cr/256 (Cr-128))`, `B = Y' + b_cb/256 (Cb-128)`, clamped, rounded to nearest. The
coefficient words are the four entries of `ltdc_ycbcr2rgb_coeffs` (read from source):

| Encoding, range | `CYR0R` | `CYR1R` |
| --- | --- | --- |
| BT.601, limited | `0x02040199` | `0x006400D0` |
| BT.601, full | `0x01C60167` | `0x005800B7` |
| BT.709, limited | `0x021D01CB` | `0x00370089` |
| BT.709, full | `0x01DB0193` | `0x00300078` |

The known values of the yuv-source document (for example BT.601 limited 81,90,240 gives 255,0,0) are checked by the
test. Note that plane_commit's own NV12 quadrants are 82,90,240 and 144,54,34 (its integer formula), so the
predicted red is 255,1,0 and green 0,254,0 there.

## Test

```
sh scripts/ltdc-picture-expect.test.sh
```

Checks the reflect-x, reflect-y, rotate-180 and rotate-90 quadrant mapping, BT.601 limited and BT.709 full
conversions of known values, NV12 quadrants, a round trip through `camera-check.py` on a synthetic picture (the
matching transform passes, a wrong one fails), and the input errors (exit 2).
