# i.MX95 display power domains and resets under the System Manager

Source study, dated 2026-11-28. Scope: SoW §5.1 / §9 M2 (WP3). This note asks
what the Cortex-M7 needs to have the display mix powered and the DPU/LDB out of
reset, who may switch these, and what Linux does to them. No board was
accessed. Nothing was built or run.

Each finding is tagged **read from source** (with file:line at the pin below)
or **inferred**. One recorded observation made by another item is cited as
such; this item observed nothing.

It does not repeat:

- [imx95-m7-scmi.md](imx95-m7-scmi.md): the Zephyr M7 SCMI client, the
  `soc_init()` power-on of `IMX95_PD_DISPLAY`/`IMX95_PD_CAMERA`, and the
  transport.
- [imx95-sm-display-lm.md](imx95-sm-display-lm.md): the stock `mx95evk` LM
  and resource assignment (which LM owns `DC*`, `LVDS`,
  `BLK_CTRL_DISPLAYMIX`), the proposed M7 LM cfg, and why it cannot be
  installed on the shared board.
- [imx95-zephyr-feasibility.md](imx95-zephyr-feasibility.md): the overall
  ownership model.
- [imx95-m7-lvds-route.md](imx95-m7-lvds-route.md) and
  [imx95-dpu-source-6.6.md](imx95-dpu-source-6.6.md): the display nodes of the
  Linux DT and the DPU and LDB register paths.

## 1. Sources and pins

| Id | Source | Pin | Notes |
|---|---|---|---|
| SM | NXP imx-sm, <https://github.com/nxp-imx/imx-sm> | tag `rel_imx_sm_2025q3` = commit `af1e37026c6ba19cdca98fd6e91494efd3e7b5ec`. The same commit is `lf-6.6.52-2.2.2^{}` (checked with `git ls-remote` on 2026-11-28) | Read over HTTPS from `https://raw.githubusercontent.com/nxp-imx/imx-sm/af1e37026c6ba19cdca98fd6e91494efd3e7b5ec/<path>` |
| LX | NXP linux-imx, <https://github.com/nxp-imx/linux-imx> | tag `lf-6.6.52-2.2.2`. The commit is `5a0a5e71d2bd` as recorded in [imx95-zephyr-feasibility.md](imx95-zephyr-feasibility.md) (S6); it was not re-resolved here | Read from `https://raw.githubusercontent.com/nxp-imx/linux-imx/lf-6.6.52-2.2.2/<path>` |

Limits, stated plainly:

- **SM line numbers checked against a clone.** On 2026-11-28 a shallow clone
  of `rel_imx_sm_2025q3` (HEAD `af1e3702`) was read. These SM citations were
  checked line by line against it and are exact: `dev_sm_power.h`,
  `dev_sm_power.c`, `dev_sm.c`, `dev_sm_config.c`, `dev_sm_reset.h`,
  `rpc_scmi_power.c`, `rpc_scmi_reset.c`, `lmm_power.c`, `lmm_power.h`,
  `config_user.h`, `config_scmi.h`, `dev_sm_api.h` and `fsl_src.c`. The
  citations into `fsl_reset.c`, `rpc_scmi.c`, `rpc_scmi_base.c`, `lmm.c`,
  `lmm_sys.c`, `mx95evk.cfg`, `configtool.pl` and `config.md` were then
  checked with `git grep -n` on the same commit and corrected where they were
  off. Tree-wide searches below were also run with `git grep`. The `displaymix.cfg`
  lines below (39, 49, 50–51, 60, 64–65) match the tarball-exact numbers in
  [imx95-sm-display-lm.md](imx95-sm-display-lm.md).
