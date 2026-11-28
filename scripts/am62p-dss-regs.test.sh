#!/bin/sh
# Self-test for am62p-dss-regs.py.
# Run: sh scripts/am62p-dss-regs.test.sh
# Uses the synthetic inputs in scripts/testdata/am62p-dss-regs/.

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/am62p-dss-regs.py
data=$here/testdata/am62p-dss-regs
root=$(mktemp -d "${TMPDIR:-.}/dssregs-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
py() { python3 -I "$tool" "$@"; }
has() { grep -qF -- "$2" "$1" && ok "$3" || bad "$3"; }

# regdump: Markdown
py "$data/regdump.txt" > "$root/reg.md" || bad "regdump exit"
has "$root/reg.md" "| 0 | yes | yes | 800x480 | 0x00000021 |" "regdump VP0 enabled, GO set"
has "$root/reg.md" "| 1 | no | no | n/a | 0x00000000 |" "regdump VP1 disabled"
has "$root/reg.md" "| 1 | yes | 0x07 | 200x100 | 100,50 | 1 | 0 | yes | 128 |" "regdump plane 1"
has "$root/reg.md" "| 2 | no | 0x27 | 200x100 | 0,0 | 2 | 0 | no | 255 |" "regdump disabled plane"

# regdump: JSON
py --json "$data/regdump.txt" > "$root/reg.json" || bad "regdump json exit"
python3 -I - "$root/reg.json" <<'EOF' && ok "regdump json" || bad "regdump json"
import json, sys
d = json.load(open(sys.argv[1]))
assert d["format"] == "regdump"
assert d["vps"][0]["go"] is True and d["vps"][1]["go"] is False
p = d["planes"]
assert [x["enabled"] for x in p] == [True, True, False]
assert p[1]["position"] == [100, 50] and p[1]["size"] == [200, 100]
assert p[1]["alpha"] == 128 and p[1]["zorder"] == 1
EOF

# drm-state
py "$data/drm-state.txt" > "$root/drm.md" || bad "drm-state exit"
has "$root/drm.md" "| crtc-0 | 44 | yes | yes | 800x480 | 3 | n/a |" "drm-state crtc"
has "$root/drm.md" "| plane-1 | 36 | yes | crtc-0 | AR24 little-endian (0x34325241) | 200x100 | 100,50 | 1 | n/a |" "drm-state plane 1"
has "$root/drm.md" "| plane-2 | 41 | no | n/a | n/a |" "drm-state disabled plane"
py --json "$data/drm-state.txt" > "$root/drm.json" || bad "drm-state json exit"
python3 -I - "$root/drm.json" <<'EOF' && ok "drm-state json" || bad "drm-state json"
import json, sys
d = json.load(open(sys.argv[1]))
assert d["format"] == "drm-state"
assert [p["enabled"] for p in d["planes"]] == [True, True, False]
assert d["planes"][0]["zorder"] == 0 and d["vps"][0]["plane_mask"] == 3
EOF

# stdin
py < "$data/drm-state.txt" > "$root/stdin.md" && has "$root/stdin.md" "drm-state" "stdin input"

# unknown format: exit 2, nothing on stdout
py "$data/unknown.txt" > "$root/unk.out" 2> "$root/unk.err"
rc=$?
[ "$rc" = 2 ] && [ ! -s "$root/unk.out" ] && ok "unknown format exits 2" || bad "unknown format exits $rc"
py "$root/missing" >/dev/null 2>&1
[ "$?" = 2 ] && ok "missing file exits 2" || bad "missing file"

[ "$fail" = 0 ] && echo "all passed"
exit "$fail"
