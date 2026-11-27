# Verdin iMX95: the System Manager configuration, pinned

Date of reads: 2026-11-28. Scope: SoW §5.1 / §9 M2 (WP3), the ownership
question that comes before any Zephyr display work on the M7. This is a
source read only. No board was accessed and nothing here is a hardware
result.

Tags: **read from source** (the file named, at the pin given, was fetched
and the line seen) and **inferred** (my deduction; no source states it).
**Inferred for Verdin** marks a row read from NXP's `mx95evk.cfg` and carried
over to the Verdin without the Verdin file to confirm it.

## 1. Result in one paragraph

Toradex does **not** build NXP's `mx95evk` config for the Verdin iMX95. Its
layer builds its own imx-sm fork, `imx-sm-toradex`, with
`SYSTEM_MANAGER_CONFIG = "verdin-imx95"` (read from source, §2). That
`configs/verdin-imx95.cfg` file **could not be read** from this node (§6):
the Toradex git host answers automated fetches with an anti-bot page, no
mirror of the fork exists on GitHub, and git/curl over the network was
refused here. The LM and resource tables (§3, §4) are therefore NXP's
`mx95evk.cfg` at the matching NXP release, and every conclusion about the
Verdin drawn from them is **inferred for Verdin**.

## 2. Sources and pins

| # | Source | Pin | What was read | Tag |
|---|---|---|---|---|
| P1 | NXP imx-sm, <https://github.com/nxp-imx/imx-sm> | tag `lf-6.6.52-2.2.2` = `rel_imx_sm_2025q3` = commit `af1e37026c6ba19cdca98fd6e91494efd3e7b5ec` | `configs/mx95evk.cfg` (1019 lines), `devices/MIMX95/configtool/displaymix.cfg`, `sm/doc/config.md`, `configs/` listing | read from source |
| P2 | NXP meta-imx, <https://github.com/nxp-imx/meta-imx> | branch `scarthgap-6.6.52-2.2.2` = commit `474d2b45015f73e6b0d77729e2028b00e008f730` | `meta-imx-bsp/recipes-bsp/imx-system-manager/imx-system-manager_2025q3.bb` (`SRCREV = "af1e37026c6ba19cdca98fd6e91494efd3e7b5ec"`, `SRCBRANCH = "master"`) and `imx-system-manager.inc` (`SYSTEM_MANAGER_CONFIG ?= "INVALID"`, `CONFIG=${SYSTEM_MANAGER_CONFIG}` then `oe_runmake cfg`) | read from source |
| P3 | Toradex `meta-toradex-nxp`, GitHub mirror <https://github.com/lmp-mirrors/meta-toradex-nxp> of `https://git.toradex.com/meta-toradex-nxp.git` | branch `scarthgap-7.x.y` = commit `33dc3310b4ee14c592da5896470dd713cc191a88` (mirror head on 2026-11-28) | `recipes-bsp/imx-system-manager/imx-sm-toradex_1.0.0.bb`, `conf/machine/verdin-imx95.conf`, the commit list of the recipe | read from source |
| P4 | Toradex imx-sm fork `imx-sm-toradex`, `https://git.toradex.com/imx-sm-toradex.git` | branch `main`, `SRCREV = 9e5555333eb27c75dc670945443a13dfc2760f16` (from P3) | **nothing**: `configs/verdin-imx95.cfg` not obtained (§6) | — |
| P5 | Upstream U-Boot, <https://github.com/u-boot/u-boot/blob/ece349ade2973e220f524ce59e59711cc919263f/doc/board/toradex/verdin-imx95.rst> | commit `ece349ade2973e220f524ce59e59711cc919263f` (master on 2026-11-28); file added by `60d8255d8dc031f2a961f1e062fc12c92573f186` | the "System Manager" build steps | read from source |
| P6 | Toradex developer page "Build U-Boot for NXP i.MX 95 based modules", <https://developer.toradex.com/linux-bsp/os-development/build-u-boot-and-linux-kernel-from-source-code/build-u-boot/build-u-boot-for-nxp-imx95-based-modules/> | page dated "Last updated on Aug 17, 2026", BSP 7.x.y, read 2026-11-28 | prebuilt artefact names | read from source |

Pin caveats:

