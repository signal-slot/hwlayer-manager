# i.MX95 display clocks: pixel, LDB and video PLL, and who may set them under the SM

Source study, dated 2026-11-28. Scope: SoW §5.1 / §9 M2 (WP3). No board was
accessed and nothing was built or run. Every finding carries one of two tags:
**read from source** (with the pin from §1) or **inferred**. Nothing here is
**observed**.

What this note does not repeat:

- The stock logical-machine (LM) layout, which LM owns `DC*`, `LVDS`,
  `BLK_CTRL_DISPLAYMIX` and `VIDEO_PLL1`, the generated `config_scmi.h`
  permissions, and the move of those resources to the M7. See
  [imx95-sm-display-lm.md](imx95-sm-display-lm.md) and §2 of
  [imx95-zephyr-feasibility.md](imx95-zephyr-feasibility.md).
- The Zephyr side: the SCMI client, `soc_init()`'s DSI clock sequence, the
  clock IDs in Zephyr's binding, and the silent-success behaviour of
  `scmi_clock_on_off` on a restricted clock. See
  [imx95-m7-scmi.md](imx95-m7-scmi.md).
- The DPU/LDB driver behaviour apart from clocks, and the Toradex kernel pin.
  See [imx95-dpu-source-6.6.md](imx95-dpu-source-6.6.md).

## 1. Sources and pins