- **LX line numbers were counted by hand.** The Linux files were read as raw
  HTTPS text and their lines counted by hand. For the LX files
  `scmi_pm_domain.c`, `imx95-blk-ctrl.c`, `imx95-ldb.c`,
  `phy-fsl-imx8mp-lvds.c`, `dpu95-drv.c` and `irq-imx-irqsteer.c`, the count
  matched GitHub's line total for each file. All other numbers could be off by
  one or two lines. Where no line was counted, the function name is given
  instead. Before quoting a number elsewhere, check it with `grep -n` on a
  checkout at the pin.
- **The stock config is not the board's config.** The board runs Toradex
  Reference Multimedia 7.8.0, kernel 6.6.157-7.8.0. Toradex's imx-sm commit
  and Verdin SM configuration were **not read**, and no public Toradex imx-sm
  fork was found (see [imx95-sm-display-lm.md](imx95-sm-display-lm.md) §1).
  The `mx95evk` permissions below are NXP's reference. That the Verdin image
  uses the same permissions is **inferred**.
- The NXP 6.6 tree stands in for the Toradex kernel. That they share these
  drivers is **inferred**, as in the docs linked above.

## 2. Power domains: IDs and what the display path sits on

| Finding | Citation | Tag |
|---|---|---|
| The SM has 23 power domains (`DEV_SM_NUM_POWER` = `PWR_NUM_MIX_SLICE`), with two states, `OFF` 0 and `ON` 1 | SM `devices/MIMX95/sm/dev_sm_power.h:59,62,100-101` | read from source |
| `DEV_SM_PD_CAMERA` = 3, `DEV_SM_PD_DISPLAY` = 13; the list runs from ANA 0 to WAKEUP 22 | SM `dev_sm_power.h:71-93` (CAMERA :74, DISPLAY :84). The numeric values come from the code comments; `fsl_power.h`, which holds the real values, was not read | read from source |
| **No separate LVDS, MIPI or PHY power domain exists.** The LVDS PHYs, the LDB and the DSI PHY can only be powered by the mix domains | SM `dev_sm_power.h:71-93` | read from source |
| Linux uses the same numbers: `IMX95_PD_CAMERA 3`, `IMX95_PD_DISPLAY 13` | LX `include/dt-bindings/power/fsl,imx95-power.h:12,22` | read from source |
| In `displaymix.cfg`, every DISPLAYMIX resource names `PD_DISPLAY`: `DC` (:39-41; it is also the only one that names `PD_CAMERA`), `DC0` (:49), `DC1` (:50-51), `BLK_CTRL_DISPLAYMIX` (:60), `LVDS` (:64-65), `GPV_DISPLAY` (:66) | SM `devices/MIMX95/configtool/displaymix.cfg` | read from source |
| In the Linux SoC dtsi, these nodes are on `<&scmi_devpd IMX95_PD_DISPLAY>`: `dispmix_csr` (syscon@4b010000), `displaymix_irqsteer`, `lvds_csr`, `ldb0_phy`, `ldb1_phy`, `dpu`. `mipi_dsi` and `display_master_csr` are on `IMX95_PD_CAMERA`. `ldb`, `display_pixel_link` and `pixel_interleaver` have no `power-domains` and depend on their parent's domain | LX `arch/arm64/boot/dts/freescale/imx95.dtsi`, by node label; lines not counted. For dtsi line ranges, see [imx95-dpu-source-6.6.md](imx95-dpu-source-6.6.md) | read from source |

So an LVDS scanout from the M7 needs **one** power domain: `PD_DISPLAY` (13).
`PD_CAMERA` (3) is needed only for the DSI branch, and through `DC`'s TRDC
entry. Whether a DC used only over LVDS still needs `PD_CAMERA` on is
**inferred**: `DC` lists it, but the SM uses that list for config generation
and TRDC, not as a runtime dependency.

## 3. What the SM does when PD_DISPLAY changes state