- P3 is a mirror branch head, not the BSP 7.8.0 release tag. No `7.x` tag
  was found in the mirror's first tag page. The earlier read in
  [imx95-zephyr-feasibility.md](imx95-zephyr-feasibility.md) (its S8)
  resolved `scarthgap-7.x.y` on git.toradex.com to `b0b9f0fd9d3e`, which is
  not `33dc3310`, so the mirror and the origin differ. Whether `9e555533` is
  the SRCREV of the 7.8.0 image is **inferred**, not shown.
- The recipe history on the mirror branch (P3, newest first) has nine
  commits. The last is `f8b1d0ca30872f1d04f4f85f1b03112ffa34282e`, "Bump
  SRCREV", 2026-03-02. Commit `2657b56f87b15ad1a75c42480d58f471f6eef4cb`
  (2025-09-03) says "imx-sm-toradex: bump to version based on
  lf-6.6.52-2.2.1" (read from source). So the fork is based on NXP
  `lf-6.6.52-2.2.1` (`707569f402147029feb7f9b90811a6d6ea730bb6`) or later.
  The recipe's `LIC_FILES_CHKSUM` md5 `f2a70813bc08547f509361c08b718861` is
  the one NXP's 2.2.1 and 2.2.2 recipes use. P1 is the 2.2.2 release, and
  using it as the reference is **inferred**.
- The mirror's `bump-SRCREV-scarthgap-7.x.y` branch
  (`77c50b3ed8fb21897f05c7e19192c8f69ad4f2cc`) points the same recipe at an
  internal Toradex host with `SRCREV = 922f8affacd89c3d7912021297a871ae0dd70947`
  (read from source). That is a pending bump, not a release.

### 2.1 What Toradex builds (read from source)

| File (P3 unless noted):line | Content | Tag |
|---|---|---|
| `imx-sm-toradex_1.0.0.bb:14` | `IMX_SYSTEM_MANAGER_SRC = "git://git.toradex.com/imx-sm-toradex.git;protocol=https"` | read from source |
| `imx-sm-toradex_1.0.0.bb:15-16` | `SRCBRANCH = "main"`, `SRCREV = "9e5555333eb27c75dc670945443a13dfc2760f16"` | read from source |
| `imx-sm-toradex_1.0.0.bb:13` | `SRC_URI = "${IMX_SYSTEM_MANAGER_SRC};branch=${SRCBRANCH}"`: no patch and no `.cfg` added by the layer, so the config lives in the fork itself | read from source (the conclusion: inferred) |
| `imx-sm-toradex_1.0.0.bb:21, 23-24` | requires the NXP `imx-system-manager.inc`; `# Disable serial debug monitor`, `PACKAGECONFIG = "m0"` | read from source |
| `verdin-imx95.conf:83-84` | `PREFERRED_PROVIDER_virtual/imx-system-manager = "imx-sm-toradex"`, `SYSTEM_MANAGER_CONFIG = "verdin-imx95"` | read from source |
| `verdin-imx95.conf:92` | `SYSTEM_MANAGER_FIRMWARE_NAME ?= "m33_image-verdin-imx95"` | read from source |
| `verdin-imx95.conf:55` | `IMXBOOT_TARGETS = "flash_a55"` | read from source |
| `verdin-imx95.conf` (whole file) | no `M7`, `m7_image` or `IMX_EXTRA_FIRMWARE` line | read from source |
| `verdin-imx95.conf:66-70` | OEI from `imx-oei-toradex`, `OEI_BOARD ?= "toradex-verdin-imx95"` | read from source |
| P5 | `$ git clone -b main https://git.toradex.com/cgit/imx-sm-toradex.git/`, `$ make config=verdin-imx95 all`, `$ cp build/verdin-imx95/m33_image.bin $(srctree)` (U-Boot at `ece349ade2973e220f524ce59e59711cc919263f`) | read from source |
| P6 | the System Manager is offered as a prebuilt `m33_image-verdin-imx95.bin`, per board (`m33_image-aquila-imx95.bin`, `m33_image-tdx-smarc-imx95.bin`); the page names no source or `.cfg` | read from source |
| P1 `configs/` at `af1e3702` | `mx94alt`, `mx94evk`, `mx95alt`, `mx95evk`, `simu` (+ `other/`): no Verdin or Toradex file in NXP's tree | read from source |

`flash_a55` is the imx-mkimage target without `-m7` (recorded in
[imx95-zephyr-feasibility.md](imx95-zephyr-feasibility.md) §2 from
imx-mkimage `lf-6.6.52-2.2.2`). So the Toradex boot container carries no M7
image (inferred from the two reads).

