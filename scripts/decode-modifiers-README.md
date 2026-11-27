# decode-modifiers.py

Names 64-bit DRM format modifiers. Python 3 standard library only.

```
python3 -I scripts/decode-modifiers.py 0x0700000000000003 0x800000000000041 72057594037927935
python3 -I scripts/decode-modifiers.py < props-or-probe-dump.txt
bash scripts/decode-modifiers.test.sh
```

Arguments are hex (`0x...`) or decimal. With no arguments the script reads
stdin and decodes, line by line, every `0x` token of 9 to 16 hex digits, and
any `0x` token directly after the word `modifier` or `mod` (so `modifier=0x0`
gives `LINEAR`). Short hex tokens elsewhere (zpos, plane ids) are ignored.

Output is one line per value: `MODIFIER 0x<16 hex digits> <name>`.

The constants are copied from `include/uapi/drm/drm_fourcc.h` of Linux 6.18
(comment at the top of the script) and were checked against the file fetched
from git.kernel.org (tag v6.18).

## Names

| Value | Name |
| --- | --- |
| `0` | `LINEAR` |
| `0x00ffffffffffffff` | `INVALID` |
| vendor 8 (ARM), type 0 | `ARM_AFBC(<block>[\|YTR\|SPLIT\|SPARSE\|CBR\|TILED\|SC\|DB\|BCH\|USM])`, block is `16x16`, `32x8`, `64x4`, `32x8_64x4` or `BLOCK_0x<n>`; bits outside the known flags are appended as `0x<bits>` |
| vendor 8, type 2 | `ARM_AFRC(P0_<16\|24\|32>\|P12_<...>[\|LAYOUT_SCAN])` |
| vendor 8, type 1 (MISC), value 1 | `ARM_16X16_BLOCK_U_INTERLEAVED` |
| vendor 7 (Broadcom), code 1..6 | `BROADCOM_VC4_T_TILED`, `BROADCOM_SAND32`, `BROADCOM_SAND64`, `BROADCOM_SAND128`, `BROADCOM_SAND256`, `BROADCOM_UIF` |
| Broadcom with a parameter | `BROADCOM_SAND128(param=<n>)`; the parameter is bits 8..55 (`fourcc_mod_broadcom_param`), the SAND column height |
| any other vendor or code | `<vendor> 0x<code>`: vendor is `NONE`, `INTEL`, `AMD`, `NVIDIA`, `SAMSUNG`, `QCOM`, `VIVANTE`, `BROADCOM`, `ARM`, `ALLWINNER`, `AMLOGIC`, `MTK`, `APPLE`, or `vendor-0x<byte>` for an unknown vendor byte; code is the low 56 bits |

## The Pi 5 IN_FORMATS conflict

`docs/hvs-backend-design.md` §4/§5 says the `IN_FORMATS` blob observed on the
Pi 5 "lists the Broadcom codes 1, 3, 4 and 5", while the source says T-tiled
and SAND64 are refused on HVS6. Those raw codes (vendor byte 7, so
`0x0700000000000001`, `...03`, `...04`, `...05`) decode to:

| Code | Name |
| --- | --- |
| 1 | `BROADCOM_VC4_T_TILED` |
| 3 | `BROADCOM_SAND64` |
| 4 | `BROADCOM_SAND128` |
| 5 | `BROADCOM_SAND256` |

So the blob, as described, lists exactly T-tiled and SAND64 (the two the
source refuses on gen 6) plus SAND128 and SAND256 (the two it allows);
SAND32 (2) and UIF (6) are absent. The conflict can be checked mechanically by
feeding a recorded blob dump to the script and looking for
`BROADCOM_VC4_T_TILED` and `BROADCOM_SAND64`. This script only names values; it
does not decide which side is right (that needs a TEST_ONLY run on the board).
