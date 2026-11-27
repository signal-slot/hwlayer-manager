# i.MX95: what NXP's 6.6 kernel shows userspace about the System Manager

Date of this note: 2026-11-28. Scope: gate check (c) of
`docs/imx95-zephyr-feasibility.md` section 8, "evidence that the SM lets the
Linux/TF-A side start and stop `CPU_M7P`". Source study only: no board was
accessed and nothing was run. The question is which paths a read-only user on
the Verdin iMX95 could read, what each would show, and whether it could settle
check (c).

Tags: **read from source (repo@pin)** or **inferred**. No path below is
**observed**.

## 1. Sources, pins and their limits

| Source | Pin | Notes |
|---|---|---|
| NXP kernel, <https://github.com/nxp-imx/linux-imx> | tag `lf-6.6.52-2.2.2` = commit `5a0a5e71d2bd130577c7330cbb3764c68ebcb3dc` (also the tip of `lf-6.6.y` when read); no `lf-6.6.157*` tag exists in the tag list | The board runs Toradex's 6.6.157-7.8.0 kernel, a different tree. The closest NXP tag is used, so **every kernel statement below is a statement about NXP 6.6.52, and whether the Toradex kernel matches is inferred (unchecked)**. |
| NXP TF-A, <https://github.com/nxp-imx/imx-atf> | tag `lf-6.6.52-2.2.2`, commit `8ec7e38031f8c022a9760a8da77bdc6e1938db8c` (from the tag list page; not re-derived with `git ls-remote`) | Which TF-A the Toradex image ships is not read. |
| NXP System Manager, <https://github.com/nxp-imx/imx-sm> | tag-less `master` commit `af1e37026c6ba19cdca98fd6e91494efd3e7b5ec` (`rel_imx_sm_2025q3`), the pin already used in `docs/imx95-sm-display-lm.md` | Toradex's SM build is not read (see that document, section 1). |

How the files were read: through an HTTPS page-fetch tool (no `git clone`
was possible here), not a checkout. The tool returns no line numbers, so
sources are cited by file and function, not by line. For some long files
(`arm_scmi/driver.c`, the imx-sm `rpc_scmi_*.c` files, `imx95.dtsi`) the tool
returned the first part verbatim and a model-written summary of the rest.
Statements that rest on such a summary are marked "(summary)" and should be
re-read from a checkout before a decision depends on them.

## 2. What the kernel has, and does not have, at the pin

All read from source (linux-imx@5a0a5e71).

- `include/linux/scmi_nxp_protocol.h` defines two vendor protocol IDs only:
  `SCMI_PROTOCOL_IMX_BBM = 0x81` and `SCMI_PROTOCOL_IMX_MISC = 0x84`. There is
  **no** LMM (0x80) or CPU (0x82) protocol, **no** `imx-sm-lmm.c` or
  `imx-sm-cpu.c`, and no `sm-lmm.c` or `sm-cpu.c` (404s under
  `drivers/firmware/arm_scmi/`, `.../vendors/imx/` and `drivers/firmware/imx/`).
- `drivers/firmware/arm_scmi/imx-sm-misc.c` implements only message IDs
  `MISC_CTRL_SET 0x3`, `MISC_CTRL_GET 0x4` and `MISC_CTRL_NOTIFY 0x8`
  (plus the generic protocol attributes). It never sends
  `MISC_DISCOVER_BUILD_INFO` (0x6), `MISC_CFG_INFO` (0xC), `MISC_BOARD_INFO`
  (0xE), `MISC_SI_INFO` (0xB), `MISC_SYSLOG` (0xD) or `MISC_RESET_REASON` (0xA).
- `drivers/firmware/imx/sm-misc.c` (driver `scmi-imx-misc-ctrl`, device id
  `imx-misc-ctrl`) exports `scmi_imx_misc_ctrl_set/get` and reads the DT
  property `wakeup-sources`. It creates no sysfs or debugfs entry.
