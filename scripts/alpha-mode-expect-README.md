# alpha-mode-expect

Computes the colour one translucent overlay pixel should show under three alpha
models (premultiplied, straight, none) and writes, if asked, the
`camera-check.py` EXPECT JSON for one of them. It serves the alpha rows of the
`twoplane` example (`crates/composition-drm/examples/twoplane.rs`, its
`--blend` option and `Look::over`). Python 3, standard library only.

**Its numbers are computed values, not hardware measurements.** They are the
KMS blending equations evaluated in exact arithmetic. What a board really shows
is a separate, dated measurement; compare it against these values, never
substitute them for it.

## Usage

Run it with `python3 -I` so nothing is imported from the current directory:

```
python3 -I scripts/alpha-mode-expect.py [--overlay RRGGBB] [--alpha 0..255]
        [--under RRGGBB]... [--plane-alpha 0..65535] [--tolerance N]
        [--expect-json FILE --model MODEL --rect u0,v0,u1,v1
         [--region-name NAME]]
python3 -I scripts/alpha-mode-expect.py --self-test
sh scripts/alpha-mode-expect.test.sh
```

`--self-test` must be the only argument.

## Arguments

Every option takes one value. An unknown argument, a missing value or a value
that does not parse is an input error.

| option | meaning | default |
|---|---|---|
| `--overlay RRGGBB` | overlay colour, six hex digits, optional leading `#` | `ff00ff` |
| `--alpha N` | pixel alpha 0..255 (`int(v, 0)`, so `0x80` works) | `0x80` |
| `--under RRGGBB` | colour below the overlay; repeatable, one block of output each | `00ff00` and `202020` |
| `--plane-alpha N` | plane alpha 0..65535 (65535 = opaque) | `65535` |
| `--tolerance N` | per-channel tolerance (float) used to judge "separable" and written into the JSON | `40` |
| `--expect-json FILE` | also write an EXPECT file; needs `--model` and `--rect` | none |
| `--model MODEL` | `premultiplied`, `straight` or `none`; the model whose colour goes into the JSON | none |
| `--rect u0,v0,u1,v1` | four comma-separated numbers, fractions of the panel | none |
| `--region-name NAME` | region name in the JSON | `overlay translucent` |
| `--self-test` | run the built-in unittest suite and exit | off |

The defaults are the constants of `twoplane.rs`: magenta at pixel alpha `0x80`
over the green block and the grey background. Pass other values if the example
changes.

## Models

With `pa` = plane alpha / 65535 and `fa` = pixel alpha / 255:

| model | buffer holds | result |
|---|---|---|
| `premultiplied` | `round(c * alpha / 255)` | `pa*s + (1 - pa*fa) * under` |
| `straight` | `c` (coverage) | `pa*fa*c + (1 - pa*fa) * under` |
| `none` | `c`, pixel alpha ignored | `pa*c + (1 - pa) * under` |

Arithmetic is exact (fractions), rounded half up and clamped to 0..255.
`twoplane` uses f64 and rounds half away from zero, so the two agree except on
exact ties.

## Output

Plain text on stdout. First a header line with the overlay colour, pixel alpha,
plane alpha and tolerance. Then, for each `--under` colour:

- `under #rrggbb`, followed by one line per model with its 8-bit RGB: the
  buffer written for that model, blended by that model.
- `models A vs B: dR= dG= dB= separable|NOT separable`, per model pair: the
  per-channel absolute difference, and whether a camera check at `--tolerance`
  can tell the pair apart (largest difference above twice the tolerance).
  A correct premultiplied and straight run give nearly the same colour, so this
  table alone cannot separate them.
- `mismatch hw=X buffer=Y: #rrggbb dR= dG= dB= separable|NOT separable`, for
  each ordered pair of different models: the colour shown when the buffer was
  written for `Y` but the hardware blends as `X`, and its difference from the
  correct `Y` result. This is what exposes an ignored or inverted alpha
  property: compare the picture of a `twoplane --blend Y` run with the row
  whose `hw` is the real behaviour.

With `--expect-json`, the file gets the colour of the chosen model over the
first `--under` colour, and the line `wrote FILE` is printed. The JSON is the
format `camera-check.py` reads: `rotation` 0, a `panel` with unit-square
`corners` and `unit` `frac`, `tolerance`, and one region with `name`, `rect`
and `expect` (RGB). The file is written whenever `--expect-json` is given;
`--model` and `--rect` are checked after the text report is printed.

## Exit status

| code | meaning |
|---|---|
| 0 | ok (for `--self-test`: all tests passed) |
| 1 | `--self-test` only: a test failed |
| 2 | input error: unknown argument, missing or bad value, colour not six hex digits, alpha or plane alpha out of range, `--expect-json` without a valid `--model` and `--rect`, output file not writable; message on stderr as `alpha-mode-expect: ...` |

## Test

`scripts/alpha-mode-expect.test.sh` (run with `sh` or `bash`) runs, all on the
host with no board:

- the `--self-test` suite (hand-computed blends, mismatch cases, plane alpha
  0, half and full, rounding, the difference table, the JSON, the CLI errors);
- the CLI at `--plane-alpha 32768`: all three models are printed, `none` over
  green is `#807f80` and `straight` over green is `#40bf40`;
- `--plane-alpha 70000` exits 2;
- an EXPECT JSON written for `straight` is accepted by `camera-check.py` on a
  generated 40x40 picture of the expected colour.

It exits 0 when every check passes. A camera check against these values proves
only that the picture matches the equations at the stated tolerance on that
day; record the measured colour and the date in a board result.
