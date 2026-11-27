# probe-table

`scripts/probe-table.py` turns the output of `probe --raw` (see
`crates/composition-drm/examples/probe.rs`) into markdown tables for
`docs/platform-capabilities.md`. Python 3, standard library only.

## Use

```
probe --raw /dev/dri/cardN > probe.txt          # on the board
python3 scripts/probe-table.py --date YYYY-MM-DD --kernel <uname -r> probe.txt
```

Without a file argument it reads stdin. `--date` and `--kernel` are required;
every table row ends with the tag `observed <date>, kernel <kernel>`. Only use
the script on a real capture: it tags everything as observed.

Tables: device and driver caps, CRTCs, connectors with encoders and modes,
planes (type, possible_crtcs, attachment, zpos range/immutability, alpha,
pixel blend mode, rotation, scaling properties), and formats grouped by
modifier (from `IN_FORMATS`, or the plain linear list when the plane has no
`IN_FORMATS`).

## Limits

- Only the section after the `raw` line is parsed; the summary lines before it
  are derived by the backend, not raw kernel data. Unrecognised lines make the
  script fail (exit 1) rather than guess, so format drift is noticed.
- The probe prints no supported-bit list for `rotation` (a bitmask), so only
  the current value is shown. KMS has no scale-range property; scaling limits
  are reported as not observable, never inferred.
- The probe's output format has not yet been seen on real hardware; the parser
  is written from the source.

## Self-test

`python3 scripts/probe-table.py --self-test` parses
`scripts/testdata/probe-table/vkms-sample.txt` and checks the result. **That
sample is made up from the format, not a board or vkms capture**, and says so
in its first lines (lines starting with `#` are ignored by the parser). Never
cite it as observed hardware.
