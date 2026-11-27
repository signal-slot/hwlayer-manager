# vc4 HVS5 / HVS6: format and modifier acceptance (SAND64/128/256, T-tiled), read from rpi-6.18.y

Dated 2026-11-28. Settles from source the conflict between the `IN_FORMATS` blob recorded on the Pi 5 and the driver code, and gives the per-generation table that `docs/hvs-backend-design.md` §4 and §5 sketch. It extends `docs/vc4-hvs-scaling-source.md` §4 with the modifier numbering, the per-generation format counts, the column-height and pitch limits, and a count check of the recorded blobs. Nothing here was run on a board.

## 1. Provenance

- Tree: `raspberrypi/linux`, branch `rpi-6.18.y`, commit `16ae2649dcfadf9d81445b1177a3191efa3cb82e`, directory `drivers/gpu/drm/vc4/`.
  Link: <https://github.com/raspberrypi/linux/tree/16ae2649dcfadf9d81445b1177a3191efa3cb82e/drivers/gpu/drm/vc4>
- Files read in full from the dependency cache copy of the raw files at that commit: `vc4_plane.c` (3004 lines), `vc4_regs.h`, `vc4_drv.h`. Every `file:line` below is a line of that copy. The copy was checked by its line count (3004) against the pin recorded in `docs/vc4-hvs-scaling-source.md`; no SHA is stored with the copy. read from source
- DRM core (`drm_plane.c`) is **not** part of the rpi pin fetch. It was read from the mainline Linux 6.18 copy in the dependency cache (`drm_plane.c:243-257`, `drm_plane.c:916-940`). Statements about the core are therefore tagged inferred for the rpi tree. inferred
- `drm_fourcc.h` was read from the libdrm copy in the dependency cache, not from the kernel tree. The Broadcom modifier numbers are the uapi values and do not change between trees. read from source
- Names: gen 5 = `VC4_GEN_5` = BCM2711 = HVS5. Gen 6 = `VC4_GEN_6_C` and `VC4_GEN_6_D` = BCM2712 = HVS6 (`vc4_drv.h:85-88`, in that order, so `gen >= VC4_GEN_6_C` means both gen 6 variants). Gen 4 (BCM2835 to 2837) is shown only where a rule differs. read from source
- Board data compared: `crates/composition-drm/tests/data/bcm2712/` (kernel `6.18.50+rpt-rpi-2712`, 2026-11-27) and `crates/composition-drm/tests/data/bcm2711/` (kernel `6.12.47+rpt-rpi-v8`, 2026-11-27), plus `docs/observed/bcm2712-topology.md` (2026-11-28). The recorded boards run Debian-style `+rpt` kernels, which are not the pin. observed 2026-11-27

## 2. Modifier numbers

The `IN_FORMATS` blob and the dumps carry the Broadcom modifiers as numbers. The numbering (read from source, `drm_fourcc.h`: `fourcc_mod_code(BROADCOM, 1)` for T-tiled, `fourcc_mod_broadcom_code(2..5, v)` for the SAND family, parameters from bit 8 up):

| Number | Name | Note |
| --- | --- | --- |
| `0x0700000000000001` | `DRM_FORMAT_MOD_BROADCOM_VC4_T_TILED` | no parameter |
| `0x0700000000000002` | SAND32 | never advertised by vc4 (not in the `modifiers[]` array below) |
| `0x0700000000000003` | `DRM_FORMAT_MOD_BROADCOM_SAND64` | `COL_HEIGHT(v)` = `3 \| v << 8` |
| `0x0700000000000004` | `DRM_FORMAT_MOD_BROADCOM_SAND128` | `COL_HEIGHT(v)` = `4 \| v << 8` |
| `0x0700000000000005` | `DRM_FORMAT_MOD_BROADCOM_SAND256` | `COL_HEIGHT(v)` = `5 \| v << 8` |

`fourcc_mod_broadcom_param(m)` is `(m >> 8) & (2^48 - 1)` and `fourcc_mod_broadcom_mod(m)` clears those bits, so the driver compares the base number and reads `v` as the column height in lines. read from source