| Finding | Citation | Tag |
|---|---|---|
| `DEV_SM_PowerStateSet` has the same body for every domain. It checks the fuse (`DEV_SM_FusePdDisabled`) and the parent (`PWR_IsParentPowered`); on B0 silicon and later it then runs `PWR_MixSsiBlockingSet(..., false)`, `SRC_MixSoftPowerUp`, `DEV_SM_PowerUpPost` and `PWR_MixSsiBlockingUpdate`. Power-off is refused while a child domain is on (`SM_ERR_POWER`); otherwise it runs `DEV_SM_PowerDownPre` and then `SRC_MixSoftPowerDown` | SM `dev_sm_power.c:165-230` (fuse :168, parent :179, B0 path :191-198, child check :209, power-down :221-224, error :230) | read from source |
| `DEV_SM_PowerStateGet` reads the hardware (`SRC_MixIsPwrReady`), not a vote | SM `dev_sm_power.c:246,258-260` | read from source |
| On power-up, `case DEV_SM_PD_DISPLAY:` calls `DEV_SM_DisplayConfigLoad()` | SM `devices/MIMX95/sm/dev_sm.c:228-229`, in `DEV_SM_PowerUpPost` (:201) | read from source |
| `DEV_SM_DisplayConfigLoad` runs four steps: it reloads TRDC D (`DEV_SM_RdcLoad(DEV_SM_TRDC_D)`), applies the board's `SM_DISPLAY_CONFIG` address/value list (`CONFIG_Load`), runs an optional `SM_DISPLAY_CONFIG_FUNC`, and restores the LDB PLL context (`FRACTPLL_SetContext(CLOCK_PLL_LDB, ...)`) | SM `devices/MIMX95/sm/dev_sm_config.c:212-248` (:215, :218, :223, :226-230, :237-239) | read from source |
| On power-down, `DEV_SM_DisplayPowerDownPre` only saves the LDB PLL context (`FRACTPLL_GetContext`). It saves no DPU or BLK_CTRL registers | SM `dev_sm_config.c:786-803` (:790); state at :63, :66; hook called from `dev_sm.c:352-353` | read from source |
| At SM boot, `DEV_SM_Init` runs `DEV_SM_PowerUpPost` for each domain that is already on ("load state (BLK_CTRL, RDC, etc.)") | SM `dev_sm.c:148,162` | read from source |
| The SM code has **no hard-coded write to `BLK_CTRL_DISPLAYMIX`**. Camera and GPU have hard-coded writes (`DEV_SM_CameraConfigLoad`, `dev_sm_config.c:92-97`; `BLK_CTRL_GPUMIX->GPURESET`, `:276`); display does not | SM `dev_sm_config.c`, `dev_sm_power.c` (the only BLK_CTRL in it is AONMIX, :67-69) | read from source |
| Any SM write to display BLK_CTRL can only come from the board's `SM_DISPLAY_CONFIG` list or an `SM_DISPLAY_CONFIG_FUNC`. In `mx95evk` the list is **empty** (`{ SM_CFG_END }`). **No config or board in the tree defines `SM_DISPLAY_CONFIG_FUNC`**: a tree-wide search finds only the `#ifdef` and the call in `dev_sm_config.c` (MIMX95 :226, :230; MIMX94 :192, :196) | SM `configs/mx95evk/config_user.h:92-95`; `git grep -n SM_DISPLAY_CONFIG_FUNC` at the pin | read from source |

Consequences (**inferred** from the rows above):

- A power cycle of `PD_DISPLAY` loses all DPU, DISPLAY_BLK_CTRL, LVDS CSR and
  irqsteer register state. The SM restores only TRDC D, its board list and the
  LDB PLL. Whoever drives the display must reprogram everything else after
  each power-up, and has no notice that one happened (§4).
- In practice **DISPLAY_BLK_CTRL is programmed by the OS that owns the
  display**: Linux's `imx95-blk-ctrl` driver (§6), and on Zephyr the direct
  `PIXEL_LINK_CTRL` write in `soc_init()` (see
  [imx95-m7-scmi.md](imx95-m7-scmi.md)). The SM's part is TRDC plus an
  optional board list, and under `mx95evk` that list is empty.
