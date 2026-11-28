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

## Part 2: Q6 transforms, NV12 colours, plane 34 scaling — reserved

This section is left for the part 2 item. Part 2 needs the same working KVM
capture, so it is blocked by the same failure.

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
3. This item runs again, and its results replace the "not run" rows above.
