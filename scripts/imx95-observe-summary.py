#!/usr/bin/env python3
"""Summarise an imx95-observe.sh snapshot into a short Markdown ownership table.

usage: imx95-observe-summary.py [FILE]    (reads stdin without FILE)
exit:  0 ok, 1 unparsable input, 2 usage
"""
import re
import sys

SECTIONS = ("date", "uname -r", "remoteproc", "device tree", "dmesg",
            "debugfs clk", "debugfs pm_genpd")


def section_key(title):
    for key in SECTIONS:
        if title.startswith(key):
            return key
    return title


def split_sections(text):
    lines = text.splitlines()
    if not lines or not lines[0].startswith("# imx95-observe"):
        return None
    secs, cur = {}, None
    for ln in lines[1:]:
        if ln.startswith("### "):
            cur = section_key(ln[4:].strip())
            secs[cur] = []
        elif cur is not None:
            secs[cur].append(ln)
    return secs


def records(body):
    """Blocks of a path line followed by indented 'key: value' lines."""
    out, cur = [], None
    for ln in body:
        if ln.startswith("/"):
            cur = {"path": ln.strip()}
            out.append(cur)
        elif cur is not None and ln.startswith("  ") and ":" in ln:
            k, v = ln.strip().split(":", 1)
            cur[k] = v.strip()
    return out


def cell(s):
    return (s or "-").replace("|", "\\|")


def table(head, rows):
    if not rows:
        return ["none", ""]
    out = ["| " + " | ".join(head) + " |", "|" + "---|" * len(head)]
    out += ["| " + " | ".join(cell(c) for c in r) + " |" for r in rows]
    return out + [""]


def main(argv):
    if len(argv) > 2 or (len(argv) == 2 and argv[1].startswith("-")):
        sys.stderr.write(__doc__)
        return 2
    try:
        if len(argv) == 2:
            with open(argv[1], encoding="utf-8", errors="replace") as f:
                text = f.read()
        else:
            text = sys.stdin.read()
    except OSError as e:
        sys.stderr.write("cannot read input: %s\n" % e)
        return 1
    secs = split_sections(text)
    if secs is None or "remoteproc" not in secs or "device tree" not in secs:
        sys.stderr.write("not an imx95-observe.sh snapshot\n")
        return 1

    o = ["# i.MX95 ownership summary", "", "## remoteproc", ""]
    rp = records(secs["remoteproc"])
    o += table(["instance", "name", "state", "firmware"],
               [(r["path"].rsplit("/", 1)[-1], r.get("name"), r.get("state"),
                 r.get("firmware")) for r in rp])

    nodes = records(secs["device tree"])
    resmem = [n for n in nodes if "/reserved-memory" in n["path"]]
    rest = [n for n in nodes if n not in resmem]

    def match(n, rx):
        return re.search(rx, n["path"] + " " + n.get("compatible", ""), re.I)

    rproc = [n for n in rest if match(n, r"rproc")]
    disp = [n for n in rest if n not in rproc and match(n, r"display|dpu|ldb|lvds|dsi")]
    for title, group in (("display nodes", disp), ("rproc nodes", rproc),
                         ("reserved-memory nodes", resmem)):
        o += ["## " + title, ""]
        o += table(["node", "compatible", "status"],
                   [(n["path"], n.get("compatible"), n.get("status") or "okay (default)")
                    for n in group])

    o += ["## SCMI / SM version", ""]
    ver = [l.strip() for l in secs.get("dmesg", [])
           if re.search(r"scmi|(^|[^a-z])sm([^a-z]|$)", l, re.I)
           and re.search(r"version|v\d", l, re.I)]
    o += ["- " + v for v in ver] if ver else ["not observed"]
    o.append("")

    for key, title in (("debugfs clk", "clock entries (dpu/ldb/lvds/disp)"),
                       ("debugfs pm_genpd", "genpd entries (dpu/ldb/lvds/disp)")):
        o += ["## " + title, ""]
        body = secs.get(key, [])
        hits = [l.strip() for l in body if re.search(r"dpu|ldb|lvds|disp", l, re.I)]
        if body[:1] == ["not readable"] or key not in secs:
            o.append("not readable")
        elif hits:
            o += ["- `" + re.sub(r"\s+", " ", l) + "`" for l in hits]
        else:
            o.append("none")
        o.append("")
    sys.stdout.write("\n".join(o).rstrip("\n") + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