- **A second ON vote does not reload a live display.** No layer checks
  whether the state changed:
  - `PowerStateUpdate` calls `LMM_PowerStateSet` ("LMM will check if
    changed", `rpc_scmi_power.c:827-828`).
  - `LMM_PowerStateSet` calls `SM_POWERSTATESET` ("device will check if
    changed", `lmm_power.c:107-108`).
  - `SM_POWERSTATESET` is `DEV_SM_PowerStateSet` (`sm/dev/dev_sm_api.h:188-191`;
    the board's `brd_sm.h:76` overrides only `SM_SYSTEMRESET`).
  - `DEV_SM_PowerStateSet` itself has no such check.

  The check lives in `SRC_MixSoftPowerUp`
  (`devices/MIMX9/drivers/fsl_src.c:615-692`). It runs the power-up sequence
  and returns `true` only when `FUNC_STAT_PSW_STAT` is set (:629-670).
  Otherwise it returns `false`, and `DEV_SM_PowerStateSet` then skips
  `DEV_SM_PowerUpPost` (`dev_sm_power.c:192-194`), so there is no TRDC D,
  config or LDB PLL reload. It always masks and restores the GPC LP handshake
  and switches to software power control and back (:624, :627, :673,
  :676-688). All of this is read from source. That a set `PSW_STAT` means
  "switch off" is **inferred** from the code's structure: the bit's meaning
  in the reference manual was not read.

## 4. Who may switch PD_DISPLAY: SCMI permission and aggregation

| Finding | Citation | Tag |
|---|---|---|
| SCMI `POWER_STATE_SET` needs `pdPerms[domain] >= SM_SCMI_PERM_SET`; otherwise it returns `SM_ERR_DENIED`. Only synchronous requests are supported | SM `sm/rpc/scmi/rpc_scmi_power.c:563,606-611,617-618` | read from source |
| `POWER_STATE_GET` returns the hardware state through `LMM_PowerStateGet` and then `DEV_SM_PowerStateGet`, not the caller's vote. **It checks no `pdPerms`**, only the length and the domain ID, so any agent may read any domain's state. `POWER_DOMAIN_ATTRIBUTES` reports sync set support only to an agent with `pdPerms >= SET` | SM `rpc_scmi_power.c:665-708` (:672, :678, :686), `:533-537`; `lmm_power.c:129-134` | read from source |
| **There are no power-state notifications.** `COMMAND_SUPPORTED_MASK` is `0x1003F`, with the comment "Sync responses only, no notificaions", and `POWER_ATTR_CHANGE(0)` | SM `rpc_scmi_power.c:58,524-526` | read from source |
| There are two levels of voting. Each agent's vote is kept in `s_powerState[domain][agent]` (SCMI layer, max over the agents of one SCMI instance), and each LM's vote in `s_powerState[domain][lm]` (LMM layer, max over all LMs). The domain goes off only when every LM votes off | SM `rpc_scmi_power.c:300,796-828` (:815, :818, :828); `sm/lmm/lmm_power.c:52,77-108` (:99, :101-104, :108) | read from source |
| The docs say the same: `SM_SCMI_PERM_SET` is "aggregated across agents and LM"; `SM_SCMI_PERM_ALL` "is currently equivalent to" `EXCLUSIVE`, which does no aggregation and gives "Handing a display over from one LM to another" as its use case; and the configtool and SM "do not prevent violating the isolation" | SM `sm/doc/config.md`, section SCMI (:229): :291-292, :298-299, :301-304, :306-310, :318-322 | read from source |
| For power, `ALL` and `SET` both pass the same `>= SET` check, and the code always aggregates power votes. The "no aggregation" wording of `EXCLUSIVE` therefore does not change power-domain behaviour in this code | the `rpc_scmi_power.c` and `lmm_power.c` rows above, read against `config.md` | inferred |
| In `mx95evk` `config_scmi.h`, `.pdPerms[DEV_SM_PD_DISPLAY]` and `.pdPerms[DEV_SM_PD_CAMERA]` are `SM_SCMI_PERM_ALL` **only for agent 2 "AP-NS"** (Linux). Agent 0 "M7" has only `.pdPerms[DEV_SM_PD_M7]`; agent 1 "AP-S" has only `PD_A55P` | SM `configs/mx95evk/config_scmi.h:452-453` (AP-NS), `:94` (M7), `:179` (AP-S) | read from source |
| `mx95evk.cfg` itself has no `PD_DISPLAY` or `PD_CAMERA` line. The generator turns `OWNER`/`api=` on a resource into `pd=<perm>` for each `PD_*` token that resource lists. AP-NS's `pdPerms[PD_DISPLAY]` therefore comes from its `OWNER` resources (`BLK_CTRL_DISPLAYMIX` at `mx95evk.cfg:599`, `DC*`, `LVDS`) and their `PD_DISPLAY` tokens in `displaymix.cfg` | SM `configs/configtool.pl`, `sub get_perms` (:2395) and the "Default resources to DEV_SM" step (:290); `mx95evk.cfg:599` | read from source (the mapping); inferred (that it is the only source of this entry) |
| The M7 LM stop and start lists (`PD_M7 start=1, stop=2`) and the AP LM lists (`PD_A55P`, `PD_A55C0..5`) do not name `PD_DISPLAY` | SM `mx95evk.cfg:341,348,353` (M7), `:442-448` (AP, mSel 0; mSel 1 and 2 at :456-462, :470-476) | read from source |

Consequences (**inferred**):

- **Under `mx95evk`, the M7 cannot switch the display domain at all.** Zephyr's
  `soc_init()` power-on of `IMX95_PD_DISPLAY` gets `SM_ERR_DENIED`, which
  Zephyr maps to `-EACCES`. This agrees with
  [imx95-m7-scmi.md](imx95-m7-scmi.md).
- **Granting the M7 its own vote is a one-token cfg change**, a `PD_DISPLAY`
  permission of at least `SET` for the M7 agent, and does not need to take
  the domain from Linux. Because votes are aggregated, the domain then stays on
  while either side wants it. This is the only SM-level guarantee that Linux
  cannot power the mix off under the M7. Like every cfg change, it means
  rebuilding and installing the SM, which the board rules forbid on the shared
  board ([imx95-sm-display-lm.md](imx95-sm-display-lm.md) §6).
- A shared vote keeps the **power** on. It does not stop Linux from **resetting
  or reprogramming** the blocks inside the mix. That still needs the
  TRDC/resource move of [imx95-sm-display-lm.md](imx95-sm-display-lm.md) §3,
  or Linux DT nodes that are disabled.

## 5. Resets: DISP0, DISP1, LVDS and DISPLAYMIX

| Finding | Citation | Tag |
|---|---|---|
| These SM reset domains exist: `DEV_SM_RST_DISP0_RESETN` 20, `DEV_SM_RST_DISP1_RESETN` 21, `DEV_SM_RST_LVDS_RESETN` 22, `DEV_SM_RST_CAMERAMIX` 34, `DEV_SM_RST_DISPLAYMIX` 45. The only reset type is `DEV_SM_RST_COLD` (no retention) | SM `devices/MIMX95/sm/dev_sm_reset.h:58` (count), `:84-86`, `:98`, `:109` | read from source |
| In hardware, DISP0, DISP1 and LVDS are bits of `SRC_XSPR_DISPLAYMIX->IRST_REQ_CTRL` (`IRST_0/1/2`); `DISPLAYMIX` is `SRC_XSPR_DISPLAYMIX->SLICE_SW_CTRL` | SM `devices/MIMX95/drivers/fsl_reset.c:263,274,285,508` | read from source |
| None of these is restricted. Only `AONMIX_TOP` and `AONMIX_M33` carry `RST_LINE_FLAG_RESTRICTED` | SM `fsl_reset.c:371,381`; `RST_ResetLineRestricted` :660-670 | read from source |
| SCMI `RESET` needs `rstPerms[domain] >= SM_SCMI_PERM_EXCLUSIVE`; otherwise it returns `SM_ERR_DENIED` | SM `sm/rpc/scmi/rpc_scmi_reset.c:503,519-520,541-546` | read from source |
| **`mx95evk` `config_scmi.h` has no `rstPerms` entry for any agent.** Neither Linux nor the M7 may assert or release any display reset over SCMI | SM `configs/mx95evk/config_scmi.h` (whole file) | read from source |
| The Zephyr M7 dtsi has no reset protocol node (see [imx95-m7-scmi.md](imx95-m7-scmi.md)) | — | read from source (in that doc) |

Consequences (**inferred**):

- In the stock configuration, the DPU and LDB come out of reset only through
  the mix power-up itself (`SRC_MixSoftPowerUp` on `PD_DISPLAY`). Any finer
  reset is the OS's job, done through block registers: for example
  `imx95-ldb.c` rewrites its control register to the POR value on runtime
  resume (§6). An M7 driver does not need SCMI reset rights for scanout. What
  it needs is the mix on and MMIO access through TRDC.
- On `mx95evk`, giving the M7 `DISP0/1` or `LVDS` reset rights would take
  `RST_* rst=exclusive` lines (the generator handles `RST_` tokens the same
  way as `PD_`, per `configtool.pl` `get_perms` at :2395). Nobody needs this
  for M2.

## 6. What Linux (NXP 6.6) does to the display domain

| Finding | Citation | Tag |
|---|---|---|
| `scmi_pm_domain` registers one genpd per SCMI domain, with `is_off` taken from the current SCMI state. Its callbacks call `state_set` and then `state_get`, and return `-EIO` if the two disagree. The only flag set is `GENPD_FLAG_ACTIVE_WAKEUP`, for `netc`. There is no `ALWAYS_ON` or `RPM_ALWAYS_ON` for `display` | LX `drivers/firmware/arm_scmi/scmi_pm_domain.c:25-53,94-111` (:36, :38-40, :103-104, :106-108, :110-111) | read from source |
| `late_initcall_sync(genpd_power_off_unused)` queues a power-off of every genpd unless `pd_ignore_unused` is on the command line. `genpd_power_off` refuses (`-EBUSY`) while any attached device is not runtime-suspended | LX `drivers/base/power/domain.c:1155-1184`, `genpd_power_off` :715-799 (:736-739, :758-764). Only the first ~40% of this file was read verbatim; these lines are in that part | read from source |
| DPU (`dpu95-drv.c`): `pm_runtime_enable` in probe, with no get or put. `runtime_suspend` turns off the blit engine and the ocram, apb and axi clocks. `runtime_resume` re-enables the clocks and **re-initialises the DPU**: QoS (`dpu95_set_qos`), the interrupt hardware and the submodules. System sleep goes through `drm_mode_config_helper_suspend/resume` | LX `drivers/gpu/drm/imx/dpu95/dpu95-drv.c:87,121-139,141-188` (:169, :177, :179), `:195-212`, `:216-217` | read from source |
| `dpu95-core.c` has no PM ops. It only takes and puts a runtime-PM reference on the irqsteer device around IRQ setup and teardown | LX `drivers/gpu/drm/imx/dpu95/dpu95-core.c`, `dpu95_irq_init`, `devm_dpu95_irq_exit` (lines not counted) | read from source |
| LDB (`imx95-ldb.c`): takes a runtime-PM reference in `mode_set` (:85) and drops it in `atomic_disable` (:226). `runtime_resume` writes 0 to the LDB control register ("disable LDB by resetting the control register to POR default", :511-512). There are no system-sleep ops | LX `drivers/gpu/drm/bridge/imx/imx95-ldb.c:85,226,482,501-519` | read from source |
| LVDS PHY (`phy-fsl-imx8mp-lvds.c`): `pm_runtime_enable` only, with no runtime callbacks and no get or put. The APB clock is switched around each `init`/`power_on`/`power_off` | LX `drivers/phy/freescale/phy-fsl-imx8mp-lvds.c:109-179,231` | read from source |
| `imx95-blk-ctrl.c` (compatibles `fsl,imx95-dispmix-lvds-csr` :440 and `fsl,imx95-dispmix-csr` :441): runtime PM is enabled only when `rpm_enabled` is set, and only the VPU data sets it (:96; the field is at :56; the check at :313-314). For dispmix and lvds CSR, probe leaves the APB clock on (:298, :354-355) and never puts a reference. System sleep saves and restores a single register (offset 0) | LX `drivers/clk/imx/imx95-blk-ctrl.c:56,96,216-220,245-249,298,313-314,354-355,401-425,432-433,436-445` | read from source |
| irqsteer: autosuspend after 100 ms; on runtime suspend it saves `CHANMASK` and gates the clock, and on resume it restores `CHANCTRL` and `CHANMASK`. System sleep uses `pm_runtime_force_suspend/resume` | LX `drivers/irqchip/irq-imx-irqsteer.c:118,125,258,291-293,331,337-367` | read from source |

Recorded observation, not made by this item: on 2026-11-27 the board's
`pm_genpd_summary` showed `display` **on**, with only
`4b400000.display-controller` (`active`) attached. `camera` was on, and the
`m7` domain was on with `remoteproc-cm7` attached
([observed-2026-11-27.txt](imx95-zephyr/observed-2026-11-27.txt), lines
324-334). Weston was active at the time.

What Linux does at runtime and in suspend (**inferred** from the rows above
and the observation):

- **While Weston scans out**, the DPU is runtime-active and genpd keeps
  `display` on. Linux's AP-NS vote is ON.
- **When the DPU runtime-suspends** (for example when the CRTC is disabled
  because Weston stopped or the output is off) and no other device on the
  domain is active, genpd powers `display` off. The SCMI vote of AP-NS
  becomes OFF. On the board, the DPU is the only device attached
  (observation above), so nothing else holds the domain. If the dispmix and
  lvds CSR were attached, they would keep it on: runtime PM is never enabled
  for them, so `pm_runtime_suspended()` is false. The Toradex DT apparently
  does not attach them, which is inferred from the summary.
- **At boot**, `genpd_power_off_unused` would power off `display` if no
  enabled device kept it on. On a Linux DT with the display nodes disabled
  (the design of [imx95-sm-display-lm.md](imx95-sm-display-lm.md) §4), Linux
  would vote the domain off about at `late_initcall_sync`, unless
  `pd_ignore_unused` is set.
- **In system suspend**, the drivers' sleep ops run and genpd powers off
  domains in the noirq phase. That last step is the standard genpd behaviour,
  and its code is in the part of `domain.c` that was not read verbatim. Linux
  votes OFF.
- **When Linux reboots or shuts down** (LM2 reset), `LMM_DoShutdown` calls
  `LMM_RpcReset`, then `RPC_SCMI_Reset`, then
  `RPC_SCMI_BaseDispatchReset`, then `RPC_SCMI_PowerDispatchReset`, then
  `PowerResetAgentConfig`. That last function turns every ON vote of LM2's
  agents into OFF and re-aggregates (SM `sm/lmm/lmm_sys.c:1070`, :1088;
  `sm/lmm/lmm.c:267`, :277; `sm/rpc/scmi/rpc_scmi.c:448-472` (:459);
  `rpc_scmi_base.c:359`, :1097; `rpc_scmi_power.c:288-292`;
  `PowerResetAgentConfig` :767-785; read from source). The display domain therefore drops
  when Linux resets, unless another LM votes ON. `LMM_PowerStateReset`
  (`lmm_power.c:120-124`) clears a vote without applying it. It is documented
  as "used **only** to clean up the LM state after testing"
  (`lmm_power.h:134-136`), so it plays no part here (read from source).

## 7. If Linux powers the domain off while the M7 is using it

Everything in this section is **inferred** from §3 to §6.

| M7 has a `PD_DISPLAY` vote? | What happens when Linux votes OFF (runtime suspend, unused-off at boot, system suspend, Linux reset) |
|---|---|
| **No** (stock `mx95evk`: the M7 agent has no `pdPerms[PD_DISPLAY]`) | The aggregate drops to OFF. The SM runs `DEV_SM_DisplayPowerDownPre` (saves only the LDB PLL) and then `SRC_MixSoftPowerDown`, and the whole DISPLAYMIX loses power. The M7 gets no SCMI notification (none is supported). Scanout stops and the panel loses its signal. DPU, BLK_CTRL, LVDS CSR and irqsteer state is lost, and the DPU interrupts (vblank/latch) stop. What an M7 access to the unpowered mix returns (bus error, hang, or reads of 0) was **not read** in any source |
| **Yes** (an M7 vote of at least `SET`) | The aggregate stays ON and the mix stays powered. Linux's own drivers still change the hardware on the way down. `dpu95_runtime_suspend` gates the DPU's axi, apb and ocram clocks, which are Linux's SCMI clocks (see [imx95-m7-scmi.md](imx95-m7-scmi.md)), and on resume Linux re-initialises the DPU submodules and QoS and writes 0 to the LDB control register. **A vote does not protect the M7 from Linux's driver activity.** The display nodes must be disabled in Linux, or the resources moved by TRDC |
| Either | The M7 can poll `POWER_STATE_GET` to see that the mix went off. It returns the hardware state and needs no permission (§4), so this works under stock `mx95evk` too. The M7 must then reprogram everything outside TRDC D, `SM_DISPLAY_CONFIG` and the LDB PLL |

Summary for M2:

1. On the shared board with stock-like SM permissions, the M7 can neither keep
   the display domain on nor switch it. Scanout from the M7 would depend on
   Linux holding `display` on, which in practice means Weston or another
   Linux client keeping the DPU runtime-active. That conflicts with the M7
   driving the same DPU.
2. A safe M7 ownership needs, at the SM level, an M7 `PD_DISPLAY` vote plus
   the TRDC/resource move, and, at the Linux level, disabled display nodes (or
   `pd_ignore_unused` as a stop-gap against unused-off). Both are boot-chain
   or rootfs changes that `docs/imx95-board.md` forbids on this board.
3. No reset rights are needed for scanout. The mix power-up is the reset.

## 8. Not done / open

- Resolved on 2026-11-28 against the clone, and folded into §3, §4 and §6:
  - the `mx95evk` `SM_DISPLAY_CONFIG` list (empty);
  - a second ON vote on a powered slice (it does not reload the display);
  - the permission `POWER_STATE_GET` needs (none);
  - the callers of `LMM_PowerStateReset` (test only);
  - `SM_DISPLAY_CONFIG_FUNC` (defined nowhere in the tree).
- The SDK `fsl_power.h` values.
- The meaning of `FUNC_STAT_PSW_STAT` in the reference manual.
- The Toradex Verdin SM configuration and the Toradex kernel deltas to the
  drivers in §6.
- The second half of `drivers/base/power/domain.c` (`pm_genpd_init`, noirq
  suspend), which was read only as a summary.
- Every hand-counted LX line number should be checked with `grep -n` on a
  linux-imx checkout at `lf-6.6.52-2.2.2`.
