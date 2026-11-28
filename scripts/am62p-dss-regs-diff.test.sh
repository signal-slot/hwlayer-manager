#!/bin/sh
# Self-test for am62p-dss-regs-diff.py.
# Run: sh scripts/am62p-dss-regs-diff.test.sh
# Modified copies of the existing fixtures are built under $TMPDIR.

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/am62p-dss-regs-diff.py
data=$here/testdata/am62p-dss-regs
root=$(mktemp -d "${TMPDIR:-.}/dssdiff-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
py() { python3 -I "$tool" "$@"; }
has() { grep -qF -- "$2" "$1" && ok "$3" || bad "$3"; }

# regdump: clear GO, move plane 1, change its alpha, disable plane 0
sed -e 's/^VP0 CONTROL = .*/VP0 CONTROL = 0x00000001/' \
    -e 's/^VID1 POSITION = .*/VID1 POSITION = 0x00400010/' \
    -e 's/^VID1 GLOBAL_ALPHA = .*/VID1 GLOBAL_ALPHA = 0x000000c0/' \
    -e 's/^VID0 ATTRIBUTES = .*/VID0 ATTRIBUTES = 0x0000004e/' \
    "$data/regdump.txt" > "$root/reg2.txt"
py "$data/regdump.txt" "$root/reg2.txt" > "$root/reg.md"; rc=$?
[ $rc -eq 0 ] && ok "regdump exit 0" || bad "regdump exit $rc"
has "$root/reg.md" "| 1 | position | 100,50 | 16,64 |" "position change"
has "$root/reg.md" "| 1 | alpha | 128 | 192 |" "alpha change"
has "$root/reg.md" "| 0 | enabled | yes | no |" "enable change"
has "$root/reg.md" "| 0 | go | yes | no |" "GO bit change"
grep -qF "| 2 |" "$root/reg.md" && bad "unchanged plane 2 listed" || ok "unchanged plane omitted"

py --json "$data/regdump.txt" "$root/reg2.txt" > "$root/reg.json" || bad "json exit"
python3 -I - "$root/reg.json" <<'PY' && ok "regdump json" || bad "regdump json"
import json, sys
d = json.load(open(sys.argv[1]))
assert d["format"] == "regdump"
p = dict((e["id"], e) for e in d["planes"])
assert sorted(p) == [0, 1]
assert p[1]["fields"]["position"] == {"before": [100, 50], "after": [16, 64]}
assert d["vps"][0]["fields"]["go"] == {"before": True, "after": False}
PY

# identical input: no change, exit 0
py "$data/regdump.txt" "$data/regdump.txt" > "$root/same.md"; rc=$?
[ $rc -eq 0 ] && ok "identical exit 0" || bad "identical exit $rc"
[ "$(grep -cF '(no change)' "$root/same.md")" -eq 2 ] && ok "identical: no change" || bad "identical: no change"

# drm-state: change the format of the first plane
sed -e '0,/format=.*/s//format=RG16 little-endian (0x36314752)/' \
    "$data/drm-state.txt" > "$root/drm2.txt"
py "$data/drm-state.txt" "$root/drm2.txt" > "$root/drm.md"; rc=$?
[ $rc -eq 0 ] && ok "drm exit 0" || bad "drm exit $rc"
has "$root/drm.md" "| plane-0 | format | XR24 little-endian (0x34325258) | RG16 little-endian (0x36314752) |" "drm format change"
py "$data/drm-state.txt" "$data/drm-state.txt" | grep -qF "(no change)" && ok "drm identical" || bad "drm identical"

# errors: mixed formats, unknown input, usage; exit 2 with empty stdout
for args in "$data/regdump.txt $data/drm-state.txt" "$data/unknown.txt $data/unknown.txt" "$data/regdump.txt" "--bogus a b"; do
    out=$(py $args 2>/dev/null); rc=$?
    [ $rc -eq 2 ] && [ -z "$out" ] && ok "exit 2 case" || bad "exit 2 for $args"
done

exit $fail
