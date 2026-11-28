# kms-sweep-expect

Gives a mechanical verdict on a saved `kms_sweep` combination run
(`crates/composition-drm/examples/kms_sweep.rs`), for example the RK3588 Q4
question "are eight simultaneous windows possible". Python 3, standard library
only, same style as `twoplane-expect.py`. It reads the log only; it needs no
board.

```
python3 scripts/kms-sweep-expect.py LOG [--expect-min N]
sh scripts/kms-sweep-expect.test.sh
```

## Input

The `COMBO` and `COMBO_MAX` lines as printed by `kms_sweep` (read from source,
the doc comment of `kms_sweep.rs`):

```
COMBO k=2 set=0 planes=31,37 scale=1.0000x1.0000 src=1920.0000x1080.0000 dst=1920x1080 result=ok
COMBO_MAX accepted_k=2
```

`k` is the number of windows (planes) in the TEST_ONLY commit; `result=ok`
is a pass, any errno name is a fail. Other lines are ignored.

## Output

```
SWEEP windows=<n> pass=<k> fail=<m>     one line per window count, ascending
SWEEP max_simultaneous=<n>              largest n with at least one pass (0: none)
EXPECT ok|fail                          only with --expect-min N: max >= N
```

`max_simultaneous` counts a pass in any plane subset of that size, while
`kms_sweep` itself sets `accepted_k` from subset 0 only; a `COMBO_MAX`
larger than the COMBO lines support prints a `SWEEP note:` line and exits 1.

Exit status: 0 ok, 1 `EXPECT fail` or COMBO_MAX disagreement, 2 input error
(unreadable file, malformed or inconsistent COMBO line, no COMBO lines, bad
option).

## Evidence

The verdict is only as good as the log: record the log's own `DATE` and
`KERNEL` lines beside the verdict. The script's behaviour is inferred from
synthetic logs in `kms-sweep-expect.test.sh`; no board result is claimed here.
