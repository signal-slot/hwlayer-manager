# STM32MP257F-DK: KVM capture of the two-plane harness

Follow-up to the `KVM result` section of `docs/observed/stm32mp257.md`. Only
the KVM part is repeated.

## KVM result: no picture — observed 2026-11-28

**The NanoKVM could not be used again.** The MCP server for the board's KVM
failed to connect at the start of this session, with the same error as before:

```
HTTP 404 dialing http://<kvm relay>/p2p/v1/<id>/kvm/<id>/mcp
```

(The relay address and ids are left out.) No KVM tool was registered, so no
screenshot was taken, and **z-order and alpha were not verified on the
physical output**. There is no captured-against-expected table, and no
screenshots exist. The KVM's web interface and credentials were not tried:
the credentials are not on this machine and the task allows only the MCP path.

An unauthenticated `GET /` on the KVM's own host answered HTTP 200 (observed
2026-11-28), so the unit is up on the network; the failure is in the MCP relay
path, and a capture needs its credentials, which are not available here.

What was run anyway, over ssh, so that the harness verdicts are current:

| Item | Value |
|---|---|
| Kernel | 6.6.129 (observed 2026-11-28) |
| Driver, mode | `stm`, 1920x1080@60.000Hz, HDMI-A-1 (observed 2026-11-28) |
| Build | `cargo build --release --target aarch64-unknown-linux-musl -p composition-drm --example twoplane --config 'target.aarch64-unknown-linux-musl.linker="rust-lld"'` |
| Display client | `weston-graphical-session` stopped for the runs, started afterwards; `systemctl is-active weston-graphical-session seatd-weston` printed `active` twice and `weston`, `weston-desktop-shell`, `weston-keyboard` were running again (observed 2026-11-28) |

## Harness lines — observed 2026-11-28

Default run:

```
RESULT frames=600 committed=600 latched=600 released=1198 dropped=0 late=0 period_ms=16.667 latch_interval_mean_ms=16.666 latch_interval_max_ms=19.236 elapsed_s=10.053
VERDICT twoplane: PASS 600 frames on two planes, 600 latched, 1198 released, 0 late, driver stm mode 1920x1080@60.000Hz
```

`--swap-z`:

```
RESULT frames=600 committed=600 latched=600 released=1198 dropped=0 late=0 period_ms=16.667 latch_interval_mean_ms=16.667 latch_interval_max_ms=19.217 elapsed_s=9.992 swapped_at=300
VERDICT twoplane: PASS 600 frames on two planes, 600 latched, 1198 released, 0 late, driver stm mode 1920x1080@60.000Hz
```

`--blend coverage --plane-alpha 32768`:

```
RESULT frames=600 committed=0 latched=0 released=0 dropped=0 late=0 period_ms=16.667 latch_interval_mean_ms=0.000 latch_interval_max_ms=0.000 elapsed_s=0.001
VERDICT twoplane: FAIL frame 0: validate: Plan(NoPlane { layer: 1, reasons: BLEND }) (1 error(s)), driver stm mode 1920x1080@60.000Hz
```

The third run is not a driver fault: the LTDC planes have no `pixel blend mode`
property, so capability detection offers premultiplied only and the planner
refuses `coverage` before any commit (committed=0). That matches
`stm32mp257.md`. So no colour check with plane alpha 32768 exists either.

## Expected colours (from the harness EXPECT lines) — not compared

These are what a working KVM capture must show. The captured column is empty
because nothing was captured.

| Phase | Point | x,y | Expected | Captured | Max channel diff |
|---|---|---|---|---|---|
| normal | background | 96,756 | #202020 | none | n/a |
| normal | red block | 384,405 | #ff0000 | none | n/a |
| normal | blue block | 1536,405 | #0000ff | none | n/a |
| normal | green block uncovered | 864,405 | #00ff00 | none | n/a |
| normal | overlay opaque over green | 1056,405 | #ffff00 | none | n/a |
| normal | overlay opaque over background | 1248,405 | #ffff00 | none | n/a |
| normal | overlay translucent over green | 1056,607 | #807f80 | none | n/a |
| normal | overlay translucent over background | 1248,675 | #901090 | none | n/a |
| swapped | overlay opaque over green | 1056,405 | #00ff00 | none | n/a |
| swapped | overlay opaque over background | 1248,405 | #202020 | none | n/a |
| swapped | overlay translucent over green | 1056,607 | #00ff00 | none | n/a |
| swapped | overlay translucent over background | 1248,675 | #202020 | none | n/a |

The background, red, blue and uncovered-green points are the same in both phases.

## State after the session — observed 2026-11-28

- Weston and `seatd-weston` active again; the harness binary and log were
  removed from `/tmp`; nothing installed, no console or reset used.
