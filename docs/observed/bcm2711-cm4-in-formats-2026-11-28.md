# BCM2711 (reTerminal CM4, HVS5): raw IN_FORMATS per plane (design Q3)

Dated 2026-11-28. Answers `docs/design/hvs-backend.md` section 11 question 3 for the BCM2711: what
modifiers does the kernel offer on every plane, and does the numbering match `drm_fourcc.h`?

This is portability evidence for SoW 5.4 step 3 (document the SoC's topology before a backend is
built) and for `docs/open-issues.md` X-07. It counts toward no milestone (not M1, M2 or M3).
Q3 has a second half, the Pi 5 `TEST_ONLY` of T-tiled and SAND buffers; it is not part of this
file.

Tags: **observed 2026-11-28** is a measurement on the board, dated in host UTC (the session ran
2026-11-28 12:30 to 12:33 UTC). **read from source** names the file read. **inferred** is a
conclusion drawn here.

## Setup

| Item | Value | Tag |
|---|---|---|
| Board | Raspberry Pi Compute Module 4 Rev 1.0 in a reTerminal, BCM2711, HVS5 | observed 2026-11-28 |
| Kernel | `6.12.47+rpt-rpi-v8 #1 SMP PREEMPT Debian 1:6.12.47-1+rpt1 (2025-09-16)` | observed 2026-11-28 |
| Driver | `vc4` on `/dev/dri/card0`; card1 and renderD128 are v3d | observed 2026-11-28 |
| Tools | `props` and `probe` examples of `crates/composition-drm`, built on the host from `54571bb` with `scripts/cross-aarch64.sh` (release, `aarch64-unknown-linux-musl`), copied to `/tmp` on the board | observed 2026-11-28 |
| Privilege | Both ran as the login user, no sudo, no DRM master taken (`props` only reads) | observed 2026-11-28 |
| Display state | Nothing stopped or started. HDMI-A-1 (crtc 102) and DSI-1 (crtc 90) were already scanning out fbcon; the Slint launcher was inactive before and after | observed 2026-11-28 |
| Underruns | `hvs_underrun` read 0 after the runs | observed 2026-11-28 |

Judges (read-only, unchanged): `scripts/in-formats-expect.py`, `scripts/decode-modifiers.py`.

## 1. Planes dumped

`props` lists every plane of the device, not only the ones the engine keeps.

| Class | Planes | Possible CRTCs | zpos | Tag |
|---|---|---|---|---|
| Primary | 6 (one per CRTC index 0 to 5; ids 48, 67, 79, 91, 103, 115) | one each | 0 (immutable) | observed 2026-11-28 |
| Overlay, reachable | 16 (ids 127 to 292) | indices 1 to 5 (mask `0b111110`) | 1..=17 | observed 2026-11-28 |
| Overlay, TXP only | 32 (ids 303 to 644) | index 0 only (mask `0b1`, the writeback CRTC) | 1..=17 | observed 2026-11-28 |
| Cursor | 6 (ids 655 to 710) | one each | 1..=17 | observed 2026-11-28 |
| **Total** | **60** | | | observed 2026-11-28 |

The engine's `MAX_PLANES` cut keeps 12 overlays per output and drops the four reachable overlays
259, 270, 281, 292 (`probe` `KEPT ... dropped_reachable=` lines, `probe-reduced.txt`). The `props`
dump is not cut, so all 60 planes, the dropped overlays and the TXP-only overlays included, were
read. observed 2026-11-28

## 2. Raw IN_FORMATS

All 60 planes carry an `IN_FORMATS` blob and every blob decodes to the same content: 35 formats,
5 modifier entries, 49 pairs. The three classes were compared line by line, and there is one
distinct body. The reduced dump (one plane per class: 48, 127, 655) is in
`bcm2711-cm4-in-formats-2026-11-28/props-in-formats-per-class.txt`. observed 2026-11-28

| Order in blob | Modifier | Name (decoded, section 3) | Formats | Pairs | Tag |
|---|---|---|---|---|---|
| 1 | `0x0700000000000001` | `BROADCOM_VC4_T_TILED` | XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 | 8 | observed 2026-11-28 |
| 2 | `0x0700000000000004` | `BROADCOM_SAND128` | NV12 NV21 P030 | 3 | observed 2026-11-28 |
| 3 | `0x0700000000000003` | `BROADCOM_SAND64` | NV12 NV21 | 2 | observed 2026-11-28 |
| 4 | `0x0700000000000005` | `BROADCOM_SAND256` | NV12 NV21 | 2 | observed 2026-11-28 |
| 5 | `0x0000000000000000` | `LINEAR` | XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12 XB12 AB12 BX12 BA12 RX12 RA12 | 34 | observed 2026-11-28 |
| | | | | **49** | observed 2026-11-28 |

Per plane type:

- **Primary, overlay and cursor planes offer the same set.** There is no AFBC, no UIF and no
  SAND32 on any plane. observed 2026-11-28
- **Linear comes last**, not first. The blob lists the Broadcom entries first and `LINEAR` is the
  fifth modifier entry. observed 2026-11-28
- **P030 has no linear pair**; it appears only under SAND128. observed 2026-11-28
- **No SAND modifier carries a column height**: the three SAND values are `3`, `4`, `5` with
  bits 8 and above zero, that is `COL_HEIGHT(0)`. observed 2026-11-28
- The cursor planes do not narrow the list either; their blob is the full 49 pairs. observed
  2026-11-28
- Four formats are absent from the linear list that the 6.18 `hvs_formats[]` table, read for
  `docs/vc4-formats-source.md`, would add on gen 5: RX24 RA24 BX24 BA24 (35 distinct formats
  here, 39 in the table). The cause was not examined: the board kernel is 6.12.47 and the table
  was read from `rpi-6.18.y` and the 6.12 tip, not from the commit this kernel was built from.
  inferred (open)

## 3. Judge verdicts

`python3 scripts/in-formats-expect.py props-output.txt`, run on the host over the full `props`
stdout (60 plane lines, all identical apart from the plane id; the three class representatives are
kept in `in-formats-expect-verdict.txt`). observed 2026-11-28

```
plane 48 pairs 49 over-max yes (max 32) linear-first no afbc-beyond-max 0
plane 127 pairs 49 over-max yes (max 32) linear-first no afbc-beyond-max 0
plane 655 pairs 49 over-max yes (max 32) linear-first no afbc-beyond-max 0
```

- `over-max yes`: 49 pairs against `MAX_FORMATS` 32, so the cap hides 17 pairs on every plane. inferred
  from the verdict and the count
- `linear-first no`: the blob head is the Broadcom pairs, yet the engine saw 32 formats with no
  modifier suffix (`probe-reduced.txt`: `formats (32): XR24 ... BA12`), so the backend's cut
  retains linear pairs and drops the rest. The 17 hidden pairs are then the 15 non-linear pairs
  plus the linear pairs RX12 and RA12. inferred (matches `docs/vc4-formats-source.md` section 4
  row "BCM2711 as recorded")
- `afbc-beyond-max 0`: there is no ARM vendor modifier on this SoC. observed 2026-11-28
- The `probe` output cannot be judged by `in-formats-expect.py`: it printed the engine's capped
  `formats (32)` lines and no `IN_FORMATS (n)` lines, so the script exits 2 with "no plane lines
  found". Only the `props` text was judged. observed 2026-11-28

`python3 -I scripts/decode-modifiers.py` over the dump (every distinct value, one line each):

```
MODIFIER 0x0700000000000001 BROADCOM_VC4_T_TILED
MODIFIER 0x0700000000000003 BROADCOM_SAND64
MODIFIER 0x0700000000000004 BROADCOM_SAND128
MODIFIER 0x0700000000000005 BROADCOM_SAND256
MODIFIER 0x0000000000000000 LINEAR
```

Fed the whole dump, the script printed each of these five values 60 times, once per plane, and
nothing else. **Every value decodes; none is unknown.** The script also names
`0x0700000000000002` as `BROADCOM_SAND32` (`decode-modifiers.txt`), which is not offered here.
observed 2026-11-28

## 4. Modifier numbering against drm_fourcc.h

Header read: `include/uapi/drm/drm_fourcc.h` of the kernel that runs, taken from the board's own
package `linux-headers-6.12.47+rpt-common-rpi` (`/usr/src/linux-headers-6.12.47+rpt-common-rpi/include/uapi/drm/drm_fourcc.h`,
md5 `17479a8e15bb81fbee46522bb25f9a33`, same package version as the running kernel
`1:6.12.47-1+rpt1`). read from source