**Correction to an earlier reading.** `docs/observed/bcm2712-topology.md` reads code 3 as SAND128 and guesses "SAND30" for code 4. By the numbering above, code 3 is SAND64, code 4 is SAND128 and code 5 is SAND256. The Pi 5 blob therefore lists **SAND64** (3) on NV12/NV21 and **SAND128** (4) on NV12/NV21/P030. The file states that its decoding is from memory (inferred), and `docs/hvs-backend-design.md` §5 inherits it ("codes 1, 3, 4 and 5 ... T-tiled, SAND128 and others"). Both files are read-only for this item; `crates/composition-drm/tests/data/bcm2711/README.md` already decodes the numbers correctly. read from source

## 3. What the driver advertises and accepts

### 3.1 Advertised arrays

| Item | Finding | Basis | Reference |
| --- | --- | --- | --- |
| Modifier array | `T_TILED`, `SAND128_COL_HEIGHT(0)`, `SAND64_COL_HEIGHT(0)`, `SAND256_COL_HEIGHT(0)`, `LINEAR`, `INVALID`. One array for all generations and all planes. This is the order of the blob's modifier rows on both boards (1, 4, 3, 5, 0) | read from source; board order observed 2026-11-27 | `vc4_plane.c:2858-2865`, `vc4_plane.c:2877-2881` |
| Format array | `hvs_formats[]` minus entries flagged for a newer generation. The test is `hvs5_only && gen < VC4_GEN_5` and `hvs6_only && gen < VC4_GEN_6_C`. So `hvs5_only` really means "not gen 4" and **stays on gen 6**: P030 and the four `*2101010` formats are on both BCM2711 and BCM2712. Only the `hvs6_only` entries (P010, P012, P016, S010, S012, S016) are gen 6 only | read from source | `vc4_plane.c:193`, `vc4_plane.c:199`, `vc4_plane.c:205`, `vc4_plane.c:211`, `vc4_plane.c:217`, `vc4_plane.c:283-313`, `vc4_plane.c:2867-2875` |
| Count of formats at the pin | 45 entries in the table; 5 are `hvs5_only`, 6 are `hvs6_only`. Gen 4: 34, gen 5: 39, gen 6: 45 (counted from the table text) | read from source (table), inferred (counts) | `vc4_plane.c:41-315` |
| Hook | `.format_mod_supported = vc4_format_mod_supported` in the plane funcs | read from source | `vc4_plane.c:2845` |
| Blob builder | For each advertised modifier and each format the core sets the format bit when the hook returns true (the blob keeps no pair the hook refuses). The same hook is used by `drm_plane_has_format()` | read from source in mainline 6.18 `drm_plane.c`; rpi tree inferred | `drm_plane.c:243-257`, `drm_plane.c:928-930` (mainline copy) |

### 3.2 `vc4_format_mod_supported` (`vc4_plane.c:2761-2837`)

Rules, in the order the function applies them. All read from source.

| Order | Rule | Reference |
| --- | --- | --- |
| 1 | Gen >= 6C and (`modifier == T_TILED` or base modifier == SAND64): **false for every format**. Comment in the code: "vc6_plane_mode_set() only handles linear, SAND128 and SAND256" | `vc4_plane.c:2767-2771` |
| 2 | XRGB8888, ARGB8888, ABGR8888, XBGR8888, RGB565, BGR565, ARGB1555, XRGB1555: base modifier LINEAR or T_TILED, else false | `vc4_plane.c:2773-2789` |
| 3 | NV12, NV21: base modifier LINEAR, SAND64, SAND128 or SAND256 | `vc4_plane.c:2790-2800` |
| 4 | P030: base modifier SAND128 only. LINEAR is **false** for P030 | `vc4_plane.c:2801-2807` |
| 5 | Everything else (the case list names `RGBX1010102` and `BGRX1010102` families, but the advertised 10-bit formats are `XRGB2101010` and `ABGR2101010` families; they reach `default:`; also 4444, 332, 3-plane YUV, NV16/NV61, P010/P012/P016, S010/S012/S016): `modifier == DRM_FORMAT_MOD_LINEAR` | `vc4_plane.c:2808-2835` |

