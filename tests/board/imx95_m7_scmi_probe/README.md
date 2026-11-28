# i.MX95 Cortex-M7: read-only SCMI ownership probe of the display resources (Zephyr)

A small upstream-Zephyr application for the Cortex-M7 of the i.MX95. It asks
the System Manager (SM), from the M7's SCMI agent, what the SM reports about
the clocks and power domains of the display pipeline (DPU, LDB/LVDS, MIPI
DSI path, display PLLs). It prints one line per answer on the M7 console.

SoW §5.1 and WP3 ask first which core owns the DPU, its clocks and its power
domains. The repository has the IDs and the permission model from source; this
program is the M7-side tool that lets a later board session turn those claims
from **inferred** into **observed**.

Status (2026-11-28): **host build only. It has not run on any board.** No
result below is a measurement; everything about run-time behaviour is read
from source or inferred.

It uses upstream Zephyr only and contains no code from any private or
showcase repository.

Source documents for the IDs and the permission model:

- [docs/imx95-dpu-scmi-ids.md](../../../docs/imx95-dpu-scmi-ids.md): the
  clock and power-domain IDs and names (§2.1, §3).
- [docs/imx95-m7-scmi.md](../../../docs/imx95-m7-scmi.md): the SCMI protocols
  upstream Zephyr implements on the M7, the transport and the error mapping.

## What it sends

Only queries. `src/main.c` calls no SCMI function that changes anything (no
clock rate, parent or config set, no power-state set, no reset, no base
permission or agent-configuration call, no pin control) and it reads no
display register.

| Protocol | Query (Zephyr function) | For |
|---|---|---|
| every protocol linked into the image (base 0x10, power 0x11, system 0x12, clock 0x14, pin control 0x19) | `PROTOCOL_VERSION` (`scmi_protocol_get_version`), `PROTOCOL_ATTRIBUTES` (`scmi_protocol_attributes_get`) | SM protocol versions; number of clocks / domains |
| base 0x10 | `PROTOCOL_ATTRIBUTES` + `DISCOVER_VENDOR` / `SUB_VENDOR` / `IMPLEMENTATION_VERSION` (`scmi_base_get_platform_info`) | SM vendor and build version, agent count |
| base 0x10 | `DISCOVER_AGENT` with the caller's own ID and with each agent ID (`scmi_base_discover_agent`) | which agent the M7 is, and the names of all agents |
| clock 0x14 | `CLOCK_ATTRIBUTES`, `CLOCK_RATE_GET`, `CLOCK_CONFIG_GET`, `CLOCK_PARENT_GET`, `CLOCK_GET_PERMISSIONS` | each display clock below |
| power 0x11 | `POWER_STATE_GET` (`scmi_power_state_get`) | each display power domain below |

Display IDs (from [docs/imx95-dpu-scmi-ids.md](../../../docs/imx95-dpu-scmi-ids.md);
a build-time check ties each number to the upstream Zephyr binding):

| Kind | IDs and names |
|---|---|
| Clocks | 18 `VIDEOPLL1_VCO`, 19 `VIDEOPLL1`, 37 `LDBPLL_VCO`, 38 `LDBPLL`, 62 `CAMAPB`, 66 `MIPIPHYCFG`, 67 `MIPIPHYPLLBYPASS`, 68 `MIPIPHYPLLREF`, 69 `MIPITESTBYTE`, 75 `DISPAPB`, 76 `DISPAXI`, 77 `RESERVED77_DISPDP`, 78 `DISPOCRAM`, 80 `DISP1PIX`, 81 `DISP2PIX`, 82 `DISP3PIX` |
| Power domains | 13 `DISPLAY`, 3 `CAMERA` |

ID 77 is reserved in the SM header but named `DISPDP` in the Linux and Zephyr
bindings (doc §2.1, §7); it is queried to record what the SM answers.

Not queried, and why (read from source, Zephyr `v4.5.0`):

- **Resets** (SM reset lines 20, 21, 22, 45): the M7 devicetree has no SCMI
  reset protocol node, so Zephyr's reset helpers are not built
  (`CONFIG_ARM_SCMI_RESET_HELPERS` depends on such a node). Left out.
