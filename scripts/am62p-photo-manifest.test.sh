#!/bin/sh
# Self-test for am62p-photo-manifest.py.
# Run: sh scripts/am62p-photo-manifest.test.sh
here=$(cd "$(dirname "$0")" && pwd)
tool=$here/am62p-photo-manifest.py
data=$here/testdata/am62p-photo-manifest
good=$here/testdata/m3-report/good.md
root=$(mktemp -d "${TMPDIR:-.}/am62p-photo-manifest-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }
py() { python3 -I "$@"; }

# 1. fixture photographs carry no metadata
py "$here/check-photo-metadata.py" check "$data"/photos/*.png >/dev/null &&
    ok "fixture photographs are clean" || bad "fixture photographs have metadata"

# 2. manifest of the 8 fixture scenes
mkdir "$root/rep" || exit 2
cp -r "$data/photos" "$root/rep/photos" # the test copies; the tool never does
py "$tool" --photos "$root/rep/photos" --verdicts "$data/verdicts" \
    --link-prefix photos > "$root/out.md" 2> "$root/err" &&
    ok "manifest exits 0" || bad "manifest failed: $(cat "$root/err")"
[ "$(grep -c '^- Scene [1-8]: camera-check ' "$root/out.md")" = 8 ] &&
    ok "8 scene lines" || bad "not 8 scene lines"
want=$(sha256sum "$data/photos/scene3.png" | cut -d' ' -f1)
grep -q "^- Scene 3: camera-check PASS - \[scene3.png\](photos/scene3.png) sha256 $want\$" "$root/out.md" &&
    ok "scene 3 line has link, sha256, verdict" || bad "scene 3 line wrong"
grep -q '^- Scene 5: camera-check FAIL' "$root/out.md" &&
    ok "FAIL verdict carried over" || bad "scene 5 verdict not FAIL"

# 3. the output satisfies check-m3-report.sh: put it into the good report
awk '/^## Photographs and camera-check/ {skip = 1; next}
     /^## / {skip = 0}
     !skip' "$good" > "$root/rep/report.md"
cat "$root/out.md" >> "$root/rep/report.md"
sh "$here/check-m3-report.sh" "$good" >/dev/null || bad "good.md itself fails"
sh "$here/check-m3-report.sh" "$root/rep/report.md" > "$root/chk" 2>&1 &&
    ok "check-m3-report accepts report with manifest lines" ||
    bad "check-m3-report rejects: $(cat "$root/chk")"
# without the manifest lines the same report must fail (the check is real)
grep -v '^- Scene [1-8]:' "$root/rep/report.md" > "$root/rep/none.md"
sh "$here/check-m3-report.sh" "$root/rep/none.md" >/dev/null 2>&1 &&
    bad "report without scene lines passed" || ok "report without scene lines fails"
grep -Eq '[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+|/home/|/tmp' "$root/out.md" &&
    bad "output has an address or host path" || ok "output has no address or host path"

# 4. missing inputs: exit 1, nothing on stdout
mkdir "$root/p2" "$root/v2"
cp "$data"/photos/* "$root/p2/"
cp "$data"/verdicts/* "$root/v2/"
rm "$root/p2/scene7.png"
py "$tool" --photos "$root/p2" --verdicts "$root/v2" --link-prefix p > "$root/o" 2> "$root/e"
[ $? = 1 ] && [ ! -s "$root/o" ] && grep -q 'scene 7: no photograph' "$root/e" &&
    ok "missing photo: exit 1" || bad "missing photo not reported"
cp "$data/photos/scene7.png" "$root/p2/"
rm "$root/v2/scene2.txt"
py "$tool" --photos "$root/p2" --verdicts "$root/v2" --link-prefix p > "$root/o" 2> "$root/e"
[ $? = 1 ] && grep -q 'scene 2: no camera-check output' "$root/e" &&
    ok "missing verdict file: exit 1" || bad "missing verdict not reported"
echo "error: bad picture" > "$root/v2/scene2.txt"
py "$tool" --photos "$root/p2" --verdicts "$root/v2" --link-prefix p > "$root/o" 2> "$root/e"
[ $? = 1 ] && grep -q "scene 2: no 'VERDICT" "$root/e" &&
    ok "verdict line absent: exit 1" || bad "absent VERDICT line accepted"

# 5. json verdict, jpg name, last VERDICT wins, --require-pass
echo '{"pass": true}' > "$root/v2/scene2.json"
rm "$root/v2/scene2.txt"
echo x > "$root/p2/scene4.jpg"
rm "$root/p2/scene4.png"
printf 'VERDICT: FAIL\nVERDICT: PASS\n' > "$root/v2/scene6.txt"
py "$tool" --photos "$root/p2" --verdicts "$root/v2" --link-prefix p > "$root/o" 2>&1
grep -q '^- Scene 2: camera-check PASS' "$root/o" &&
    grep -q '^- Scene 4: camera-check PASS - \[scene4.jpg\](p/scene4.jpg)' "$root/o" &&
    grep -q '^- Scene 6: camera-check PASS' "$root/o" &&
    ok "json verdict, .jpg, last VERDICT" || bad "json/jpg/last-verdict case"
py "$tool" --photos "$root/p2" --verdicts "$root/v2" --link-prefix p --require-pass >/dev/null 2>&1
[ $? = 1 ] && ok "--require-pass rejects FAIL" || bad "--require-pass accepted FAIL"
cp "$data/photos/scene4.png" "$root/p2/"
py "$tool" --photos "$root/p2" --verdicts "$root/v2" --link-prefix p >/dev/null 2>"$root/e"
[ $? = 1 ] && grep -q 'more than one' "$root/e" && ok "two photos of a scene: exit 1" ||
    bad "ambiguous photo accepted"

# 6. usage and absolute prefix
py "$tool" --photos "$root/p2" >/dev/null 2>&1
[ $? = 2 ] && ok "usage error: exit 2" || bad "usage exit"
py "$tool" --photos "$root/p2" --verdicts "$root/v2" --link-prefix /abs >/dev/null 2>&1
[ $? = 2 ] && ok "absolute prefix refused" || bad "absolute prefix accepted"

[ "$fail" = 0 ] && echo "am62p-photo-manifest.test: all ok"
exit "$fail"