The SAND comparisons use `fourcc_mod_broadcom_mod()`, so every `SAND*_COL_HEIGHT(n)` passes for its group. T-tiled in rule 1 and rule 2 differs: rule 1 compares the full 64-bit value, rule 2 the base. Since T-tiled takes no parameter this makes no difference. read from source

### 3.3 Result per generation

Derived from §3.1 and §3.2. inferred

| Format group | Gen 5 (BCM2711, HVS5) | Gen 6 (BCM2712, HVS6) |
| --- | --- | --- |
| XR24, AR24, AB24, XB24, RG16, BG16, AR15, XR15 | LINEAR, T_TILED | LINEAR |
| NV12, NV21 | LINEAR, SAND64, SAND128, SAND256 | LINEAR, SAND128, SAND256 |
| P030 | SAND128 only (no LINEAR) | SAND128 only (no LINEAR) |
| XR30, AR30, AB30, XB30 (`*2101010`) | LINEAR | LINEAR |
| RX24, RA24, BX24, BA24, RGB888, BGR888, 4444, 332, YU12/YV12, YU16/YV16, YU24/YV24, NV16, NV61 | LINEAR | LINEAR |
| P010, P012, P016, S010, S012, S016 | not advertised | LINEAR |
| SAND256 on P030, SAND64 on P030 | refused (rule 4) | refused (rule 4) |
| SAND on any RGB format, T_TILED on any YUV format | refused | refused |

Exclusions that differ per generation (all of them): T_TILED and SAND64 are gen 5 (and gen 4) only; the six `hvs6_only` formats are gen 6 only. No other pair differs. inferred from `vc4_plane.c:2767-2771`, `vc4_plane.c:283-313`

## 4. What `mode_set` accepts after the hook says yes

The hook is the first gate; `vc4_plane_mode_set` (gen 4 and 5, used through `vc5_plane_helper_funcs`, `vc4_plane.c:2886-2887`) and `vc6_plane_mode_set` (gen 6, `vc4_plane.c:2020`) are the second. A pair the hook accepts can still fail `TEST_ONLY`.

### 4.1 Modifier switch

| Generation | Accepted by the switch | Refusal | Reference |
| --- | --- | --- | --- |
| Gen 5 | LINEAR, T_TILED, SAND64, SAND128, SAND256. T_TILED forces `vdownsample = 1` (no line-skip decimation), uses tiling code 3 (`SCALER_CTL0_TILING_256B_OR_T`, the same code SAND256 uses). SAND64 tiling 1, SAND128 tiling 2 | `default:` returns `-EINVAL` | `vc4_plane.c:1539`, `vc4_plane.c:1554-1556`, `vc4_plane.c:1602`, `vc4_plane.c:1624-1659`, `vc4_plane.c:1717-1721`, `vc4_regs.h:1143-1148` |
| Gen 6 | LINEAR, SAND128 (`SCALER6_CTL0_ADDR_MODE_128B` = 1), SAND256 (`..._256B` = 2). The mode field also defines MAP8 and UIF, which vc4 does not use. No case for T_TILED or SAND64 | `default:` returns `-EINVAL` | `vc4_plane.c:2118-2164`, `vc4_plane.c:2236-2240`, `vc4_regs.h:1386-1391` |

So on gen 6 the T_TILED and SAND64 refusal is in two places: the hook (blob and `drm_plane_has_format`) and the `mode_set` switch. A client that skipped the hook would still get `-EINVAL` from the switch. read from source, the client case inferred

### 4.2 SAND parameters and per-format details

