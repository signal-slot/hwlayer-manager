#!/usr/bin/env python3
"""Parse a saved, read-only AM62P DSS snapshot into per-VP and per-plane tables.

Run: python3 -I scripts/am62p-dss-regs.py [--json] [FILE]
Reads FILE (or stdin). Two text formats, detected from the content:
  drm-state  /sys/kernel/debug/dri/N/state (DRM atomic state dump)
  regdump    a register dump in the line format of the README
Exit status: 0 parsed, 2 unknown format or usage error.
The script only parses text; it never reads /dev/mem or any device.
Python 3 standard library only.
"""
import json
import re
import sys

# Bit fields of the regdump format. Source and verification status of each
# entry: scripts/am62p-dss-regs-README.md ("Register fields").
VP_CONTROL_ENABLE = (0, 0)
VP_CONTROL_GOBIT = (5, 5)
VP_SIZE_SCREEN_W = (11, 0)       # width - 1
VP_SIZE_SCREEN_H = (27, 16)      # height - 1
VID_ATTR_ENABLE = (0, 0)
VID_ATTR_FORMAT = (6, 1)
VID_GLOBAL_ALPHA = (7, 0)
VID_POS_X = (13, 0)
VID_POS_Y = (29, 16)
VID_SIZE_W = (13, 0)             # width - 1
VID_SIZE_H = (29, 16)            # height - 1
OVR_ATTR_ENABLE = (0, 0)
OVR_ATTR_PLANE = (4, 1)          # hardware plane placed in this layer
OVR_ATTR_VP = (13, 10)           # video port the layer input comes from


def field(val, bits):
    hi, lo = bits
    return (val >> lo) & ((1 << (hi - lo + 1)) - 1)


def signed(text):
    return int(text.replace("+", ""))


# ---- DRM atomic state ----------------------------------------------------

def parse_drm_state(lines):
    planes, vps = [], []
    cur = None
    for raw in lines:
        line = raw.rstrip("\n")
        m = re.match(r"^(plane|crtc)\[(\d+)\]: (.*)$", line)
        if m:
            kind, num, name = m.group(1), int(m.group(2)), m.group(3).strip()
            cur = {"object_id": num, "name": name}
            if kind == "plane":
                cur.update(crtc=None, fb=0, format=None, size=None,
                           position=None, src_size=None, zorder=None,
                           alpha=None, enabled=False)
                planes.append(cur)
            else:
                cur.update(enable=None, active=None, mode=None,
                           plane_mask=None, go=None)
                vps.append(cur)
            cur["_kind"] = kind
            continue
        if cur is None:
            continue
        s = line.strip()
        if cur["_kind"] == "plane":
            if s.startswith("crtc="):
                v = s[5:]
                cur["crtc"] = None if v.startswith("(null)") else v
            elif s.startswith("fb="):
                cur["fb"] = int(s[3:] or 0)
            elif s.startswith("format="):
                cur["format"] = s[7:]
            elif s.startswith("crtc-pos="):
                m = re.match(r"crtc-pos=(\d+)x(\d+)([+-]\d+)([+-]\d+)", s)
                if m:
                    cur["size"] = [int(m.group(1)), int(m.group(2))]
                    cur["position"] = [signed(m.group(3)), signed(m.group(4))]
            elif s.startswith("src-pos="):
                m = re.match(r"src-pos=(\d+)\.\d+x(\d+)\.\d+", s)
                if m:
                    cur["src_size"] = [int(m.group(1)), int(m.group(2))]
            elif s.startswith("normalized-zpos="):
                cur["zorder"] = int(s.split("=", 1)[1], 16)
            elif s.startswith("alpha="):
                cur["alpha"] = int(s.split("=", 1)[1], 0)
        else:
            if s.startswith("enable="):
                cur["enable"] = bool(int(s[7:]))
            elif s.startswith("active="):
                cur["active"] = bool(int(s[7:]))
            elif s.startswith("plane_mask="):
                cur["plane_mask"] = int(s[11:], 16)
            elif s.startswith("mode:"):
                m = re.match(r'mode: "([^"]*)"', s)
                if m:
                    cur["mode"] = m.group(1)
    for p in planes:
        p["enabled"] = p["fb"] != 0 and p["crtc"] is not None
        del p["_kind"]
    for v in vps:
        del v["_kind"]
    return {"format": "drm-state", "vps": vps, "planes": planes}


# ---- register dump -------------------------------------------------------

REG_RE = re.compile(
    r"^(VP|VID|OVR)(\d+)(?:\.L(\d+))?\s+([A-Z_]+)\s*=\s*(0x[0-9a-fA-F]+)\s*$")


