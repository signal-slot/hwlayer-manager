# STM32MP257F-DK (LTDC): board session C, 2026-11-28

Session C was meant to take pictures of the board's HDMI output through the NanoKVM.
Sessions A and B ([stm32mp257-ltdc-session-2026-11-28.md](stm32mp257-ltdc-session-2026-11-28.md))
answered their questions from logs and the debugfs CRC only. This session has
**no pictures**. The KVM capture still fails, so the session stops before any
run.

## Tags, clock and conditions

- Tags: `observed 2026-11-28` means read in this session. `inferred` means
  reasoned from other results.
- Times are host UTC (`date -u`). The board clock runs about 10 h behind, so
  it is not used.
- Access: ssh and KVM only. The console and reset resources are away.

## Gate: KVM rehearsal — observed 2026-11-28

The A1 rehearsal ([docs/observed/stm32mp257-kvm.md](observed/stm32mp257-kvm.md),
section "KVM rehearsal, 2026-11-28") gave the verdict **capture fails**. This
item may only run if that rehearsal reported "capture works".

The capture was tried again at the start of this session, at 2026-11-28 01:37
host UTC (observed 2026-11-28). The KVM's MCP server failed to connect with
the same error as in every earlier session:

```
HTTP 404 dialing http://<kvm relay>/p2p/v1/<id>/kvm/<id>/mcp
```

The relay address and ids are left out. No KVM tool was registered, so there
was no screenshot tool.

## Part 1: Q1 pixel colours — not run

Because of the failed gate, none of the following was done:

- No twoplane binary was built or copied to the board.
- Weston was not stopped.
- No screenshot was taken.
- `scripts/camera-check.py` was not run on any frame.

So nothing was measured for any of the planned runs:

| Run | Status |
|---|---|
| (a) default `twoplane` | not run, no picture |
| (b) `--swap-z`, first phase (about 2 s) | not run, no picture |
| (b) `--swap-z`, second phase (about 7 s) | not run, no picture |
| (c) `--plane-alpha 32768` | not run, no picture |

**Q1 verdict: open.** The prediction from sessions A and B (inferred) is still
untested on the picture: opaque regions should pass, and translucent regions
should match straight alpha (α = pixel α × plane α), not premultiplied. The
expected colours from the 2026-11-28 harness EXPECT lines are listed in
[docs/observed/stm32mp257-kvm.md](observed/stm32mp257-kvm.md) and in the Q1
section of the sessions A/B document.

## Part 2: Q6 transforms, NV12 colours, plane 34 scaling — not run

The part 2 item started at 2026-11-28 01:51 host UTC (observed 2026-11-28).
The KVM's MCP server again failed to connect with the same HTTP 404 as in the
gate above. No KVM tool was registered, so no screenshot could be taken. Part
1 also found no working capture, so part 2 stops here.

None of the following was done: no plane_commit build or copy to the board,
no Weston stop, no TEST_ONLY or commit, no screenshot, no `camera-check.py`
run. So no probe has a verdict:

| Probe | Status |
|---|---|
| Q6, plane 34 (primary): 0, rotate-180, reflect-x | not run, no picture |
| Q6, primary CRC instability as a visible defect | not run, no picture |
| Q6, plane 42: 0, reflect-x, reflect-y, reflect-x+reflect-y | not run, no picture |
| Q6, plane 49: 0, reflect-x, reflect-y, rotate-180 | not run, no picture |
| NV12 colours on plane 42, 1:1 | not run, no picture |
| Plane 34 scaling, smaller src upscaled to the mode | not run, no picture |

**Q6 verdict: open.** The transform claims still rest on session B's
TEST_ONLY and CRC results and on the source reading (inferred). Plane 49
transforms and plane 34 scaling were never probed at all.

## Session C summary

| Question | Result | Tag |
|---|---|---|
| Gate (KVM capture) | fails: MCP HTTP 404, at 01:37 and 01:51 host UTC | observed 2026-11-28 |
| Q1 pixel colours | open, not run | observed 2026-11-28 |
| Q6 transforms (planes 34, 42, 49) | open, not run | observed 2026-11-28 |
| NV12 colours, plane 42 | open, not run | observed 2026-11-28 |
| Plane 34 scaling | open, not run | observed 2026-11-28 |
| Straight-alpha prediction for Q1 | untested on the picture | inferred |

## State after part 2 — observed 2026-11-28

The state was read over ssh, read only, at 2026-11-28 01:51 host UTC:

- `systemctl is-active weston-graphical-session seatd-weston` printed `active`
  and `active`.
- Kernel 6.6.129.
- The board's `/tmp` holds no harness files.

Nothing was stopped, installed, run or changed on the board.

## State after part 1 — observed 2026-11-28

The state was read over ssh, read only, at 2026-11-28 01:37 host UTC:

- `systemctl is-active weston-graphical-session seatd-weston` printed `active`
  and `active`.
- Kernel 6.6.129.
- The board's `/tmp` holds no harness files.

Nothing was stopped, installed, run or changed on the board.

## What a retry needs (inferred)

1. The MCP relay path to the KVM is fixed, or the MCP server is registered
   again.
2. A fresh A1 rehearsal reports "capture works".
3. The part 1 and part 2 items run again, and their results replace the
   "not run" rows above.