| Item | Gen 5 | Gen 6 | Reference |
| --- | --- | --- | --- |
| Column height `param` (the `v` of `COL_HEIGHT(v)`) | Refused with `-EINVAL` when `param > SCALER_TILE_HEIGHT_MASK` = 0xFFFF (15:0) | Same test with the same gen-5 constant; the register field `SCALER6_PTR2_TILE_HEIGHT` is 16:0 | `vc4_plane.c:1627-1634`, `vc4_plane.c:2134-2144`, `vc4_regs.h:1364`, `vc4_regs.h:1443` |
| `param == 0` | Pitch word = `fb->pitches[i]` (the column pitch is the framebuffer pitch) | Same, packed into the tile-height field | `vc4_plane.c:1674-1677`, `vc4_plane.c:2185-2188`, `vc4_plane.c:2226` |
| `param != 0` | Pitch word = `param` in the 15:0 tile-height field | Same | `vc4_plane.c:1678-1680`, `vc4_plane.c:2189-2191` |
| Column width in bytes | SAND64: 64, SAND128: 128, SAND256: 256; P030: 128 regardless (96 pixels) | SAND128: 128, SAND256: 256; P030: 128 | `vc4_plane.c:1639-1655`, `vc4_plane.c:2149-2161` |
| Hardware pixel format | non-P030 SAND: `HVS_PIXEL_FORMAT_H264` (12); P030: `YCBCR_10BIT` | non-P030 SAND: `YCBCR_YUV420_2PLANE`; P030: `YCBCR_10BIT` | `vc4_plane.c:1636-1641`, `vc4_regs.h:1099`, `vc4_plane.c:2146-2151` |
| Vertical line skip | `max_tile_skip` = `FIELD_MAX(SCALER_TILE_SKIP_0_MASK) + 1` = 8 (bits 18:16) | `FIELD_MAX(SCALER6_PTR2_TILE_LSKIP_MASK) + 1` = 8 (bits 31:29) | `vc4_plane.c:1504-1505`, `vc4_regs.h:1362`, `vc4_plane.c:2087-2088`, `vc4_regs.h:1441` |
| Gen 6 fetch count | n/a | `ceil((width + src_x % cpw) / cpw)` with `cpw` = 32 components per word (24 for P030), stored in `SCALER6_PTR2_TILE_FETCH_COUNT` (26:16) | `vc4_plane.c:2167-2170`, `vc4_plane.c:2227-2228`, `vc4_regs.h:1442` |
| Source width | refused above `SCALER5_POS2_WIDTH_MASK` = 8191 (12:0), after the doubling for YUV444 | no check; the field `SCALER6_POS2_SRC_WIDTH` is 12:0, so a larger width is truncated by `VC4_SET_FIELD` after a `WARN_ON` | `vc4_plane.c:1726-1731`, `vc4_regs.h:1271`, `vc4_plane.c:2293-2295`, `vc4_regs.h:1418`, `vc4_regs.h:14-18` (as read in the earlier scaling doc) |

The 0xFFFF column-height limit is the only numeric limit on the SAND parameter. Neither the hook nor `mode_set` checks that the column height is a multiple of anything or that `fb->pitches` matches the column width times `cpp`. The `drm_fourcc.h` text says the stride should be `width * cpp` and the height travels in the modifier. read from source (both)

On gen 6 the register fields `SCALER6_PTR2_TILE_HEIGHT` (16:0) and `SCALER6_PTR2_TILE_FETCH_COUNT` (26:16) overlap in bit 16 as the header writes them. The driver ORs them into one word. A tile height above 0xFFFF is refused earlier, so bit 16 of the height is always 0 unless `param == 0` and `fb->pitches` is at least 65536. That corner is not exercised by anything recorded. read from source (masks), inferred (the corner)

### 4.3 T-tiled geometry (gen 5 only)

| Item | Finding | Reference |
| --- | --- | --- |
| Tile size | 4 KiB tiles. 32 pixels wide at 4 bytes per pixel, 64 wide at 2 bytes per pixel (`cpp[0] == 2`), 32 high | `vc4_plane.c:1558-1561` |
| Pitch meaning | `tiles_w = fb->pitches[0] >> 7` (4096 / 32 rows): `pitch * tile_h == tile_size * tiles_per_row` | `vc4_plane.c:1567-1572` |
| Left and right tile counts | `tiles_l` and `tiles_w - tiles_l` go into 7-bit fields (`TILE_WIDTH_L` 22:16, `TILE_WIDTH_R` 6:0). A value above 127 only warns in `VC4_SET_FIELD`. That caps a usable T-tiled width at about 127 tiles on each side of the source origin, i.e. 4064 pixels at 4 bytes and 8128 at 2 bytes | `vc4_plane.c:1573-1574`, `vc4_plane.c:1603-1606`, `vc4_regs.h:1372`, `vc4_regs.h:1379`; cap inferred |
| Reflection | `REFLECT_Y` sets `TILE_LINE_DIR` and mirrors the y offset; `REFLECT_X` goes through the control word flag like the other layouts | `vc4_plane.c:1595-1600` |
| Vertical decimation | none (`vdownsample = 1`) | `vc4_plane.c:1556` |