Tree and commit: `raspberrypi/linux`, branch `rpi-6.12.y`. The Debian package is dated 2025-09-16;
the Raspberry Pi tag `stable_20250916` resolves to commit
`359f37f0faefb712add32a39f98751aea67d5c1f` (GitHub tag page,
https://github.com/raspberrypi/linux/releases/tag/stable_20250916). The header was fetched from
`https://raw.githubusercontent.com/raspberrypi/linux/359f37f0faefb712add32a39f98751aea67d5c1f/include/uapi/drm/drm_fourcc.h`
(1650 lines) and has the same md5 as the installed package's copy, so the two are byte-identical.
That the tag is the build of this exact kernel rests on the package date and the tag name; the
header content at that commit is read from source (commit `359f37f0`).

| Define in the header | Value | Dump value | Match | Tag |
|---|---|---|---|---|
| `DRM_FORMAT_MOD_LINEAR` = `fourcc_mod_code(NONE, 0)` | `0x0` | `0x0000000000000000` | yes | read from source, observed 2026-11-28 |
| `DRM_FORMAT_MOD_VENDOR_BROADCOM` | `0x07` | high byte `0x07` on four values | yes | read from source, observed 2026-11-28 |
| `DRM_FORMAT_MOD_BROADCOM_VC4_T_TILED` = `fourcc_mod_code(BROADCOM, 1)` | `0x0700000000000001` | `0x0700000000000001` | yes | read from source, observed 2026-11-28 |
| `..._SAND32_COL_HEIGHT(v)` = `fourcc_mod_broadcom_code(2, v)` | `0x07000000000000(v<<8)\|2` | not offered | n/a | read from source, observed 2026-11-28 |
| `..._SAND64_COL_HEIGHT(v)` = `fourcc_mod_broadcom_code(3, v)` | `3 \| v << 8` | `0x0700000000000003` (v = 0) | yes | read from source, observed 2026-11-28 |
| `..._SAND128_COL_HEIGHT(v)` = `fourcc_mod_broadcom_code(4, v)` | `4 \| v << 8` | `0x0700000000000004` (v = 0) | yes | read from source, observed 2026-11-28 |
| `..._SAND256_COL_HEIGHT(v)` = `fourcc_mod_broadcom_code(5, v)` | `5 \| v << 8` | `0x0700000000000005` (v = 0) | yes | read from source, observed 2026-11-28 |
| `__fourcc_mod_broadcom_param_shift` | 8 | SAND values have no bits 8 and above | v = 0 | read from source, observed 2026-11-28 |
| `DRM_FORMAT_MOD_BROADCOM_UIF` = `fourcc_mod_code(BROADCOM, 6)` | `0x0700000000000006` | not offered | n/a | read from source, observed 2026-11-28 |

Reading: code 3 is SAND64, 4 is SAND128, 5 is SAND256. This confirms the correction in
`docs/vc4-hvs-format-source.md` section 2 for the BCM2711 and for the 6.12.47 header
(`docs/observed/bcm2712-topology.md` read code 3 as SAND128). The header comment says only 128-pixel
columns are used in practice and that SAND128 also carries P030 (96 pixels per 128-byte column);
the blob matches that: P030 under SAND128 only. read from source, observed 2026-11-28

Agreement with the source reading: the modifier order T_TILED, SAND128, SAND64, SAND256, LINEAR,
the 8 RGB formats under T_TILED, and NV12 NV21 (P030) under the three SAND values are exactly rows
1 to 4 of `docs/vc4-formats-source.md` section 3.1, and the pair total 49 equals the "BCM2711 as
recorded (5.1)" count there. The BCM2711 blob and the gen-5 source reading do not disagree on the
modifier rows; the design section 5 conflict (blob lists T-tiled and SAND64 where the gen-6 source
rejects them) concerns the BCM2712 and is untouched. observed 2026-11-28 (blob), read from source
(rows)

## 5. Answer to Q3 for BCM2711

- Offered: LINEAR, `VC4_T_TILED`, SAND64, SAND128, SAND256 (all `COL_HEIGHT(0)`), on every plane
  class including cursors and TXP-only overlays. observed 2026-11-28
- Not offered: SAND32, UIF, any ARM or other vendor modifier. observed 2026-11-28
- Linear is the last entry of the blob and carries 34 of the 49 pairs; the capped engine list
  (32 pairs) shows only linear formats. observed 2026-11-28 (blob and engine list), inferred
  (that the cut is what removes the Broadcom pairs)
- Every value decodes and matches `drm_fourcc.h` of the running kernel's headers. read from source
- Not answered here: whether `TEST_ONLY` accepts a T-tiled or SAND buffer on this board
  (`docs/observed/bcm2711-cm4.md` has `TEST_ONLY` results for linear, T_TILED XR24 and NV12 under
  SAND64, SAND128 and SAND256), and the Pi 5 half of Q3. Not repeated.

## 6. Cleanup

On the board only `/tmp/props` and `/tmp/probe` were created; both were removed and `/tmp` holds
only the systemd private directories that were already there. No config, service, DRM, udev or
seat setting was changed, nothing was started or stopped, no kernel module was loaded, and
`slint-launcher.service` stayed inactive. observed 2026-11-28

Evidence directory: `bcm2711-cm4-in-formats-2026-11-28/`

| File | Content |
|---|---|
| `props-in-formats-per-class.txt` | `props` excerpt for planes 48 (Primary), 127 (Overlay), 655 (Cursor): `IN_FORMATS` lines, alpha, blend, rotation, type, zpos |
| `probe-reduced.txt` | `probe` stdout with the overlay planes repeated 12 times trimmed to the first, plus the two `KEPT` lines |
| `in-formats-expect-verdict.txt` | verdict lines of the three class representatives |
| `decode-modifiers.txt` | decoded modifier values |