def parse_regdump(lines):
    regs = {}
    for raw in lines:
        line = raw.split("#", 1)[0].strip()
        m = REG_RE.match(line)
        if m:
            kind, n, layer, name, val = m.groups()
            regs[(kind, int(n), None if layer is None else int(layer),
                  name)] = int(val, 16)
    vps = []
    for n in sorted({k[1] for k in regs if k[0] == "VP"}):
        ctl = regs.get(("VP", n, None, "CONTROL"))
        scr = regs.get(("VP", n, None, "SIZE_SCREEN"))
        vps.append({
            "id": n,
            "enable": None if ctl is None else bool(field(ctl, VP_CONTROL_ENABLE)),
            "go": None if ctl is None else bool(field(ctl, VP_CONTROL_GOBIT)),
            "size": None if scr is None else [field(scr, VP_SIZE_SCREEN_W) + 1,
                                              field(scr, VP_SIZE_SCREEN_H) + 1],
            "control_raw": None if ctl is None else "0x%08x" % ctl,
        })
    # layer table: (ovr, layer) -> plane, vp, enable
    layers = {}
    for (kind, n, layer, name), val in regs.items():
        if kind == "OVR" and name == "ATTRIBUTES" and layer is not None:
            layers[(n, layer)] = val
    planes = []
    for n in sorted({k[1] for k in regs if k[0] == "VID"}):
        attr = regs.get(("VID", n, None, "ATTRIBUTES"))
        pos = regs.get(("VID", n, None, "POSITION"))
        sz = regs.get(("VID", n, None, "SIZE"))
        if sz is None:
            sz = regs.get(("VID", n, None, "PICTURE_SIZE"))
        alpha = regs.get(("VID", n, None, "GLOBAL_ALPHA"))
        zorder = vp = None
        layer_on = None
        for (ovr, layer), val in sorted(layers.items()):
            if field(val, OVR_ATTR_PLANE) == n:
                zorder, vp = layer, field(val, OVR_ATTR_VP)
                layer_on = bool(field(val, OVR_ATTR_ENABLE))
                break
        planes.append({
            "id": n,
            "enabled": None if attr is None else bool(field(attr, VID_ATTR_ENABLE)),
            "format_code": None if attr is None else "0x%02x" % field(attr, VID_ATTR_FORMAT),
            "size": None if sz is None else [field(sz, VID_SIZE_W) + 1,
                                             field(sz, VID_SIZE_H) + 1],
            "position": None if pos is None else [field(pos, VID_POS_X),
                                                  field(pos, VID_POS_Y)],
            "zorder": zorder,
            "vp": vp,
            "layer_enabled": layer_on,
            "alpha": None if alpha is None else field(alpha, VID_GLOBAL_ALPHA),
        })
    return {"format": "regdump", "vps": vps, "planes": planes}


def detect(lines):
    for raw in lines:
        if re.match(r"^(plane|crtc)\[\d+\]: ", raw):
            return "drm-state"
        if REG_RE.match(raw.split("#", 1)[0].strip()):
            return "regdump"
    return None


# ---- output --------------------------------------------------------------

def cell(v):
    if v is None:
        return "n/a"
    if isinstance(v, bool):
        return "yes" if v else "no"
    if isinstance(v, list):
        if len(v) == 2 and all(isinstance(x, int) for x in v):
            return "%dx%d" % tuple(v) if v is not None else "n/a"
        return ",".join(str(x) for x in v)
    return str(v)


def table(title, rows, cols):
    out = ["### " + title, ""]
    if not rows:
        return out + ["(none)", ""]
    out.append("| " + " | ".join(h for h, _ in cols) + " |")
    out.append("|" + "|".join("---" for _ in cols) + "|")
    for r in rows:
        out.append("| " + " | ".join(cell(r.get(k)) for _, k in cols) + " |")
    return out + [""]


def pos_cell(r):
    p = r.get("position")
    return None if p is None else "%d,%d" % tuple(p)


def markdown(doc):
    out = ["Source format: " + doc["format"], ""]
    if doc["format"] == "drm-state":
        out += table("Video ports (CRTCs)", doc["vps"], [
            ("crtc", "name"), ("id", "object_id"), ("enable", "enable"),
            ("active", "active"), ("mode", "mode"),
            ("plane mask", "plane_mask"), ("GO bit", "go")])
        rows = [dict(p, pos=pos_cell(p)) for p in doc["planes"]]
        out += table("Planes", rows, [
            ("plane", "name"), ("id", "object_id"), ("enabled", "enabled"),
            ("crtc", "crtc"), ("format", "format"), ("size", "size"),
            ("position", "pos"), ("zorder", "zorder"), ("alpha", "alpha")])
    else:
        out += table("Video ports", doc["vps"], [
            ("vp", "id"), ("enable", "enable"), ("GO bit", "go"),
            ("size", "size"), ("CONTROL", "control_raw")])
        rows = [dict(p, pos=pos_cell(p)) for p in doc["planes"]]
        out += table("Planes", rows, [
            ("plane", "id"), ("enabled", "enabled"),
            ("format code", "format_code"), ("size", "size"),
            ("position", "pos"), ("zorder", "zorder"), ("vp", "vp"),
            ("layer on", "layer_enabled"), ("alpha", "alpha")])
    return "\n".join(out)


def main(argv):
    args = argv[1:]
    as_json = "--json" in args
    args = [a for a in args if a != "--json"]
    if len(args) > 1 or any(a.startswith("-") for a in args):
        sys.stderr.write("usage: am62p-dss-regs.py [--json] [FILE]\n")
        return 2
    try:
        if args:
            with open(args[0], encoding="utf-8", errors="replace") as f:
                lines = f.read().splitlines()
        else:
            lines = sys.stdin.read().splitlines()
    except OSError as e:
        sys.stderr.write("am62p-dss-regs: %s\n" % e)
        return 2
    fmt = detect(lines)
    if fmt is None:
        sys.stderr.write("am62p-dss-regs: unknown input format\n")
        return 2
    doc = parse_drm_state(lines) if fmt == "drm-state" else parse_regdump(lines)
    if as_json:
        print(json.dumps(doc, indent=2, sort_keys=True))
    else:
        print(markdown(doc))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
