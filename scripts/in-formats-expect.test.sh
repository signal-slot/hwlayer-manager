#!/bin/sh
# Host test for in-formats-expect.py on fixtures written here.
# Run: sh scripts/in-formats-expect.test.sh

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/in-formats-expect.py
root=$(mktemp -d "${TMPDIR:-.}/ife-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
py() { python3 -I "$@"; }

A=0x0800000000000001

# probe-style: short list, linear first.
cat > "$root/short.txt" <<P
planes
plane 31 type primary possible crtcs 0x1
  attached to none
  IN_FORMATS (3): XR24:0x0 AR24:0x0 RG16:0x0
  properties:
P
out=$(py "$tool" "$root/short.txt") || bad "short exit"
[ "$out" = "plane 31 pairs 3 over-max no (max 32) linear-first yes afbc-beyond-max 0" ] && ok "short list" || { bad "short list"; echo "$out"; }

# probe-style: 34 linear pairs then 4 AFBC pairs at the end.
lin=""; i=0
while [ $i -lt 34 ]; do lin="$lin XR24:0x0"; i=$((i+1)); done
printf 'plane 40 type overlay possible crtcs 0x3\n  IN_FORMATS (38):%s XR24:%s AR24:%s XB24:%s AB24:%s\n' "$lin" $A $A $A $A > "$root/long.txt"
out=$(py "$tool" "$root/long.txt") || bad "long exit"
[ "$out" = "plane 40 pairs 38 over-max yes (max 32) linear-first yes afbc-beyond-max 4" ] && ok "long, AFBC at end" || { bad "long, AFBC at end"; echo "$out"; }

# props-style: AFBC entry first, linear later; second plane has no IN_FORMATS.
cat > "$root/props.txt" <<P
plane 51: possible_crtcs 0b1 (crtc indices [0]), current crtc None, fb None
    IN_FORMATS (id 9): blob = 12
      IN_FORMATS: 2 formats, 2 modifiers
      modifier $A: XR24 AR24
      modifier 0x0000000000000000: XR24 AR24
    type (id 3): enum = 1
plane 52: possible_crtcs 0b1 (crtc indices [0]), current crtc None, fb None
    type (id 3): enum = 0
P
out=$(py "$tool" "$root/props.txt") || bad "props exit"
want="plane 51 pairs 4 over-max no (max 32) linear-first no afbc-beyond-max 0
plane 52 pairs none (no IN_FORMATS) over-max no (max 32) linear-first absent afbc-beyond-max 0"
[ "$out" = "$want" ] && ok "linear not first (props)" || { bad "linear not first"; echo "$out"; }

# --max-formats override changes the beyond-max count.
out=$(py "$tool" --max-formats 36 "$root/long.txt")
case $out in *"over-max yes (max 36)"*"afbc-beyond-max 2") ok "override" ;; *) bad "override: $out" ;; esac

# parse errors: count mismatch, garbage pair, no planes.
printf 'plane 1 type primary possible crtcs 0x1\n  IN_FORMATS (5): XR24:0x0\n' > "$root/e1.txt"
printf 'plane 1 type primary possible crtcs 0x1\n  IN_FORMATS (1): XR24-zero\n' > "$root/e2.txt"
printf 'nothing here\n' > "$root/e3.txt"
for e in e1 e2 e3; do
    py "$tool" "$root/$e.txt" > /dev/null 2>&1
    [ $? -eq 2 ] && ok "parse error $e is exit 2" || bad "parse error $e"
done

exit $fail
