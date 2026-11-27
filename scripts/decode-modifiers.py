#!/usr/bin/env python3
"""Name 64-bit DRM format modifiers (AFBC flags, Broadcom SAND/T-tiled, ...).

Run: python3 -I scripts/decode-modifiers.py [VALUE ...]
Values are hex (0x...) or decimal arguments. Without arguments, stdin is
scanned line by line for modifier-looking tokens (see the README).
Output: one line per value, `MODIFIER 0x<hex> <name>`.
Python 3 standard library only.
"""
import re
import sys

# Values below are copied from include/uapi/drm/drm_fourcc.h of Linux 6.18
# (DRM_FORMAT_MOD_VENDOR_*, fourcc_mod_code, DRM_FORMAT_MOD_ARM_*,
# AFBC_FORMAT_MOD_*, AFRC_FORMAT_MOD_*, DRM_FORMAT_MOD_BROADCOM_*).

VENDORS = {
    0x00: "NONE", 0x01: "INTEL", 0x02: "AMD", 0x03: "NVIDIA",
    0x04: "SAMSUNG", 0x05: "QCOM", 0x06: "VIVANTE", 0x07: "BROADCOM",
    0x08: "ARM", 0x09: "ALLWINNER", 0x0A: "AMLOGIC",
    0x0B: "MTK", 0x0C: "APPLE",
}
V_NONE, V_BROADCOM, V_ARM = 0x00, 0x07, 0x08

MOD_LINEAR = 0                       # DRM_FORMAT_MOD_LINEAR
MOD_INVALID = (1 << 56) - 1          # fourcc_mod_code(NONE, (1ULL << 56) - 1)
MASK56 = (1 << 56) - 1
MASK64 = (1 << 64) - 1

# DRM_FORMAT_MOD_ARM_CODE(type, val) =
#   fourcc_mod_code(ARM, type << 52 | val & 0xfffffffffffff)
ARM_TYPE_SHIFT = 52
ARM_TYPE_AFBC = 0x00
ARM_TYPE_AFRC = 0x02
ARM_TYPE_MISC = 0x01
ARM_VAL_MASK = (1 << 52) - 1

# AFBC_FORMAT_MOD_*
AFBC_BLOCK_SIZE_MASK = 0xF
AFBC_BLOCK_SIZES = {1: "16x16", 2: "32x8", 3: "64x4", 4: "32x8_64x4"}
AFBC_FLAGS = [  # (bit, name) in header order
    (1 << 4, "YTR"), (1 << 5, "SPLIT"), (1 << 6, "SPARSE"), (1 << 7, "CBR"),
    (1 << 8, "TILED"), (1 << 9, "SC"), (1 << 10, "DB"), (1 << 11, "BCH"),
    (1 << 12, "USM"),
]

# AFRC_FORMAT_MOD_*: CU_SIZE_P0 bits 0-3, CU_SIZE_P12 bits 4-7, LAYOUT_SCAN bit 8
AFRC_CU_SIZES = {1: "16", 2: "24", 3: "32"}
AFRC_LAYOUT_SCAN = 1 << 8

# DRM_FORMAT_MOD_ARM_CODE(MISC, n)
ARM_MISC = {1: "16X16_BLOCK_U_INTERLEAVED"}

# DRM_FORMAT_MOD_BROADCOM_*: fourcc_mod_broadcom_code(val, params) =
# fourcc_mod_code(BROADCOM, params << 8 | val); the param is 48 bits
# (SAND: column height in pixels / lines, see fourcc_mod_broadcom_param).
BC_PARAM_SHIFT = 8
BC_PARAM_BITS = 48
BC_MODS = {1: "VC4_T_TILED", 2: "SAND32", 3: "SAND64", 4: "SAND128",
           5: "SAND256", 6: "UIF"}


def decode_afbc(val):
    bs = val & AFBC_BLOCK_SIZE_MASK
    parts = [AFBC_BLOCK_SIZES.get(bs, "BLOCK_0x%x" % bs)]
    rest = val & ~AFBC_BLOCK_SIZE_MASK
    for bit, name in AFBC_FLAGS:
        if rest & bit:
            parts.append(name)
            rest &= ~bit
    if rest:
        parts.append("0x%x" % rest)
    return "ARM_AFBC(" + "|".join(parts) + ")"


def decode_afrc(val):
    p0, p12 = val & 0xF, (val >> 4) & 0xF
    parts = ["P0_" + AFRC_CU_SIZES.get(p0, "0x%x" % p0),
             "P12_" + AFRC_CU_SIZES.get(p12, "0x%x" % p12)]
    rest = val & ~0xFF
    if rest & AFRC_LAYOUT_SCAN:
        parts.append("LAYOUT_SCAN")
        rest &= ~AFRC_LAYOUT_SCAN
    if rest:
        parts.append("0x%x" % rest)
    return "ARM_AFRC(" + "|".join(parts) + ")"


def decode_arm(code):
    typ, val = code >> ARM_TYPE_SHIFT, code & ARM_VAL_MASK
    if typ == ARM_TYPE_AFBC:
        return decode_afbc(val)
    if typ == ARM_TYPE_AFRC:
        return decode_afrc(val)
    if typ == ARM_TYPE_MISC and val in ARM_MISC:
        return "ARM_" + ARM_MISC[val]
    return "ARM 0x%x" % code


def decode_broadcom(code):
    name = BC_MODS.get(code & 0xFF)
    param = (code >> BC_PARAM_SHIFT) & ((1 << BC_PARAM_BITS) - 1)
    if name is None:
        return "BROADCOM 0x%x" % code
    if param:
        return "BROADCOM_%s(param=%d)" % (name, param)
    return "BROADCOM_" + name


def decode(m):
    """Return the name of a 64-bit modifier."""
    m &= MASK64
    if m == MOD_LINEAR:
        return "LINEAR"
    if m == MOD_INVALID:
        return "INVALID"
    vendor, code = m >> 56, m & MASK56
    if vendor == V_ARM:
        return decode_arm(code)
    if vendor == V_BROADCOM:
        return decode_broadcom(code)
    vname = VENDORS.get(vendor, "vendor-0x%02x" % vendor)
    return "%s 0x%x" % (vname, code)


def parse_arg(s):
    s = s.strip()
    return int(s, 16) if s.lower().startswith("0x") else int(s, 10)


# stdin: a 0x token of 9..16 hex digits, or any 0x token right after the word
# "modifier" / "mod" (so a plain `modifier=0x0` is found as LINEAR).
MOD_RE = re.compile(
    r"\bmod(?:ifier)?s?\b\W{0,3}(0[xX][0-9a-fA-F]+)\b"
    r"|\b(0[xX][0-9a-fA-F]{9,16})\b")


def scan(text):
    for line in text.splitlines():
        for a, b in MOD_RE.findall(line):
            yield int(a or b, 16)


def main(argv):
    if argv:
        try:
            vals = [parse_arg(a) for a in argv]
        except ValueError as e:
            print("decode-modifiers: bad value: %s" % e, file=sys.stderr)
            return 2
    else:
        vals = list(scan(sys.stdin.read()))
    for v in vals:
        print("MODIFIER 0x%016x %s" % (v & MASK64, decode(v)))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
