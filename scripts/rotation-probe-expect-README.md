# rotation-probe-expect

Judges a saved `rotation_probe` run: for each plane, which rotation and
reflect values a `TEST_ONLY` atomic commit accepted or refused, compared
with what the kernel source says the plane offers. Python 3, standard
library only.

The input line format is the one planned for the `rotation_probe` example
(a new example, not yet in the tree when this script was written; check the
example's output if it differs):

```
ROT plane=<id> type=<Primary|Overlay|Cursor> value=<name> bits=0x<hex> result=ok
ROT plane=<id> type=<Primary|Overlay|Cursor> value=<name> bits=0x<hex> result=errno=<n>
```

The eight values are `rotate-0`, `rotate-90`, `rotate-180`, `rotate-270`,
`reflect-x`, `reflect-y`, `rotate-90|reflect-x` and
`rotate-0|reflect-x|reflect-y`.

```sh
python3 -I scripts/rotation-probe-expect.py rot.log --profile rk3588
python3 -I scripts/rotation-probe-expect.py rot.log --profile hvs6
```

## Options

- `--profile rk3588|hvs5|hvs6`: compare with the expected table below.
  Without it the script only prints the tables and checks completeness.
- `--cluster IDS`, `--esmart IDS` (rk3588): comma-separated plane ids whose
  window class is known. Other planes are inferred: a plane that accepted
  `rotate-90` is judged as Cluster, any other plane as Esmart. The log does
  not carry the window class, so name the planes when you know them; an
  inferred class cannot catch a Cluster plane that wrongly refuses
  `rotate-90`, it would be judged as Esmart.
- Several files are read as one log; with no file, or `-`, stdin is read.

## Output

```
TABLE plane=42 type=Overlay accepted=rotate-0,reflect-y refused=rotate-90(errno=22),...
VERDICT plane=42 match class=esmart
VERDICT plane=42 mismatch accepted_unexpected=reflect-x refused_unexpected=... class=esmart
VERDICT plane=42 incomplete missing=rotate-270,... class=esmart
VERDICT match|mismatch|incomplete
```

- `mismatch`: a value was accepted that the source says is not offered
  (`accepted_unexpected`), refused although offered (`refused_unexpected`),
  or the same value was accepted in one run and refused in another
  (`conflicting`). It outranks `incomplete`.
- `incomplete`: not all eight values have a line for the plane. A plane
  known only from a `PLANE id <n>` or `PLANE plane=<n>` line has no ROT lines
  and is incomplete.
- Unknown lines are ignored. Without `--profile` the last line is
  `VERDICT unchecked` when every plane is complete, else `VERDICT incomplete`.

| exit | meaning |
|---|---|
| 0 | match (or unchecked and complete) |
| 1 | mismatch or incomplete |
| 2 | usage error, unreadable file, unparsable `ROT` line, or no plane found |

## Expected tables

Accepted values, per plane. Every other value is expected refused. All rows
are read from source, not measured. Sources:
[RK3588 scale and transform](../docs/rk3588-vop2-scale-transform-source.md) §3,
[RK3588 limits](../docs/rk3588-vop2-limits-source.md),
[vc4 transform](../docs/vc4-transform-source.md) §2, §3 and §5.

| Profile and plane | Accepted | Tag |
|---|---|---|
| rk3588, Cluster0-3 (mask `ROTATE_90 \| ROTATE_270 \| REFLECT_X \| REFLECT_Y`, plus `ROTATE_0`) | all eight values | read from source |
| rk3588, Esmart0-3 (mask `REFLECT_Y`, plus `ROTATE_0`) | `rotate-0`, `reflect-y` | read from source |
| hvs5, hvs6, every plane incl. cursor (mask `ROTATE_0 \| ROTATE_180 \| REFLECT_X \| REFLECT_Y`) | `rotate-0`, `rotate-180`, `reflect-x`, `reflect-y`, `rotate-0\|reflect-x\|reflect-y` | read from source |

Refused for hvs5 and hvs6: `rotate-90`, `rotate-270`, `rotate-90|reflect-x`
(not in the mask; that the DRM core rejects values outside the mask is
inferred, the core was not read).

Not covered by the table: the source has no `atomic_check` rule that rejects a
transform for a format, a size or scaling (read from source), but the probe
uses one buffer, so a refusal that depends on those would show up as a
mismatch. Whether an accepted transform draws correctly needs a picture check
(inferred; this script only reads `TEST_ONLY` results).

Self-test: `bash scripts/rotation-probe-expect.test.sh` (synthetic logs
inline, no board data).
