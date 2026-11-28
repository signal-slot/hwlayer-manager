# zpos-probe-expect

Judges a saved log of the `zpos_probe` example
(`crates/composition-drm/examples/zpos_probe.rs`): one verdict for each of its
three TEST_ONLY cases and a summary line that infers whether `zpos` is compared
per CRTC or globally. Used for RK3588 question Q6 in the NanoPC-T6 sessions.
Python 3, standard library only, same style as `twoplane-expect.py`.

```
python3 scripts/zpos-probe-expect.py [LOG|-]
sh scripts/zpos-probe-expect.test.sh
```

Exit status 0 = ok, 2 = input error (unreadable file, malformed `result=`).

## Input

The `ZPOS` lines `zpos_probe` prints (*read from source*):

```
ZPOS case=distinct planes=31,37 zpos=0,1 result=ok
ZPOS case=equal planes=31,37 zpos=1,1 result=EINVAL
ZPOS case=cross_equal planes=31,57 zpos=1,1 result=ok
ZPOS case=<name> result=skipped reason=...
```

`result` is `ok`, `skipped`, an errno name (`EINVAL`, `EOPNOTSUPP`, ...) or
`errnoN` for an errno without a name. Other lines are ignored. If a case
appears twice the last line wins.

## Output

```
ZPOS distinct accepted
ZPOS equal refused(EINVAL)
ZPOS cross_equal accepted
ZPOS scope=per_crtc
```

Each case line is `ZPOS <case> accepted|refused(<errno>)|not_run`. A case is
`not_run` when its line is absent or says `result=skipped`. `cross_equal` is
absent when `zpos_probe` found only one active CRTC.

## Rule

*Inferred.* The scope line follows from the three verdicts alone:

| distinct | equal | cross_equal | `scope` |
|---|---|---|---|
| accepted | refused | accepted | `per_crtc` |
| accepted | refused | refused | `global` |
| anything else | | | `undetermined` |

Reasoning (*inferred*): the kernel refuses equal `zpos` on planes of one CRTC.
If the same value on planes of two different CRTCs is accepted, the comparison
does not cross CRTCs. If it is refused as well, the comparison spans CRTCs. The
baseline `distinct` must be accepted, so that a refusal of `equal` is about the
value and not about the plane set. If `equal` is accepted the kernel does not
reject duplicates on one CRTC, so the cross-CRTC case cannot tell the two
apart. With `cross_equal` not run (one CRTC) nothing can be said.

Limits of the rule (*inferred*): a `cross_equal` refusal can also come from a
resource limit that has nothing to do with `zpos` (bandwidth, a plane
assignment between video ports). Read the errno and the `PLANE` lines of the
log before accepting `global`. This script judges one TEST_ONLY run; it
measures nothing itself.