## 3. Logical machines (P1 `configs/mx95evk.cfg`)

Line numbers: counted by hand on the raw file at `af1e3702` and checked
against GitHub's blob view (1019 lines). They agree with the tarball-based
numbers in [imx95-sm-display-lm.md](imx95-sm-display-lm.md).

| Line(s) | LM / agent | Content | Core | Tag |
|---|---|---|---|---|
| 106 (section 106-324) | LM0 | `LM0 name="SM", rpc=none, boot=1, did=2, safe=feenv` | M33 (`M33P OWNER`, l. 197) | read from source; inferred for Verdin |
| 330 (section 330-424) | LM1 | `LM1 name="M7", rpc=scmi, boot=2, skip=1, did=4, safe=seenv` | M7 (`M7P OWNER # CPUs must be first`, l. 386) | read from source; inferred for Verdin |
| 342, 349, 354 | LM1 boot list | `CPU_M7P start=2, stop=1` (also with `msel=1`, `msel=2`) | M7 | read from source; inferred for Verdin |
| 358 | LM1 agent | `SCMI_AGENT0 name="M7"` | M7 | read from source; inferred for Verdin |
| 430 (section 430-535) | LM2 | `LM2 name="AP", rpc=scmi, boot=3, skip=1, did=3, default` | A55 (`A55C0`..`A55C5 OWNER`, l. 512-517) | read from source; inferred for Verdin |
| 483 | LM2 agent | `SCMI_AGENT1 name="AP-S", secure` (TF-A) | A55, secure | read from source; inferred for Verdin |
| 552 (block 541-1018) | LM2 agent | `SCMI_AGENT2 name="AP-NS"` (Linux) | A55, non-secure | read from source; that the block belongs to LM2 (no new `LMn` line before it) is inferred |

Option meanings (P1 `sm/doc/config.md`, read from source): `boot` is
"Optional, boot order starting with 1, undefined/0 = do not boot"; `skip`
"if not 0, ignore error on boot if no image in boot container"; `default` is
"The deault LM for the debug monitor" (sic); a CPU line's `start`/`stop`
"Call config function on that resource when the LM boots / shuts down". An LM
not booted by the SM "can be booted later using LMM_Boot() or via SCMI using
SCMI_LmmBoot()".

## 4. Display resources and M7 rights (P1)

Permission words (P1 `mx95evk.cfg`, read from source): `ALL: api=all`
(l. 58), `NOTIFY: api=notify` (l. 54); in AP-NS `OWNER: perm=rw, api=all`
(l. 544) and `ACCESS: perm=rw, api=all, mdid=none` (l. 545); in LM1
`OWNER: perm=rw, api=all` (l. 334). `config.md`: SCMI permissions "are
accumulative … higher permissions imply the lower ones", and `ALL` "is
currently equivalent to" `EXCLUSIVE`.

