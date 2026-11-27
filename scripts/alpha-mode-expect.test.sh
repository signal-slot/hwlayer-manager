#!/bin/sh
# Host test for alpha-mode-expect.py: its unittest self-test, the CLI at
# plane alpha 32768, and an EXPECT JSON round trip through camera-check.py.
# Run: sh scripts/alpha-mode-expect.test.sh

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/alpha-mode-expect.py
root=$(mktemp -d "${TMPDIR:-.}/ame-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
py() { python3 -I "$@"; }

if py "$tool" --self-test > "$root/st.log" 2>&1; then ok "self-test"; else bad "self-test"; cat "$root/st.log"; fi

py "$tool" --plane-alpha 32768 > "$root/out.txt" || bad "cli exit"
for m in premultiplied straight none; do
    grep -q "^  $m .*#" "$root/out.txt" && ok "cli prints $m" || bad "cli prints $m"
done
grep -q '^  none  *#807f80$' "$root/out.txt" && ok "none over green" || bad "none over green"
grep -q '^  straight  *#40bf40$' "$root/out.txt" && ok "straight over green" || bad "straight over green"

py "$tool" --plane-alpha 70000 > /dev/null 2>&1
[ $? -eq 2 ] && ok "bad plane alpha is exit 2" || bad "bad plane alpha"

py "$tool" --expect-json "$root/e.json" --model straight --rect 0.4,0.4,0.6,0.6 > /dev/null || bad "json exit"
cat > "$root/mk.py" <<'P'
import sys
w, h = 40, 40
c = bytes((0x80, 0x7f, 0x80))
open(sys.argv[1], "wb").write(b"P6\n%d %d\n255\n" % (w, h) + c * (w * h))
P
py "$root/mk.py" "$root/p.ppm"
py "$here/camera-check.py" "$root/p.ppm" "$root/e.json" > "$root/cc.log" 2>&1
[ $? -eq 0 ] && ok "camera-check accepts the EXPECT json" || { bad "camera-check round trip"; cat "$root/cc.log"; }

exit $fail