- `imx-sm-bbm.c` (RTC, GPR, button) creates no sysfs or debugfs entry.
- `drivers/remoteproc/imx_rproc.c` has **no SCMI code at all**. For
  `fsl,imx95-cm7` (`imx_rproc_cfg_imx95_m7`, method `IMX_RPROC_SMC`) start and
  stop are `arm_smccc_smc(IMX_SIP_RPROC = 0xC2000005, START 0x00 / STOP 0x02,
  ...)` and the result is `res.a0`. There is no rights pre-check; permission is
  decided in firmware and Linux only sees a return code.

Consequence (inferred from the above): the Linux agent never speaks the CPU or
LMM protocols itself, so no kernel interface at this pin can report the CPU
protocol's permission state to userspace.

## 3. What TF-A does with the SMC (the real start/stop path)

All read from source (imx-atf@8ec7e38).

- `plat/imx/common/include/imx_sip_svc.h` names `0xC2000005` `IMX_SIP_SRC`
  (`M4_START 0x00`, `M4_STARTED 0x01`, `M4_STOP 0x02`,
  `M4_RESET_ADDR_SET 0x03`). Dispatch is in `plat/imx/common/imx_sip_svc.c`
  to `imx_src_handler` in `plat/imx/imx95/imx95_m7.c`.
- START = SCMI CPU `CPU_RESET_VECTOR_SET` (boot flag) then `CPU_START` on the
  hard-coded `IMX9_SCMI_CPU_M7P = 1`. STOP = `CPU_STOP`, then base-protocol
  `RESET_AGENT_CONFIG` on the agent named `"M7"`. STARTED = `CPU_INFO_GET`:
  returns 1 when `runMode` is `START` (0) or `SLEEP` (3), else 0; the return
  value of `CPU_INFO_GET` is ignored in that branch.
- The calls go through the single EL3 handle `imx95_scmi_handle` (secure-agent
  mailbox), so the SM checks the **TF-A agent's** `cpuPerms[CPU_M7P]`, not the
  Linux agent's.
- A non-zero SCMI status is returned unchanged in `x0` with no print.
  Unknown sub-op returns `SMC_UNK`. No SIP call returns SM cfg, build or board
  data; `IMX_SIP_GET_SOC_INFO` (0xC2000006) returns ELE SoC data and
  `IMX_SIP_BUILDINFO` (0xC2000003) returns the TF-A commit hash. TF-A prints
  "SCMI AP core protocol version 0x%x detected" only at INFO level.

## 4. The SM side: what is permission-gated (imx-sm@af1e3702)

Read from source, `sm/rpc/scmi/rpc_scmi_{misc,lmm,cpu}.c`; the parts marked
(summary) rest on the tool's summary.

| Protocol / message | Permission demanded of the caller | Readable by any agent? |
|---|---|---|
| CPU `CPU_START` 0x4, `CPU_STOP` 0x5, `CPU_RESET_VECTOR_SET` 0x6 | `cpuPerms[cpuId] >= EXCLUSIVE`, else `SM_ERR_DENIED` (verbatim for START) | no |
| CPU `CPU_ATTRIBUTES` 0x3, `CPU_INFO_GET` 0xC (run mode, sleep mode, reset vector) | none | yes |
| LMM `LMM_ATTRIBUTES` 0x3 | `lmmPerms[lmId] >= GET`, only for a target other than the caller's own LM | partly |
| LMM `LMM_BOOT` 0x4, `RESET` 0x5, `SHUTDOWN` 0x6 | `lmmPerms[lmId] >= PRIV` | no |
| MISC `DISCOVER_BUILD_INFO` 0x6, `CFG_INFO` 0xC (mSel + 16-char cfg basename), `BOARD_INFO` 0xE, `SI_INFO` 0xB, `RESET_REASON` 0xA, `SYSLOG` 0xD | none (summary) | yes (summary) |
| MISC `CONTROL_SET` 0x3 / `ACTION` 0x5 | `ctrlPerms[ctrl] >= EXCLUSIVE` | no |
| MISC `CONTROL_GET` 0x4 | `>= GET` | no |
| Base (0x10) `PROTOCOL_VERSION`, discover vendor / sub-vendor / impl version | none | yes |