| Id | Source | Pin | Files read |
|---|---|---|---|
| C1 | NXP imx-sm, <https://github.com/nxp-imx/imx-sm> | tag `rel_imx_sm_2025q3` = commit `af1e37026c6ba19cdca98fd6e91494efd3e7b5ec`. The annotated tag `lf-6.6.52-2.2.2` (tag object `5c94700d6a54`) points to the same commit, as checked in a clone's `packed-refs` on 2026-11-28. This is the same pin as S1 in the feasibility note and as the other SM notes | `devices/MIMX95/sm/dev_sm_clock.h`, `dev_sm_clock.c` (verbatim up to about 70 % of the file, the rest summarised); `devices/MIMX95/drivers/fsl_clock.c`; `devices/MIMX9/drivers/fsl_fract_pll.c`, `fsl_ccm.c`; `sm/doc/examples/example.cfg`; `devices/MIMX95/configtool/ccmsrcgpcmix.cfg`; `configs/mx95evk.cfg`; `sm/rpc/scmi/rpc_scmi_clock.c`, `rpc_scmi_config.h`, `rpc_scmi.c`, `rpc_scmi_base.c`; `sm/lmm/lmm_clock.c`, `lmm.c`, `lmm_sys.c`; `sm/doc/config.md`; listings of `configs/` and `configs/other/` |
| C2 | NXP linux-imx, <https://github.com/nxp-imx/linux-imx> | tag `lf-6.6.52-2.2.2` = commit `5a0a5e71d2bd` | `include/dt-bindings/clock/fsl,imx95-clock.h`; `arch/arm64/boot/dts/freescale/imx95.dtsi` (display nodes); `imx95-19x19-verdin-panel-cap-touch-10inch-lvds.dts`; `drivers/gpu/drm/imx/dpu95/dpu95-{core,drv,crtc,kms,framegen}.c`; `drivers/gpu/drm/bridge/imx/imx95-ldb.c`; `drivers/clk/imx/imx95-blk-ctrl.c`; `drivers/clk/clk-scmi.c`; `drivers/clk/clk-fixed-factor.c`; `drivers/phy/freescale/phy-fsl-imx8mp-lvds.c` |
| C3 | Toradex linux-toradex, `git://git.toradex.com/linux-toradex.git` | commit `64f85d65c112c45ceb6c7e0566885afc62b1a551` (the board's 6.6.157 kernel) | **Not re-read here.** Its DPU clocks (`imx95.dtsi:2759-2767`) and the 10" LVDS overlay rates (`:43-45`) are cited from [imx95-dpu-source-6.6.md](imx95-dpu-source-6.6.md), and they agree with C2 |
| — | Toradex SM configuration for the Verdin iMX95 | — | **Not read.** C1 has no Verdin or Toradex config: `configs/` holds `mx94alt`, `mx94evk`, `mx95alt`, `mx95evk`, `simu`, and `configs/other/` holds only NXP variants (`mx95evk-android`, `-jailhouse`, `-rpmsg`, `-sof`, `mx95netc`, `mx95rte`) (read from source, C1). Toradex's layers on git.toradex.com could not be fetched (see S8 in the feasibility note) |

How the files were read: `git clone` and direct downloads were refused on
this node. The files were therefore read over HTTPS through a page fetcher
(raw.githubusercontent.com at the commit above). The quoted identifiers and
code are verbatim, but **line numbers are approximate** unless a row gives an
exact number. The line numbers given for `fsl_fract_pll.c`, `fsl_ccm.c` and
`example.cfg` were counted on the fetched raw text and should be within ±2.
Re-check against a clone before citing a line elsewhere. **Not read**: the
`imx95-19x19-verdin.dts` base file in C2, the `CCM_CLOCK_ROOT_DIV_MASK`
register header, and `mx95evk.cfg`'s own definition of `OWNER`.

Everything about the SM below describes **NXP's `mx95evk` configuration**.
That the Verdin's SM grants the same permissions is **inferred** and
unchecked.

## 2. The display clock tree

### 2.1 SM clock IDs (read from source, C1 `dev_sm_clock.h`)

Linux's binding (C2 `fsl,imx95-clock.h`, `IMX95_CCM_NUM_CLK_SRC` = 41) uses
the same numbers as the SM's `DEV_SM_CLK_*`. Zephyr's binding uses them too
(see [imx95-m7-scmi.md](imx95-m7-scmi.md)).

| Clock | SM ID | Kind | Notes |
|---|---|---|---|
| `VIDEOPLL1_VCO` | 18 | source (PLL VCO) | parent of `VIDEOPLL1` (`fsl_clock.c`, `s_clockSourceParent`) |
| `VIDEOPLL1` | 19 | source (PLL output) | IDs 20-23 are reserved: no VIDEOPLL1 DFS or ODIV variants |
| `LDBPLL_VCO` | 37 | source (PLL VCO) | |
| `LDBPLL` | 38 | source (PLL output) | parent `LDBPLL_VCO`; usable only while the DISPLAY power domain is on (`CLOCK_SourcePdIsOn`) |
| `MIPIPHYCFG` / `MIPIPHYPLLBYPASS` / `MIPIPHYPLLREF` / `MIPITESTBYTE` | 66 / 67 / 68 / 69 | root | DSI path only |
| `DISPAPB` / `DISPAXI` / `DISPOCRAM` | 75 / 76 / 78 | root | Linux also names 77 `IMX95_CLK_DISPDP`; the SM marks 77 reserved (`DEV_SM_CLK_RESERVED77`) |
| `DISP1PIX` / `DISP2PIX` / `DISP3PIX` | 80 / 81 / 82 | root | |

IDs 0-40 are source clocks and 41-163 root clocks, followed by GPR selects
and CGCs (read from source). The header never states `DEV_SM_NUM_CLOCK`;
175 is computed from the last ID (inferred). Linux adds
`IMX95_CLK_LDBPLLDIV7` = 175 as a Linux-only ID, outside the SM range (read
from source, C2).

The SM has **no LDB pixel or LDB serializer clock ID**. The LVDS gates live in
the display block-control registers, not in the CCM (read from source, C1 and
C2, see §2.3).

### 2.2 CCM root muxes (read from source, C1 `fsl_clock.c`, `g_clockRootMux`, approx. lines 300-340)

| Root | mux 0 | mux 1 | mux 2 | mux 3 |
|---|---|---|---|---|
| `DISP1PIX` | OSC24M | AUDIOPLL1 | VIDEOPLL1 | VIDEOPLL1 |
| `DISP2PIX` | OSC24M | AUDIOPLL1 | RESERVED21 | SYSPLL1_PFD2 |
| `DISP3PIX` | OSC24M | AUDIOPLL1 | RESERVED23 | SYSPLL1_PFD2 |
| `DISPAPB` | OSC24M | SYSPLL1_PFD0_DIV2 | SYSPLL1_PFD1_DIV2 | FRO |
| `DISPAXI`, `DISPOCRAM` | OSC24M | SYSPLL1_PFD0 | SYSPLL1_PFD1 | SYSPLL1_PFD2 |

- `LDBPLL` is **not an input of any CCM root**, and only `DISP1PIX` can
  select `VIDEOPLL1` (read from source).
- `CLOCK_SourceSetBypass` handles only AUDIOPLL1, AUDIOPLL2 and VIDEOPLL1, the
  "PLLs available as CCM clock root inputs" (read from source).

### 2.3 Block-control clocks below the SM (read from source, C2 `drivers/clk/imx/imx95-blk-ctrl.c`)

These clocks are bits in display-mix registers that Linux writes over MMIO.
They are **not SCMI clocks**. Access to them is TRDC access to
`BLK_CTRL_DISPLAYMIX` / `LVDS`, not an SCMI clock permission (inferred from
C1's resource split, see [imx95-sm-display-lm.md](imx95-sm-display-lm.md)).

| Provider (DT node) | Clock | Parent(s) | Type, flags |
|---|---|---|---|
| `lvds_csr` (`fsl,imx95-dispmix-lvds-csr`, 0x4b0c0000) | `ldb_phy_div` | `ldbpll` | divider, power of two, 1 bit; **no `CLK_SET_RATE_PARENT`** |
| | `lvds_ch0_gate`, `lvds_ch1_gate` | `ldb_phy_div` | gate, `CLK_SET_RATE_PARENT` |
| | `lvds_di0_gate`, `lvds_di1_gate` | `ldb_pll_div7` | gate, `CLK_SET_RATE_PARENT` |
| `dispmix_csr` (`fsl,imx95-dispmix-csr`, 0x4b010000) | `disp_engine0_sel`, `disp_engine1_sel` | `videopll1`, `dsi_pll`, `ldb_pll_div7`, `dummy` | 2-bit mux at bits [1:0] / [3:2], `CLK_SET_RATE_NO_REPARENT \| CLK_SET_RATE_PARENT` |
| top-level DT (`imx95.dtsi`) | `ldb_pll_div7` (`ldb_pll_pixel`) | `<&scmi_clk IMX95_CLK_LDBPLL>` | `fixed-factor-clock`, div 7. A DT fixed-factor clock is registered with flags 0 (`clk-fixed-factor.c`, `_of_fixed_factor_clk_setup`), so **it does not pass rate requests to `LDBPLL`** |

Resulting LVDS chain (inferred from the rows above):
`LDBPLL_VCO` → `LDBPLL` (SCMI) → `÷7` (`ldb_pll_div7`) → `lvds_diN_gate` → LDB
pixel input, and `LDBPLL` → `ldb_phy_div` → `lvds_chN_gate` → LVDS PHY serial
clock. The DPU frame generator takes its pixel clock from `ldb_pll_div7`
through the `disp_engineN_sel` mux. For LVDS, **`DISP1PIX` and `VIDEOPLL1`
are not in the path**. They matter only for MIPI-DSI.

## 3. How Linux 6.6 sets them

### 3.1 DT (read from source, C2 `imx95.dtsi`; same values at C3 per [imx95-dpu-source-6.6.md](imx95-dpu-source-6.6.md))

| Node | clocks | assigned-clocks / parents / rates | power-domains |
|---|---|---|---|
| `dpu` (`nxp,imx95-dpu`, disabled) | `pix` = `DISP1PIX`, `apb` = `DISPAPB`, `axi` = `DISPAXI`, `ocram` = `DISPOCRAM`, `ldb` = `ldb_pll_pixel`, `ldb_vco` = `LDBPLL_VCO` | none | `IMX95_PD_DISPLAY` |
| `dispmix_csr` (no status) | `DISPAPB` | `DISPAXI`, `DISPOCRAM`, `DISPAPB` ← `SYSPLL1_PFD1`, `SYSPLL1_PFD1`, `SYSPLL1_PFD1_DIV2` at 400 MHz, 400 MHz, 133.333 MHz | `IMX95_PD_DISPLAY` |
| `lvds_csr` (no status) | `DISPAPB` | none | `IMX95_PD_DISPLAY` |
| `ldb` (`fsl,imx95-ldb`, disabled) | `ldb_di0/1` = `PIX_DI0/1_GATE`, `ldb_ch0/1` = `LVDS_CH0/1_GATE` | none in the SoC dtsi | none |
| `ldb0_phy`, `ldb1_phy` (disabled) | `apb` = `DISPAPB` | none | `IMX95_PD_DISPLAY` |
| `mipi_dsi` (disabled) | `cfg`, `bypass`, `ref`, `pclk` = `CAMAPB`, `pix` = `DISP1PIX` | `MIPIPHYCFG` ← 24M, `MIPIPHYPLLBYPASS` ← `VIDEOPLL1`, `MIPIPHYPLLREF` ← 24M; no rates | `IMX95_PD_CAMERA` |

10" LVDS panel file (C2 `imx95-19x19-verdin-panel-cap-touch-10inch-lvds.dts`,
exact lines 43-46): on `&ldb`, `assigned-clocks = <&scmi_clk
IMX95_CLK_LDBPLL_VCO>, <&scmi_clk IMX95_CLK_LDBPLL>; assigned-clock-rates =
<2986200000>, <497700000>; status = "okay";` (read from source). 2986.2 MHz ÷
6 = 497.7 MHz, and 497.7 MHz ÷ 7 = 71.1 MHz, the typical pixel clock of the
`logictechno,lt170410-2whc` panel (arithmetic, inferred). As the other notes
say, this file is NXP's 19x19 EVK adaptation, not the Mallow overlay, so these
rates are the expected shape and not proof of the board's values (inferred).

### 3.2 Runtime (read from source, C2)

| Who | What it calls | Effect |
|---|---|---|
| Device core, at `ldb` probe | applies `&ldb`'s `assigned-clocks`/`-rates`, which become SCMI `CLOCK_RATE_SET` on `LDBPLL_VCO` and `LDBPLL` | generic kernel behaviour (`of_clk_set_defaults`); the call path itself was not re-read (inferred) |
| `imx95-ldb.c` `imx95_ldb_bridge_mode_set` | `clk_set_rate(clk_di[ch], adjusted_mode->clock * 1000)`, return value ignored | the request climbs `lvds_diN_gate` (`CLK_SET_RATE_PARENT`) and stops at the fixed-factor `ldb_pll_div7`, so **no SCMI rate change reaches `LDBPLL` at mode set** (inferred from §2.3). The panel mode has to match the DT PLL rate |
| `imx95-ldb.c` `atomic_enable` | `clk_prepare_enable(clk_ch)`, then `clk_di` | blk-ctrl gate bits. The SCMI enable of `LDBPLL` follows only through the parent chain (inferred) |
| `imx95-ldb.c` `mode_valid` | rejects > 330 MHz, and > 165 MHz on a single link; `clk_round_rate` check only if the last bridge has DETECT and EDID | a fixed panel skips the rounding check (inferred) |
| `dpu95-framegen.c` `dpu95_fg_cfg_videomode` | `if (enc_is_dsi) clk_set_rate(dpu->clk_pix, crtc_clock * 1000);`, then `regmap_update_bits(CLOCK_CTRL, DSIP_CLK_SEL(fg->id), enc_is_dsi ? CCM : LVDS_PLL_7(fg->id))` | LVDS: no rate call, and the source mux is written **directly** to `LVDS_PLL_7` (value 2). That bypasses the `disp_engineN_sel` clock that `imx95-blk-ctrl.c` registers on the same bits (inferred: same bit layout, values 0/1/2 match the parent order) |
| `dpu95-framegen.c` `dpu95_fg_enable_clock` | DSI: `clk_prepare_enable(clk_pix)`; otherwise `clk_prepare_enable(clk_ldb_vco)` then `clk_ldb` | for LVDS, Linux SCMI-enables `LDBPLL_VCO`, and `LDBPLL` through `ldb_pll_div7` (inferred) |
| `dpu95-drv.c` runtime PM | enables only `axi`, `apb`, `ocram` | |
| `dpu95-kms.c` | `drm_simple_encoder_init(..., DRM_MODE_ENCODER_NONE)` | the DPU's own encoders are not DSI, so `enc_is_dsi` is false unless something else sets the type (not found in the files read) |
| `clk-scmi.c` | `.flags = CLK_GET_RATE_NOCACHE` only, no `CLK_SET_RATE_PARENT`; `set_rate` → `rate_set`, `set_parent` → `parent_set`; `determine_rate` returns the request unchanged; **no permission or restriction query** | Linux learns of a denial only as an error return from the firmware (read from source) |

The DISP AXI/APB/OCRAM rates in `dispmix_csr`'s `assigned-clocks` need SCMI
rate and parent rights that AP-NS does not hold. In `mx95evk.cfg` those clocks
belong to LM0 and are "Modified via PERF protocol" (read from source, C1). The
clock core skips the firmware call when the rate and parent already match.
The SM boot defaults probably equal the DT values, so no denied call is made
(inferred, not checked).

## 4. SM permission model for clocks (read from source, C1)

### 4.1 Levels (`rpc_scmi_config.h`; semantics from `sm/doc/config.md`)

`NONE 0 < GET 1 < NOTIFY 2 < SET 3 < PRIV 4 < EXCLUSIVE 5 < ALL 255`. Each
level includes the lower ones. `ALL` "is currently equivalent to"
`EXCLUSIVE`. `SET` means a setting per agent, aggregated across agents and
LMs. `EXCLUSIVE` means "no aggregation is done. If two agents (regardless of
LM) make calls they will be overwriting each other". Also, "configtool and SM
implementation do not prevent violating the isolation by giving multiple
agents access where aggregation does not occur". In a `.cfg`, an API line
`CLOCK_x ALL` grants `clk=all`, and a resource line `VIDEO_PLL1 OWNER` expands
to TRDC rights plus full API rights on the clocks the resource maps to. The
`OWNER` macro is defined per LM section. In `sm/doc/examples/example.cfg` it
is `OWNER: perm=sec_rw, api=all` for LM0 (line 22) and `OWNER: perm=rw,
api=all` for LM1 (line 46) (read from source). That `mx95evk.cfg` defines
`OWNER` the same way for AP-NS is inferred.

