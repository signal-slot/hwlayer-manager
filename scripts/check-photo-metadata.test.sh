#!/bin/sh
# Self-test for check-photo-metadata.py.
# Run: sh scripts/check-photo-metadata.test.sh
# Builds tiny JPEG and PNG fixtures under a mktemp dir in $TMPDIR; never touches the repository.

here=$(cd "$(dirname "$0")" && pwd)
tool=$here/check-photo-metadata.py
work=$(mktemp -d "${TMPDIR:-.}/pm-test.XXXXXX") || exit 2
trap 'rm -rf "$work"' EXIT HUP INT TERM
fail=0

python3 -I - "$work" <<'PY' || exit 2
import struct, sys, zlib
w = sys.argv[1]

def seg(m, body):
    return b"\xff" + bytes([m]) + struct.pack(">H", len(body) + 2) + body

def jpeg(extra):
    sof = seg(0xC0, b"\x08\x00\x02\x00\x02\x01\x01\x11\x00")
    dqt = seg(0xDB, b"\x00" + bytes(range(1, 65)))
    dht = seg(0xC4, b"\x00" + b"\x01" + bytes(15) + b"\x00")
    sos = seg(0xDA, b"\x01\x01\x00\x00\x3f\x00")
    scan = b"\x12\xff\x00\x34\xff\xd0\x56"  # stuffed FF00 and an RST marker
    jfif = seg(0xE0, b"JFIF\0\x01\x01\x00\x00\x01\x00\x01\x00\x00")
    return b"\xff\xd8" + jfif + extra + dqt + sof + dht + sos + scan + b"\xff\xd9"

def chunk(t, d):
    return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

def png(extra):
    ihdr = chunk(b"IHDR", struct.pack(">IIBBBBB", 2, 2, 8, 0, 0, 0, 0))
    idat = chunk(b"IDAT", zlib.compress(b"\x00\x10\x20\x00\x30\x40"))
    return (b"\x89PNG\r\n\x1a\n" + ihdr + chunk(b"gAMA", b"\0\0\xb1\x8f") + extra + idat
            + chunk(b"IEND", b""))

exif = seg(0xE1, b"Exif\0\0SerialNumber=ABC123")
xmp = seg(0xE1, b"http://ns.adobe.com/xap/1.0/\0<x:xmpmeta owner='someone'/>")
iptc = seg(0xED, b"Photoshop 3.0\0" + b"8BIM")
icc = seg(0xE2, b"ICC_PROFILE\0\x01\x01profile")
files = {
    "clean.jpg": jpeg(icc),
    "exif.jpg": jpeg(exif + icc),
    "xmp-iptc-com.jpg": jpeg(xmp + iptc + seg(0xFE, b"host=lab-pc")),
    "clean.png": png(b""),
    "text.png": png(chunk(b"tEXt", b"Author\0someone") + chunk(b"iTXt", b"XML:com.adobe.xmp\0\0\0\0\0<x/>")
                    + chunk(b"zTXt", b"Comment\0\0" + zlib.compress(b"serial")) + chunk(b"eXIf", b"MM\0*")),
}
for n, d in files.items():
    open(w + "/" + n, "wb").write(d)
open(w + "/bad.jpg", "wb").write(b"\xff\xd8\xff\xe1\xff\xff")
PY

ok() { printf 'ok   %s\n' "$1"; }
no() { printf 'FAIL %s\n' "$1"; fail=1; }
run() { python3 -I "$tool" "$@"; }

# check hits
out=$(run check "$work/exif.jpg"); rc=$?
[ $rc -eq 1 ] && ok "check exif.jpg exits 1" || no "check exif.jpg exits $rc"
case $out in "meta $work/exif.jpg: APP1 EXIF") ok "exif line";; *) no "exif line: $out";; esac

out=$(run check "$work/xmp-iptc-com.jpg"); rc=$?
[ $rc -eq 1 ] && ok "check xmp/iptc/com exits 1" || no "check xmp/iptc/com exits $rc"
for w in "APP1 XMP" "APP13 IPTC" "COM comment"; do
    case $out in *"$w"*) ok "reports $w";; *) no "missing $w in: $out";; esac
done

out=$(run check "$work/text.png"); rc=$?
[ $rc -eq 1 ] && ok "check text.png exits 1" || no "check text.png exits $rc"
for w in tEXt iTXt zTXt eXIf; do
    case $out in *"text.png: $w "*) ok "reports $w";; *) no "missing $w in: $out";; esac
done

# check clean
out=$(run check "$work/clean.jpg" "$work/clean.png"); rc=$?
if [ $rc -eq 0 ] && [ -z "$out" ]; then ok "clean fixtures exit 0, no output"; else no "clean: rc=$rc out=$out"; fi

# usage and input errors
run >/dev/null 2>&1; rc=$?
[ $rc -eq 2 ] && ok "no args exits 2" || no "no args exits $rc"
run strip only-one >/dev/null 2>&1; rc=$?
[ $rc -eq 2 ] && ok "strip with one arg exits 2" || no "strip usage exits $rc"
run check "$work/bad.jpg" >/dev/null 2>&1; rc=$?
[ $rc -eq 2 ] && ok "malformed jpeg exits 2" || no "malformed exits $rc"
run check "$work/missing.png" >/dev/null 2>&1; rc=$?
[ $rc -eq 2 ] && ok "missing file exits 2" || no "missing exits $rc"

# strip, then check clean
for f in exif.jpg xmp-iptc-com.jpg text.png clean.jpg; do
    if run strip "$work/$f" "$work/out-$f" && run check "$work/out-$f" >/dev/null; then
        ok "strip $f then check clean"
    else
        no "strip $f then check clean"
    fi
done

# pixel data unchanged
if python3 -I - "$work" <<'PY'
import sys, struct
w = sys.argv[1]
def rd(n): return open(w + "/" + n, "rb").read()
def jpeg_scan(d):
    return d[d.index(b"\xff\xda"):d.rindex(b"\xff\xd9")]
def png_idat(d):
    i, out = 8, b""
    while i < len(d):
        ln, t = struct.unpack(">I4s", d[i:i+8])
        if t == b"IDAT":
            out += d[i+8:i+8+ln]
        i += 12 + ln
    return out
for n in ("exif.jpg", "xmp-iptc-com.jpg", "clean.jpg"):
    assert jpeg_scan(rd(n)) == jpeg_scan(rd("out-" + n)), n
assert png_idat(rd("text.png")) == png_idat(rd("out-text.png"))
assert rd("clean.jpg") == rd("out-clean.jpg")  # nothing to strip: identical bytes
assert b"SerialNumber" not in rd("out-exif.jpg")
assert b"someone" not in rd("out-text.png")
PY
then ok "pixel data unchanged, metadata bytes gone"; else no "pixel data or metadata bytes"; fi

if [ $fail -eq 0 ]; then echo PASS; else echo FAIL; fi
exit $fail