- **NXP LMM and SM-misc vendor protocols**: upstream Zephyr has no client for
  them (`drivers/firmware/scmi/nxp/` has only `cpu.c` and `shmem.c`).
- **NXP CPU protocol 0x82**: built out (`CONFIG_NXP_SCMI_CPU_DOMAIN_HELPERS=n`),
  so that `soc_init()` does not send its CPU sleep-mode request.
- **Perf domains**: no Zephyr binding or client.

## Output

One line per query, then a summary:

```
hwlm-imx95-m7-scmi-probe: read-only SCMI queries, Zephyr <version>, imx95_evk/mimx9596/m7
SCMI <proto> id=<n> name=<name> result=<ok|denied|not_found|err=<code>> value=<...>
...
SCMI-PROBE SUMMARY queries=<n> ok=<n> denied=<n> not_found=<n> err=<n> clocks=16 power_domains=2
hwlm-imx95-m7-scmi-probe: done
```

- `<proto>` is one of `protocol.version`, `protocol.attributes`,
  `base.platform`, `base.agent_self`, `base.agent`, `clock.attributes`,
  `clock.rate`, `clock.config`, `clock.parent`, `clock.permissions`,
  `power.state`.
- `name` is the name from the source doc (`-` for protocol and agent lines;
  `OWN` for the caller's own agent).
- `result`: `ok` (0), `denied` (`-EACCES`, SM `DENIED`), `not_found`
  (`-ENOENT`, SM `NOT_FOUND`), otherwise `err=<negative errno>`, for example
  `err=-95` (`-EOPNOTSUPP`, SM `NOT_SUPPORTED`), `err=-22` (`-EINVAL`) or a
  transport timeout. Mapping from Zephyr `drivers/firmware/scmi/core.c`.
- `value` is `-` unless the result is `ok`. Decoded fields:
  - `clock.attributes`: raw word, `on` (bit 0), `restricted` (bit 1), the
    SM's clock name.
  - `clock.rate`: Hz, lower 32 bits only (Zephyr returns only those).
  - `clock.config`: raw word, `on` (bit 0).
  - `clock.permissions`: raw word, `state` (bit 31, Zephyr `clk.h`), `parent`
    (bit 30) and `rate` (bit 29) (SCMI v3.2 layout, not checked against the
    SM source here).
  - `power.state`: raw state, `on` (`0x00000000`), `off` (`0x40000000`) or
    `other`.

How to read it (from [docs/imx95-display-clocks.md](../../../docs/imx95-display-clocks.md)
§4, read from source in that note; untested):

- The NXP SM answers the read messages above **without a permission check**.
  So `denied` is not expected for them; the ownership signal is in `value`.
- `restricted=1` means the M7 agent's level on that clock is below
  `EXCLUSIVE` (it may not set rate or parent).
- `state=1` in `clock.permissions` means the M7 agent has at least `SET` on
  that clock; `rate=1` / `parent=1` mean `EXCLUSIVE`.
- Under NXP's `mx95evk` configuration the expected answer is: no permission
  on any display clock for the M7 agent (doc §2.3). Whether the Verdin's SM
  agrees is exactly what a run would show.
- `POWER_STATE_GET` needs no permission, so it shows whether Linux keeps
  `DISPLAY` on, not who may switch it.

## SCMI traffic that is not from this program

The image still contains upstream Zephyr code that talks to the SM at boot
(read from source at the pinned tag; not observed):

- **Protocol version negotiation** for each linked protocol
  (`drivers/firmware/scmi/core.c`, `scmi_core_protocol_setup`). It changes no
  resource.
- **The console UART**: the LPUART driver enables the `LPUART3` clock through
  the SCMI clock driver, and the pin control driver applies the `LPUART3`
  pin mux of the EVK board over SCMI pin control. These are the same requests
  every M7 image of this board target sends. They touch no display resource.
  If the M7 agent lacks permission on the `LPUART3` clock, the Zephyr clock
  driver skips the request silently (doc
  [imx95-m7-scmi.md](../../../docs/imx95-m7-scmi.md)).
- **No SoC-init clock or power request.** The overlay
  `boards/imx95_evk_mimx9596_m7.overlay` disables every EVK peripheral except
  the console (same list as [../imx95_m7](../imx95_m7/README.md), without the
  rpmsg nodes). So upstream `soc_init()` sets no FlexCAN or SAI clock, the
  NETC and MIPI DSI blocks are not built (`CONFIG_NET_DRIVERS` and
  `CONFIG_MIPI_DSI` are off in the generated `.config`), and the NXP CPU
  protocol call is built out (see above).

`CONFIG_POWER_DOMAIN`, `CONFIG_PM` and `CONFIG_PM_DEVICE` are off. The SCMI
reply timeout is raised from 500 us to 10 ms; a timeout is printed as
`err=<code>`, never retried.

## Board target and limits

There is no Verdin iMX95 board in upstream Zephyr. Like the other i.MX95 M7
apps, this one uses the NXP EVK target `imx95_evk/mimx9596/m7` (TCM: code in
ITCM, data in DTCM; no DDR carve-out). The limits recorded in
[../imx95_m7/README.md](../imx95_m7/README.md) apply here too (inferred, not
checked on a Verdin board):

- **SCMI channel:** the M7 devicetree uses MU5 channel 0 and the shared
  memory window of the EVK's M7 agent. If the Toradex SM gives the M7 logical
  machine another MU or window, every query fails (`err=`) or the SCMI
  transport fails at boot and nothing is printed.
- **Console:** `LPUART3` at 115200 baud with the EVK pin mux. On the Mallow
  carrier the M7 console may be routed elsewhere; see
  [docs/imx95-m7-console.md](../../../docs/imx95-m7-console.md).
- **No resource table, no rpmsg.** Output goes to the UART only. That Linux
  remoteproc starts an ELF without a `.resource_table` section is inferred
  from the generic remoteproc code, not checked on this board.

## Pinned versions

| Item | Value |
|---|---|
| Zephyr | <https://github.com/zephyrproject-rtos/zephyr>, tag `v4.5.0`, commit `41d10b59730406bbab4ca9f1fbbe9e96aafb4966` (the pin of [docs/zephyr-workspace.md](../../../docs/zephyr-workspace.md)) |
| Zephyr SDK | 1.0.1, toolchain `arm-zephyr-eabi`, GCC 14.3.0, binutils 2.43.1 |
| west | 1.5.0 |
| Board target | `imx95_evk/mimx9596/m7` |

## Build

Prepare the workspace and SDK with `scripts/prepare-zephyr.sh` as
[docs/zephyr-workspace.md](../../../docs/zephyr-workspace.md) describes, then,
from inside the west workspace:

```sh
cd "$ZEPHYR_WORKSPACE"
west build -p always -b imx95_evk/mimx9596/m7 -d <build-dir> <repo>/tests/board/imx95_m7_scmi_probe \
    -- -DZEPHYR_SDK_INSTALL_DIR="$ZEPHYR_SDK_INSTALL_DIR"
```

Use a build directory outside the repository and outside `/tmp`. The
artefact is `<build-dir>/zephyr/zephyr.elf`.

Host build result (2026-11-28, pinned versions above, x86_64 Linux host):
builds with no compiler warning; `FLASH` (ITCM) 25,992 B of 256 KiB, `RAM`
(DTCM) 8,248 B of 256 KiB. CI does not build this program.

## Running it (not done)

Nothing here has been run. A board session would load `zephyr.elf` through
Linux remoteproc from `/tmp` and restore every setting afterwards, as
[docs/imx95-m7-runbook.md](../../../docs/imx95-m7-runbook.md) and the load
procedure in [../imx95_m7/README.md](../imx95_m7/README.md) describe, and
capture the M7 console. The program prints its report once and then idles;
stop the core with remoteproc when the summary line has appeared.

Whether Linux may start the M7 on this board at all depends on the SM
configuration; see [docs/imx95-verdin-sm-config.md](../../../docs/imx95-verdin-sm-config.md)
§5 and [docs/imx95-zephyr-feasibility.md](../../../docs/imx95-zephyr-feasibility.md).