Protocol versions: MISC `0x10001`, LMM `0x10001`, CPU `0x10000`.
The numeric values of the `SM_SCMI_PERM_*` levels and every board's actual
`lmmPerms` / `cpuPerms` / `ctrlPerms` come from the cfg file and were not read
here; they are not exposed by any message above (inferred).

## 5. Paths a read-only user could read

"Settles (c)?" means: would the content, on its own, show that the SM grants
`CPU_M7P` start/stop to the agent TF-A uses. The answer is **no for every
row**; the rows differ in how much they narrow it.

| # | Path or string | What it would show | Settles (c)? | Tag |
|---|---|---|---|---|
| 1 | dmesg `NXP SM MISC Version %d.%d` (`imx-sm-misc.c`, `scmi_imx_misc_protocol_init`) | The MISC protocol version the SM reports (0x10001 in imx-sm@af1e3702). Says nothing about cfg or rights | No | read from source (linux-imx@5a0a5e71) |
| 2 | dmesg `i.MX MISC NUM DEV CTRL: %d, NUM BRD CTRL: %d,NUM Reason: %d` (`scmi_imx_misc_attributes_get`) | Counts of device controls, board controls and reset reasons: a coarse build fingerprint | No | read from source (linux-imx@5a0a5e71) |
| 3 | dmesg `NXP SM BBM Version %d.%d` (`imx-sm-bbm.c`) | BBM protocol version | No | read from source (linux-imx@5a0a5e71) |
| 4 | `/sys/bus/scmi_protocol/devices/` (node names of the SCMI devices that bound, e.g. `scmi_dev.*`) and the `protocol_version`, `firmware_version` (`0x%x` of `impl_ver`), `vendor_id`, `sub_vendor_id` sysfs attributes the SCMI core attaches to those devices (`driver.c`, `DEVICE_ATTR_RO` via `dev_groups`, summary) | Which protocols Linux bound (expect 0x11..0x19, 0x81, 0x84; **no 0x80/0x82**, section 2) and the SM's vendor strings and implementation version | No: identifies the SM build; carries no permission data | read from source (summary) for the attribute names; the exact device-path layout is inferred |
| 5 | dmesg SCMI base-protocol line (mainline `base.c` prints protocol version, vendor `'%s:%s'` and firmware version in one `dev_info`) | Same data as row 4, in the log | No | **inferred**: `base.c` was not read at the pin; feasibility doc section 2 already records an observed SM version (2026-11-27), which is consistent with such a line |
| 6 | `/sys/kernel/debug/scmi/<instance>/instance_name`, `atomic_threshold_us`, `transport/{type,is_atomic,max_rx_timeout_ms,max_msg_size,tx_max_msg,rx_max_msg}` (`driver.c` `scmi_debugfs_common_setup`, summary); further `raw/` files exist only with `CONFIG_ARM_SCMI_RAW_MODE_SUPPORT` (`raw_mode.c`, not read) | Transport parameters only (mailbox/shmem), nothing about the SM configuration. The debugfs tree needs root and `CONFIG_ARM_SCMI_DEBUG`-style options (unverified on the Toradex build) | No | read from source (summary); raw-mode presence on the board is inferred |
| 7 | `/sys/class/remoteproc/remoteproc*/state` | `offline` when `imx_rproc_detect_mode` saw `IMX_SIP_RPROC_STARTED` return 0, `detached` when it returned non-zero (started by an earlier boot stage) | No: `offline` is also what a TF-A call that failed (the `scmi_core_info_get` result is ignored) or a core stopped by SM would give; it shows no grant | read from source (`imx_rproc_detect_mode`, `imx_rproc_attach`); the mapping to what the board shows is inferred |
| 8 | `/sys/class/remoteproc/remoteproc*/{name,firmware}` and `/proc/device-tree/.../fsl,imx95-cm7` node (`mbox-names`, `memory-region`, `fsl,auto-boot`, `power-domains`) | That Linux was told it owns an M7 and the firmware name; no rights | No | read from source (`imx_rproc.c`); node presence in the Toradex DT is not read here (see `docs/imx95-verdin-bsp-src.md`) |
| 9 | dmesg `imx-rproc ...: No resource table in elf`, `Not in wfi, force stopped`, `Failed to enable remote core!`, `Failed to stop remote core` | Outcomes of an actual start/stop only (the last two are the symptom of a refused start/stop) | **Only by attempting a start or stop**; that is a write, outside a read-only session. After an attempt, `Failed to enable remote core!` would show a refusal, but a missing string does not prove a grant for stop | read from source (`imx_rproc_start`/`stop`) |
| 10 | dmesg TF-A strings (`SCMI AP core protocol version 0x%x detected`, `Unimplemented i.MX SiP Service Call`) | Not visible to Linux: BL31 output goes to the console UART at its own log level, not into the kernel ring buffer | No | read from source (imx-atf@8ec7e38); the UART-only visibility is inferred |
| 11 | A cfg name / `MISC_CFG_INFO` (mSel, `cfgName`) or `MISC_DISCOVER_BUILD_INFO` (commit, date) | The most useful SM data: which cfg built the SM (e.g. `mx95evk` versus a Toradex cfg) and when. **No kernel code sends these messages** at the pin and no sysfs, debugfs or dmesg string carries them | Would narrow (c) (an unmodified cfg name implies the stock permissions of `docs/imx95-sm-display-lm.md`), but is **not reachable read-only** | read from source (absence in linux-imx@5a0a5e71; message layout in imx-sm@af1e3702) |
| 12 | SM log (`MISC_SYSLOG`) / SM boot banner | SM-side reset reasons and logs | No | read from source (message exists; no Linux consumer). The SM banner goes to the SM's own UART (inferred) |