## 5. Comparison with the recorded blobs

### 5.1 BCM2711 (CM4), kernel 6.12.47+rpt-rpi-v8, 2026-11-27

Recorded blob (`crates/composition-drm/tests/data/bcm2711/props-2026-11-27.txt`, the five-modifier block on every plane; also described in `.../bcm2711/README.md`): 35 formats, 5 modifiers. observed 2026-11-27

| Modifier | Recorded formats | Pin, gen 5 (§3.3) | Match |
| --- | --- | --- | --- |
| LINEAR | 34 formats: XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12 XB12 AB12 BX12 BA12 RX12 RA12 | the same 34 plus RX24 RA24 BX24 BA24 (38 linear; P030 has no linear) | the board kernel has four fewer linear formats; every recorded one is in the pin's list |
| T_TILED (1) | XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 | the same eight | yes |
| SAND64 (3) | NV12 NV21 | NV12 NV21 | yes |
| SAND128 (4) | NV12 NV21 P030 | NV12 NV21 P030 | yes |
| SAND256 (5) | NV12 NV21 | NV12 NV21 | yes |

Format count: pin gen 5 = 39 (§3.1); recorded 35 = 39 − 4 (RX24, RA24, BX24, BA24 not advertised). The older kernel differs only by those four linear formats. inferred; the cause (an older table in the 6.12 tree) is also inferred

**Verdict BCM2711: no conflict.** The modifier rows of the blob equal the source's gen-5 rules row by row, so the blob and the source agree on SAND64, SAND128, SAND256 and T-tiled. inferred (comparison), tags above

### 5.2 BCM2712 (Pi 5), kernel 6.18.50+rpt-rpi-2712, 2026-11-27

Recorded blob (`.../bcm2712/props-2026-11-27.txt`, identical on the five planes read): 45 formats, 5 modifiers. observed 2026-11-27

| Modifier | Recorded formats | Pin, gen 6 (§3.3) | Match |
| --- | --- | --- | --- |
| LINEAR | 44 formats (no P030), including RX24 RA24 BX24 BA24 and P010 P012 P016 S010 S012 S016 | 44 (45 table entries minus P030, which has no LINEAR) | yes |
| T_TILED (1) | XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 | **none** (rule 1) | **no** |
| SAND64 (3) | NV12 NV21 | **none** (rule 1) | **no** |
| SAND128 (4) | NV12 NV21 P030 | NV12 NV21 P030 | yes |
| SAND256 (5) | NV12 NV21 | NV12 NV21 | yes |

Count check: the pin gen-6 table has 45 entries (section 3.1) and P030 has no LINEAR pair (rule 4), so the pin linear row has 44 formats. The recorded linear row has 44 names (counted from the dump) and the blob header says 45 formats, which is the 44 linear plus P030. inferred (count), observed 2026-11-27 (names)

The format list therefore matches the pin exactly (45 formats, same set); the modifier set does **not**: the board lists T_TILED and SAND64, which the pin's hook refuses on gen 6.

The blob is internally coherent with the pin in other ways: the modifier row order (1, 4, 3, 5, 0) is the order of the `modifiers[]` array (§3.1), P030 appears only under SAND128 and not under LINEAR (rule 4), SAND appears only on NV12/NV21/P030 and T_TILED only on the eight RGB formats (rules 2-4). The only difference is that rule 1 is absent. inferred

### 5.3 Why the Pi 5 blob lists the two modifiers

Candidate explanations, in the order of how well the source supports them. All inferred:

1. **The board kernel's `vc4_plane.c` has no rule 1.** The rule is a comment-annotated early return that is easy to add later. The board kernel is `6.18.50+rpt`, the same series as the pin but a distribution build from an unknown commit; the pin is the branch tip of 2026-11-26 (`docs/vc4-hvs-load-source.md`). Whether the 6.18.50 build predates rule 1 is not known from here. This explanation needs one fact: the commit that added `vc4_plane.c:2767-2771`, which was not read.
2. **The blob builder ignores the hook for these pairs.** The mainline builder uses the hook for every pair (`drm_plane.c:245-248`), and a blob that ignored it would also list SAND on RGB formats and T_TILED on NV12, which the recorded blob does not. Fits worse.
3. **A per-plane difference.** The hook reads only `vc4->gen`, not the plane, and the five planes read show the same blob. Excluded.

What the hardware can do (inferred from §4.1): the gen-6 address-mode field has LINEAR, 128B, 256B, MAP8 and UIF, with no 64B mode and no T mode (`vc4_regs.h:1386-1391`), and the gen-6 `mode_set` switch has no case for either. If the board kernel's `mode_set` matches the pin's, a T_TILED or SAND64 framebuffer fails atomic check on the Pi 5 although the blob lists it.

## 6. Verdict: blob against source, and which a profile should trust

| Question | Answer | Tag |
| --- | --- | --- |
| Do the blob and the source conflict on BCM2711? | No. Same modifier sets (T_TILED on 8 RGB formats; SAND64/128/256 on NV12/NV21; SAND128 on P030). The older 6.12 kernel lacks four linear RGB formats only | inferred |
| Do they conflict on BCM2712? | Yes, on two modifiers: the blob lists T_TILED (8 RGB formats) and SAND64 (NV12/NV21); the pin's hook refuses both on gen 6. Everything else matches (45 formats, SAND128 and SAND256 sets, P030 under SAND128 only, modifier order) | inferred, from observed 2026-11-27 and read from source |
| Which should a profile trust before `TEST_ONLY` has run? | **The source, for gen 6.** `Vc4Hvs6` should offer LINEAR, SAND128 and SAND256 only (SAND128 and SAND256 on NV12/NV21, SAND128 on P030), and drop T_TILED and SAND64 even when the blob lists them | inferred |
| Why the source and not the blob? | Two independent places in the pin refuse the pairs (hook, then `mode_set` switch), and the register map has no address mode for either. A wrongly dropped pair costs an optimisation; a wrongly kept pair makes a commit fail. The blob is the better source for everything else it says on that board (it was observed), and it matched the pin on the format list | inferred |
| Which for gen 5? | Both, they agree. The profile may use the blob intersected with the source rule, which equals either alone | inferred |
| What settles it? | The `rpi5-sweep` and `cm4-sweep` `TEST_ONLY` runs: a T_TILED XRGB and a SAND64 NV12 layer on the Pi 5 (expected: refused), and each of T_TILED, SAND64, SAND128 and SAND256 on the CM4 (expected: accepted, subject to §4). A Pi 5 acceptance would show that the board kernel differs from the pin and the profile should then follow the blob and record the kernel version next to it | inferred |

Rule for a profile that reads the blob at run time (inferred): keep a (format, modifier) pair for gen 6 only if the blob lists it **and** the gen-6 table of §3.3 allows it; for gen 5 keep a pair if the blob lists it. The intersection keeps the source's refusal and the blob's per-kernel format list.

Two facts a profile needs that the blob does not carry (read from source): the SAND column height lives in the modifier (`COL_HEIGHT(v)`, v up to 0xFFFF), and the blob lists only the `COL_HEIGHT(0)` form, so a profile that matches modifiers by equality must compare `modifier & ~(param mask)` (`fourcc_mod_broadcom_mod`) to find the base. `vc4_plane.c:2858-2862`, `drm_fourcc.h` (libdrm copy)

## 7. Not read, left open

- The commit that added the gen-6 early reject (rule 1), to say which `+rpt` kernels carry it. Not read; the data above does not need it.
- The DRM core of the rpi tree (only the mainline 6.18 core was read). Tagged inferred where it matters.
- The atomic-check callers of `drm_plane_has_format()` (not in the dependency cache). The refusal of an unsupported modifier at atomic check is therefore inferred, not read.
- Whether any plane differs in capability within a generation. `format_mod_supported` ignores the plane (`vc4_plane.c:2761-2765`), so none should. read from source
- `TEST_ONLY` behaviour: nothing here was measured.