Resource-to-clock mapping (`ccmsrcgpcmix.cfg`, approx. lines 48-50 and
63-64): `CLOCK_DISP1PIX: CLK_DISP1PIX` (likewise 2PIX and 3PIX), `LDB_PLL:
CLK_LDBPLL, CLK_LDBPLL_VCO`, `VIDEO_PLL1: CLK_VIDEOPLL1, CLK_VIDEOPLL1_VCO`.
This file has no `MIPI_PHY` clock resource.

In `mx95evk.cfg`, AP-NS (Linux) has `CLOCK_DISP1PIX/2PIX/3PIX ALL` and
`LDB_PLL ALL` as API lines, and `VIDEO_PLL1 OWNER` as a resource line. The
M7 agent (`SCMI_AGENT0`, LM1) and AP-S have no display clock line. So the M7
has `NONE` on every clock in this note (read from source; full table in
[imx95-zephyr-feasibility.md](imx95-zephyr-feasibility.md) §2).

### 4.2 Check per SCMI clock message (`rpc_scmi_clock.c`)

| Message | Required level | If below |
|---|---|---|
| `CLOCK_ATTRIBUTES` | none (sets the `RESTRICTED` attribute bit when below `EXCLUSIVE`) | answered |
| `CLOCK_DESCRIBE_RATES`, `CLOCK_RATE_GET`, `CLOCK_CONFIG_GET`, `CLOCK_POSSIBLE_PARENTS_GET`, `CLOCK_PARENT_GET` | none | answered |
| `CLOCK_GET_PERMISSIONS` (0xF) | none; reports `STATE` at `>= SET`, `RATE \| PARENT` at `>= EXCLUSIVE` | answered |
| `CLOCK_CONFIG_SET` enable/disable | `SET` | `SM_ERR_DENIED` |
| `CLOCK_CONFIG_SET` extended config | `EXCLUSIVE` | `SM_ERR_DENIED` |
| `CLOCK_RATE_SET` (sync only; async is `INVALID_PARAMETERS`) | `EXCLUSIVE` | `SM_ERR_DENIED` |
| `CLOCK_PARENT_SET` | `EXCLUSIVE` | `SM_ERR_DENIED` |