| Resource | Line(s) in `mx95evk.cfg` | Holder | Tag |
|---|---|---|---|
| DPU: `DC OWNER, test`, `DC0`, `DC1` | 616, 617, 618 | AP-NS (Linux) | read from source; inferred for Verdin |
| DPU sub-blocks `DC_2DBLIT`, `DC_BLITINT`, `DC_CMDSEQ`, `DC_DISPENG`, `DC_DISPENG_INT`, `DC_FL0`, `DC_FL1`, `DC_INT_CTL`, `DC_PIXENGINE`, `DC_XPC`, `DC_YUV0`..`DC_YUV3` (all `OWNER`) | 619-632 | AP-NS | read from source; inferred for Verdin |
| DISPLAYMIX block control `BLK_CTRL_DISPLAYMIX OWNER` | 599 | AP-NS | read from source; inferred for Verdin |
| LDB/LVDS `LVDS OWNER` | 790 | AP-NS | read from source; inferred for Verdin |
| MIPI DSI `MIPI_DSI OWNER`, `MIPI_PHY OWNER` | 793, 794 | AP-NS | read from source; inferred for Verdin |
| Pixel clocks `CLOCK_DISP1PIX`, `CLOCK_DISP2PIX`, `CLOCK_DISP3PIX` (`ALL`) | 572-574 | AP-NS API | read from source; inferred for Verdin |
| LDB PLL `LDB_PLL ALL` | 586 | AP-NS API | read from source; inferred for Verdin |
| Video PLL `VIDEO_PLL1 OWNER` | 878 | AP-NS | read from source; inferred for Verdin |
| Display bus clocks `CLK_DISPAPB`, `CLK_DISPAXI`, `CLK_DISPOCRAM` (`ALL`); `GPV_DISPLAY OWNER` | 159-161; 227 | LM0 (SM) | read from source; inferred for Verdin |
| Power domain `PD_DISPLAY`, perf `PERF_DISP` | none in `mx95evk.cfg`; `displaymix.cfg` l. 39-66 attaches `PD_DISPLAY` to every display resource, and `DC` (l. 39) also `PD_CAMERA, PERF_DISP` | AP-NS, through the resources it owns | read from source (both files); "follows the resources" inferred |
| Display interrupt routing | none for the display irqsteer; the only `IRQSTEER` entry is `IRQSTEER_M7 OWNER` (389) | `IRQSTEER_M7`: LM1; display irqsteer: no line | read from source (the absence); that the display irqsteer is covered by the DC/DISPLAYMIX TRDC regions is inferred |
| Any display, pixel, LDB, DSI or PD entry in LM1 | none in 330-424 | — | read from source; inferred for Verdin |
| M7 memory: `M7MIX DATA 0x020380000-0x02047FFFF`, `M7MIX DATA 0x04A060000-0x04A09FFFF`, `DDR EXEC 0x080000000-0x089FFFFFF` | 414, 415, 416 | LM1 | read from source; inferred for Verdin |
| A55 non-secure DDR: `DDR EXEC 0x088000000-0x089FFFFFF` and `0x08E000000-0x87FFFFFFF` | 1017, 1018 | AP-NS | read from source; the overlap with LM1's window at 0x88000000-0x89FFFFFF is read from source, how the SM resolves it is not known |
| M7 boot at reset | LM1 `boot=2, skip=1` (330) | SM boots the M7 if the container has an M7 image, else ignores its absence | read from source; inferred for Verdin |
| LMM rights over the M7 LM | AP-NS `LMM_1 NOTIFY` (587); AP-S API (488-508) has no `LMM_` line | Linux may only be notified about LM1, not boot or stop it | read from source; inferred for Verdin |
| CPU protocol rights over `CPU_M7P` | no `CPU_M7P` line in the AP-S or AP-NS API blocks (the only `CPU_M7P` lines are LM1's own, 342-354) | neither A55 agent can start or stop the M7 core | read from source (the absence); inferred for Verdin |
| M7's own rights | `LMM_2 ALL` (379), `SYS ALL` (382) in the M7 agent; AP-NS has `SYS NOTIFY` (593) | the M7 may boot/reset the AP LM, not the other way round | read from source; inferred for Verdin |

## 5. Verdict

**Read this first: every point below about the Verdin is inferred for
Verdin.** It rests on NXP `mx95evk.cfg` at `af1e3702`, because Toradex's
`configs/verdin-imx95.cfg` (P4, fork `SRCREV 9e555533`) was not read. A
Toradex edit to the display or M7 sections would change it.

1. **M7 owns the DPU/LVDS route alongside Linux: no.** Every display
   resource (DC and all sub-blocks, LVDS, DSI, DISPLAYMIX block control,
   video PLL, pixel and LDB clocks, and with them `PD_DISPLAY`) is in the
   AP-NS agent, and LM1 has none (read from source for `mx95evk`; inferred
   for Verdin). The cfg gives each resource line to one LM. The display
   engine is one pipeline. The only shared mechanism `config.md` names is the
   runtime `EXCLUSIVE` permission ("Handing a display over from one LM to
   another"), with the clients made responsible for exclusivity. So
   side-by-side ownership is not expressible, and a timed handover would
   still need a cfg that grants the M7 agent those rights (inferred).
2. **M7 owns it instead of Linux: only with a different SM image.** It is
   possible in principle: a cfg moving those lines to LM1, as drafted in
   [imx95-sm-display-lm.md](imx95-sm-display-lm.md), plus a Linux DT without
   the display nodes. That needs a rebuilt `m33_image` and boot container.
   The board rules forbid writing the eMMC boot partitions, so it is **not
   possible on the shared farm board** (inferred).
3. **As shipped, on this board: not at all** (inferred for Verdin). M2's
   display scanout from the M7 is blocked by SM resource assignment. The
   runnable path is the reduced scope in
   [imx95-m7-reduced-scope.md](imx95-m7-reduced-scope.md).
4. **M7 started from Linux remoteproc: not expected to work under an
   `mx95evk`-like config** (inferred for Verdin). The 6.6 `imx_rproc` starts
   the M7 through TF-A's SiP call, which uses the SCMI CPU protocol on
   `CPU_M7P` as the AP-S agent ([imx95-zephyr-feasibility.md](imx95-zephyr-feasibility.md) §2).
   `mx95evk` grants AP-S no `CPU_M7P` right, and AP-NS has only
   `LMM_1 NOTIFY`, so the SM should refuse start and stop (inferred; the SM's
   permission check was not read). Two Toradex facts are read from source:
   `IMXBOOT_TARGETS = "flash_a55"`, and no M7 firmware is configured in the
   machine. So the M7 is not started from the container either (inferred).
   Whether `verdin-imx95.cfg` adds `CPU_M7P` or `LMM_1` rights for AP-S or
   AP-NS, as NXP's `configs/other/mx95evkrpmsg.cfg` does, is the one question
   that decides the remoteproc gate. It stays **open** until P4 is read.

## 6. Fetch record (2026-11-28)

| Attempt | Outcome |
|---|---|
| `git clone --depth 1 --branch rel_imx_sm_2026q3 https://github.com/nxp-imx/imx-sm.git` (tag commit `eec232878ab7dd460f5a8f3767d39dc7e9412efa`) into the deps dir | refused by this node's tool permissions before running |
| `git clone --depth 1 --branch main https://git.toradex.com/imx-sm-toradex.git` (wanted commit `9e5555333eb27c75dc670945443a13dfc2760f16`) into the deps dir | refused before running (no git or curl network access was granted on this node) |
| `git ls-remote`, `gh api`, `curl` to GitHub or git.toradex.com | `git ls-remote` (allowed on retry) fails on this node with "refusing to work with credential missing host field", before any network exchange (the node's git setup, not the server); `gh api` and general `curl` were refused |
| `curl` of `https://git.toradex.com/meta-toradex-nxp.git/info/refs?service=git-upload-pack` (status only) | HTTP 200: git's smart-HTTP endpoint answers plain clients, unlike the cgit pages, so a normal `git clone` from another node should work (inferred) |
| OE Layer Index (`layers.openembedded.org/layerindex/api/recipes/?filter=pn:imx-sm-toradex` and related) | HTTP 403 to the web fetcher |
| `codeload.github.com` tarball of imx-sm `lf-6.6.52-2.2.2` (archive download) | first refused; allowed later but not needed, since P1's files were read from `raw.githubusercontent.com` at the same commit |
| git.toradex.com `cgit/imx-sm-toradex.git/plain/configs/verdin-imx95.cfg?h=main`, `cgit/meta-toradex-nxp.git/...`, `imx-sm-toradex.git/info/refs` | HTTP 200 with an "Anubis" anti-bot reject page, no content |
| GitHub: `toradex/imx-sm`, `toradex/imx-sm-toradex`, `toradex/meta-toradex-nxp`, `lmp-mirrors/imx-sm-toradex` | HTTP 404 |
| GitHub repository search `imx-sm-toradex`; forks of `nxp-imx/imx-sm` | 0 results; no fork owned by Toradex or named Verdin |
| Software Heritage API for the fork origin and revision `9e555533` | first refused; on retry HTTP 200 with an "Anubis" anti-bot reject page, so whether the revision is archived is unknown |
| `raw.githubusercontent.com` / `api.github.com` for P1, P2, P3, P5 | HTTP 200; the rows above |

No download was kept, so no deps path holds these sources. Everything was
read over HTTPS from the pinned URLs.

## 7. To close the gap

- Obtain `configs/verdin-imx95.cfg` at fork commit
  `9e5555333eb27c75dc670945443a13dfc2760f16`, for example
  `git clone -b main https://git.toradex.com/imx-sm-toradex.git` and
  `git checkout 9e5555333eb27c75dc670945443a13dfc2760f16` on a node where git
  is allowed through. Then re-check §3 and §4 line by line, especially LM1's
  resources and any `CPU_M7P` / `LMM_1` line in the AP-S and AP-NS blocks.
- Pin the BSP 7.8.0 tag of `meta-toradex-nxp` on git.toradex.com, to confirm
  that `9e555533` is the SRCREV of the image the board runs.