## 6. What this means for gate check (c)

- Inferred: **no read-only path in the NXP 6.6 kernel settles (c).** The
  permission that matters (`cpuPerms[CPU_M7P] >= EXCLUSIVE` for TF-A's agent)
  is held in the SM cfg, no message returns it, and the kernel does not ask
  for even the cfg name.
- What read-only data can add: the SM version and vendor strings (rows 1-5)
  and the remoteproc `state` (row 7). With an SM cfg identified out of band
  (feasibility doc / `docs/imx95-sm-display-lm.md` read the stock
  `mx95evk` assignment), they support an inference, not evidence.
- Only two things would decide (c) on the board: a start attempt of a harmless
  image (row 9, a write, a planner decision per the feasibility doc), or
  sending `MISC_CFG_INFO` / `CPU_INFO_GET` over SCMI raw mode (debugfs, needs a
  kernel with raw mode and is a write path). Raw mode would speak as the
  **Linux** agent, so it still would not give TF-A's permissions; `CPU_INFO_GET`
  and `CPU_ATTRIBUTES` need no permission and `MISC_CFG_INFO` needs none
  (summary), so those reads would work even without grants and would not prove
  the grant.
- The best source-level route to the SM cfg is outside the board: the
  Toradex SM cfg file (see `docs/imx95-verdin-bsp-src.md`, item `sm-config-read`).

## 7. Limits and what to re-check

- The kernel on the board is Toradex's 6.6.157-7.8.0. NXP may have added
  LMM/CPU vendor protocols and `imx_rproc` SCMI paths after 6.6.52; NXP's
  later trees (6.12) have them, but that was not read here. A read-only
  `ls /sys/bus/scmi_protocol/devices` plus the loaded drivers would show it
  directly (row 4).
- `driver.c`, `imx95.dtsi` and the imx-sm `rpc_scmi_*.c` files were only partly
  verbatim; rows marked (summary) should be re-read from a checkout.
- The imx-atf tag commit was taken from a web page, not `git ls-remote`.