Every handler first returns `SM_ERR_NOT_FOUND` for `clockId >= SM_NUM_CLOCK`.

### 4.3 Aggregation and arbitration

- **Enable is aggregated** at two levels. `rpc_scmi_clock.c` keeps one bit
  per agent in `s_clockState[clockId]`, ORed over the agents of the caller's
  SCMI instance. `lmm_clock.c` keeps `s_clockState[clockId][lmId]`, ORed over
  all LMs, and calls the device only on a change. One side's disable cannot
  gate a clock the other side has enabled. `LMM_ClockInit` marks the boot
  clock list as enabled by LM0 (read from source).
- **Rate and parent set are last-writer-wins.** `ClockRateSet` and
  `ClockParentSet` check only `clkPerms >= EXCLUSIVE`, record the caller in
  `s_clockAgent[clockId]` ("Mark owning agent", overwritten every time), and
  pass the call to the device: `LMM_ClockRateSet` is `/* Not shared, just
  passthru to device */ return SM_CLOCKRATESET(...)`. The header comment of
  `ClockRateSet` lists `SM_ERR_DENIED` for "other users of the clock", but no
  code returns it for that reason (read from source).
- **Device layer.** `DEV_SM_ClockRateSet` sends a PLL to
  `CLOCK_SourceSetRate`, which calls `FRACTPLL_SetRate(CLOCK_PLL_VIDEO1 |
  CLOCK_PLL_LDB, vco?, rate)`. Its only gate is `CLOCK_SourcePdIsOn`, and
  there is no in-use or enabled-children check. With floor rounding it fails
  if the read-back rate is below the request ("Overclocking of clock sources
  is not supported"). A root rate goes to `CCM_RootSetRate`, and the summary
  of `dev_sm_clock.c` names only a reserved-clock guard (read from source,
  partly from the summarised part of `dev_sm_clock.c`).
- **PLL write sequence** (`devices/MIMX9/drivers/fsl_fract_pll.c`,
  `FRACTPLL_SetRate` from line 364; read from source):
  - *Output (ODIV) rate*, e.g. `LDBPLL` or `VIDEOPLL1`: ODIV is computed
    from the current VCO rate and clamped to 2-255. It is then written by a
    plain read-modify-write of `DIV` (lines 414-460). There is no power-down,
    no output gating and no wait, so the output changes **live**.
  - *VCO rate*, e.g. `LDBPLL_VCO`: first `FRACTPLL_DynamicSetRate`
    (lines 317-359). On a powered fractional PLL with the default
    denominator, this rewrites only `NUMERATOR` (MFN) live, if the new rate
    keeps the current integer part "to avoid relock". Otherwise
    `FRACTPLL_UpdateRate` (lines 232-312) clears `CLKMUX_EN` (output gated),
    clears `POWERUP`, writes `DIV` (MFI, RDIV 0, ODIV kept) and MFN, powers
    up, and polls `PLL_LOCK`. It re-enables the output only after lock. On a
    lock timeout it returns false and leaves the PLL powered down with its
    output gated.
  - No path refuses the change because the PLL is enabled or in use. A PLL
    in bypass is reported as disabled (line 63).
- **Root rate** (`devices/MIMX9/drivers/fsl_ccm.c`, `CCM_RootSetRate`,
  lines 145-230; read from source): this changes only the root divider of
  the currently selected mux input (lines 152, 223-225). It never changes the
  mux; that is `CCM_RootSetParent`. The divider is clamped to at most 256.
  There is no enable or in-use check; the only guard is
  `rootIdx < CLOCK_NUM_ROOT && rate != 0` (line 149).
- **LM reset.** `LMM_DoShutdown` → `LMM_RpcReset` → `RPC_SCMI_Reset` →
  `BaseResetAgentConfig` for every agent of that LM: it revokes device access
  (`SM_RDCACCESSSET(..., false)`), then `ClockResetAgentConfig` clears the
  agent's enable bits. If that agent was the last rate/parent setter, it calls
  `LMM_ClockReset`, which restores the boot parent and rate **only if the
  clock is off in hardware**. Otherwise it returns `SM_ERR_BUSY`, which the
  caller ignores, and the owner mark is cleared anyway. On reboot,
  `LM_ClockStart` re-applies the start-list parents and rates (read from
  source).

## 5. Could the M7 set them, or only read them?

| Case | Answer | Tag |
|---|---|---|
| M7 under stock `mx95evk`: read rates, parents, enable state and permissions of `LDBPLL(_VCO)`, `VIDEOPLL1(_VCO)`, `DISP1PIX` | **Yes.** These messages have no permission check, so `NONE` is enough. A Zephyr probe could read Linux's live LVDS PLL rate over SCMI without changing anything, provided the M7's SCMI channel works on the Verdin SM | read from source (C1); Verdin SM equality inferred |
| M7 under stock: enable any display clock | **No**, `SM_ERR_DENIED` (needs `SET`). Zephyr's `scmi_clock_on_off` then returns 0 without sending the request (see [imx95-m7-scmi.md](imx95-m7-scmi.md)), so the denial is silent | read from source (C1 + Zephyr note) |
| M7 under stock: set rate or parent | **No**, `SM_ERR_DENIED` (needs `EXCLUSIVE`) → Zephyr `-EACCES` | read from source |
| M7 granted `SET` only (e.g. `LDB_PLL SET` in its agent) | It could hold the PLL on alongside Linux (aggregated), but not change the rate. This is the only level that lets both sides share a display clock safely | inferred from §4.3 |
| M7 granted `ALL`/`EXCLUSIVE` while AP-NS keeps `ALL` | Both may set the rate. The SM does not arbitrate, so the last writer wins | read from source (C1 config.md + code) |
| M7 is the sole display owner (cfg of [imx95-sm-display-lm.md](imx95-sm-display-lm.md)) | Full control. For LVDS the M7 needs `LDB_PLL` (rate on `LDBPLL_VCO`/`LDBPLL`, enable), `PD_DISPLAY`, and TRDC access to `dispmix_csr` (engine source select, `CLOCK_CTRL`) and `lvds_csr` (`ldb_phy_div` and the `lvds_ch`/`lvds_di` gates). `DISP1PIX` and `VIDEOPLL1` are needed only for DSI. `DISPAPB/AXI/OCRAM` stay with LM0. They are probably held on from boot by LM0's clock list, which was not checked | inferred from §2-§4 |

### What happens if both sides touch the PLL (inferred unless tagged)

- **Rate.** Suppose Linux has set `LDBPLL` to 497.7 MHz for the 71.1 MHz
  panel and the M7, given `EXCLUSIVE`, writes another rate. The SM applies it
  with no check (read from source). An `LDBPLL` (ODIV) write changes the
  output live. An `LDBPLL_VCO` write either retunes MFN live or gates the
  output and relocks it, and a failed lock leaves it off (read from source,
  §4.3). The LDB serial clock and the DPU pixel clock both derive from
  `LDBPLL`. Linux's running LVDS stream therefore jumps in frequency, or
  stops for the relock time, or stays dark after a failed lock (inferred
  from that sequence). Linux does not notice: `ldb_pll_div7` does not forward rate requests, the LDB
  driver ignores `clk_set_rate`'s return, and `CLK_GET_RATE_NOCACHE` only
  shows the new rate when someone reads it. Nothing restores 497.7 MHz until
  the next `ldb` probe.
- **Video PLL.** The same holds for `VIDEOPLL1` under DSI. Zephyr's DSI
  `soc_init()` writes `VIDEOPLL1` to 446.333 MHz and `DISP1PIX` to
  148.444 MHz (see [imx95-m7-scmi.md](imx95-m7-scmi.md)). With rights it
  would override whatever Linux set. Without rights it is denied. `DISP1PIX`
  is the only root that can use `VIDEOPLL1` (read from source), so a fight
  over `VIDEOPLL1` is a fight over the DSI pixel clock. The LVDS panel does
  not use it.
- **Enable.** This is safe in both directions at `SET`: aggregation keeps the
  clock on while either side holds it (read from source).
- **M7 stop or reset.** The M7's enables are dropped, which is harmless while
  Linux holds the clock. Its rate change is rolled back only if it was the
  last setter and the clock is off. With Linux holding `LDBPLL` on, the reset
  returns `BUSY` and the M7's rate stays (read from source).

Conclusion for M2 (inferred): the M7 can **read** the display clocks today.
It can **set** them only if the SM config grants `EXCLUSIVE`. Doing that
while Linux keeps the LVDS route makes two uncoordinated writers on one PLL,
and the SM explicitly leaves that to the clients. Sharing is safe only at
`SET` (enable only), with Linux as the sole rate writer. So an M7 that drives
the panel itself needs the panel's PLL rate fixed by one side: either Linux
leaves it at the DT rate and the M7 only reads it, or the M7 owns the whole
display (the "instead of Linux" design).

## 6. Open points

- Read the Toradex SM `.cfg` for the Verdin iMX95 and the Mallow LVDS overlay
  (git.toradex.com was not reachable). This note's permission statements are
  for `mx95evk`.
- Get exact line numbers from a local checkout of `af1e3702`. A clone exists,
  but reading the pinned commit from it was not permitted on this node.
- Confirm on the board, read-only, that Linux's `clk_summary` shows `ldbpll`
  at 497.7 MHz and `disp1pix` unused for LVDS. This belongs to an observation
  item, not to this note.
- A Zephyr SCMI `CLOCK_RATE_GET` / `CLOCK_GET_PERMISSIONS` on IDs 37, 38, 18,
  19 and 80 from the M7 would show the Verdin SM's real grants without
  changing anything. It is a candidate for the M7 probe app (inferred).
