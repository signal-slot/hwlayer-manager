#!/usr/bin/env python3
"""check-photo-metadata: find and strip metadata blocks in JPEG and PNG pictures.

Board reports commit camera photographs. scripts/check-no-board-secrets.sh scans
only added text lines, so EXIF/XMP/IPTC (JPEG APPn, COM) and PNG text/eXIf/tIME
chunks are unchecked. They can carry serial numbers, owner names, host names, GPS.
Standard library only. No network access.

  check-photo-metadata.py check [PATH...]   (default: every tracked *.jpg *.jpeg *.png)
  check-photo-metadata.py strip IN OUT

Exit status: 0 = clean, 1 = metadata found, 2 = usage or input error.
See scripts/check-photo-metadata-README.md.
"""
import os
import struct
import subprocess
import sys
import zlib

PNG_SIG = b"\x89PNG\r\n\x1a\n"
# chunks needed to display the image (tRNS and the APNG frame chunks hold alpha/pixels)
PNG_KEEP = {b"IHDR", b"PLTE", b"IDAT", b"IEND", b"gAMA", b"cHRM", b"sRGB",
            b"iCCP", b"pHYs", b"tRNS", b"acTL", b"fcTL", b"fdAT"}
PNG_REASON = {
    b"tEXt": "text chunk", b"iTXt": "text chunk (XMP possible)",
    b"zTXt": "compressed text chunk", b"eXIf": "EXIF block",
    b"tIME": "modification time", b"bKGD": "background colour",
    b"sBIT": "significant bits", b"sPLT": "suggested palette",
    b"hIST": "histogram",
}
JPEG_PIXEL_MARKERS = (0xC0, 0xC1, 0xC2, 0xC4, 0xDA, 0xDB, "scan")


class PhotoError(Exception):
    pass


# ---------------------------------------------------------------- JPEG
def jpeg_segments(data):
    """Return [(start, end, marker, payload_start)] covering the whole JPEG.
    Entropy-coded data is a segment with marker "scan", bytes after EOI "trailer"."""
    if data[:2] != b"\xff\xd8":
        raise PhotoError("not a JPEG (no SOI)")
    segs = [(0, 2, 0xD8, 2)]
    i, n = 2, len(data)
    while i < n:
        if data[i] != 0xFF:
            raise PhotoError("JPEG: expected marker at offset %d" % i)
        j = i
        while j < n and data[j] == 0xFF:
            j += 1
        if j >= n:
            raise PhotoError("JPEG: truncated at marker")
        m = data[j]
        j += 1
        if m == 0xD9:
            segs.append((i, j, m, j))
            if j < n:
                segs.append((j, n, "trailer", j))
            return segs
        if m == 0x00 or m == 0x01 or 0xD0 <= m <= 0xD8:
            segs.append((i, j, m, j))
            i = j
            continue
        if j + 2 > n:
            raise PhotoError("JPEG: truncated segment length")
        ln = struct.unpack(">H", data[j:j + 2])[0]
        end = j + ln
        if ln < 2 or end > n:
            raise PhotoError("JPEG: bad segment length at offset %d" % i)
        segs.append((i, end, m, j + 2))
        i = end
        if m == 0xDA:  # entropy-coded data up to the next real marker
            k = i
            while k < n - 1:
                nxt = data[k + 1]
                if data[k] == 0xFF and nxt not in (0, 0xFF) and not 0xD0 <= nxt <= 0xD7:
                    break
                k += 1
            else:
                raise PhotoError("JPEG: no EOI after scan data")
            segs.append((i, k, "scan", i))
            i = k
    raise PhotoError("JPEG: no EOI")


def jpeg_meta(data, seg):
    """Return (name, reason) if the segment is metadata, else None."""
    s, e, m, p = seg
    if m == "trailer":
        return ("trailer", "%d bytes after EOI" % (e - s))
    if m == 0xFE:
        return ("COM", "comment")
    if not (isinstance(m, int) and 0xE0 <= m <= 0xEF):
        return None
    name = "APP%d" % (m - 0xE0)
    body = data[p:e]
    if m == 0xE0 and body.startswith(b"JFIF\0"):
        return None
    if m == 0xE2 and body.startswith(b"ICC_PROFILE\0"):
        return None
    if m == 0xEE and body.startswith(b"Adobe"):
        return None  # colour transform flag, needed to decode CMYK/YCCK
    if body.startswith(b"Exif\0"):
        why = "EXIF"
    elif body.startswith(b"http://ns.adobe.com/xap/1.0/"):
        why = "XMP"
    elif m == 0xED:
        why = "IPTC/Photoshop"
    elif m == 0xE0:
        why = "non-JFIF block"
    elif m == 0xE2:
        why = "non-ICC block (MPF/FlashPix)"
    else:
        why = "application block"
    return (name, why)


def jpeg_scan(data):
    out = []
    for s in jpeg_segments(data):
        h = jpeg_meta(data, s)
        if h:
            out.append((h, s))
    return out


def jpeg_strip(data):
    drop = {s[:2] for _, s in jpeg_scan(data)}
    return b"".join(data[s:e] for s, e, _, _ in jpeg_segments(data) if (s, e) not in drop)


def jpeg_pixels(data):
    return b"".join(data[s:e] for s, e, m, _ in jpeg_segments(data) if m in JPEG_PIXEL_MARKERS)


# ---------------------------------------------------------------- PNG
def png_chunks(data):
    if data[:8] != PNG_SIG:
        raise PhotoError("not a PNG (bad signature)")
    i, n, out = 8, len(data), []
    while True:
        if i + 12 > n:
            raise PhotoError("PNG: truncated chunk header")
        ln, typ = struct.unpack(">I4s", data[i:i + 8])
        end = i + 12 + ln
        if end > n:
            raise PhotoError("PNG: chunk %r runs past end of file" % typ)
        crc = struct.unpack(">I", data[end - 4:end])[0]
        if zlib.crc32(data[i + 4:end - 4]) & 0xFFFFFFFF != crc:
            raise PhotoError("PNG: bad CRC in chunk %r" % typ)
        out.append((i, end, typ))
        i = end
        if typ == b"IEND":
            if i < n:
                out.append((i, n, b"trailer"))
            return out


def png_meta(seg):
    s, e, typ = seg
    if typ == b"trailer":
        return ("trailer", "%d bytes after IEND" % (e - s))
    if typ in PNG_KEEP:
        return None
    return (typ.decode("latin-1"), PNG_REASON.get(typ, "ancillary chunk not needed to display"))


def png_scan(data):
    out = []
    for s in png_chunks(data):
        h = png_meta(s)
        if h:
            out.append((h, s))
    return out


def png_strip(data):
    drop = {s[:2] for _, s in png_scan(data)}
    return b"".join(data[s:e] for s, e, _ in png_chunks(data) if (s, e) not in drop)


def png_pixels(data):
    return b"".join(data[s + 8:e - 4] for s, e, t in png_chunks(data)
                    if t in (b"IHDR", b"PLTE", b"tRNS", b"IDAT", b"fdAT"))


# ---------------------------------------------------------------- driver
def kind(data, strict=False):
    if data[:2] == b"\xff\xd8":
        return "jpeg"
    if data[:8] == PNG_SIG:
        return "png"
    if strict:
        raise PhotoError("neither JPEG nor PNG")
    return None


def scan(data):
    return jpeg_scan(data) if kind(data, True) == "jpeg" else png_scan(data)


def strip(data):
    return jpeg_strip(data) if kind(data, True) == "jpeg" else png_strip(data)


def pixels(data):
    return jpeg_pixels(data) if kind(data, True) == "jpeg" else png_pixels(data)


def read(path):
    with open(path, "rb") as f:
        return f.read()


def tracked_pictures():
    try:
        top = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True,
                             text=True, check=True).stdout.strip()
        r = subprocess.run(["git", "-C", top, "ls-files", "-z"], capture_output=True, check=True)
    except (OSError, subprocess.CalledProcessError):
        raise PhotoError("git ls-files failed: give PATH arguments")
    names = [x.decode() for x in r.stdout.split(b"\0") if x]
    return [os.path.join(top, x) for x in names if x.lower().endswith((".jpg", ".jpeg", ".png"))]


def cmd_check(paths):
    default = not paths
    if default:
        paths = tracked_pictures()
    bad = err = 0
    for p in paths:
        try:
            if default and kind(read(p)[:8]) is None:
                print("note %s: not a JPEG or PNG, skipped" % p, file=sys.stderr)
                continue
            for (name, why), _ in scan(read(p)):
                print("meta %s: %s %s" % (p, name, why))
                bad += 1
        except (PhotoError, OSError) as ex:
            print("error %s: %s" % (p, ex), file=sys.stderr)
            err += 1
    if err:
        return 2
    return 1 if bad else 0


def cmd_strip(src, dst):
    data = read(src)
    out = strip(data)
    if scan(out) or pixels(out) != pixels(data):  # re-parsing also proves OUT is well formed
        raise PhotoError("strip verification failed")
    with open(dst, "wb") as f:
        f.write(out)
    return 0


def main(argv):
    try:
        if argv[:1] == ["check"]:
            return cmd_check(argv[1:])
        if argv[:1] == ["strip"] and len(argv) == 3:
            return cmd_strip(argv[1], argv[2])
    except (PhotoError, OSError) as ex:
        print("error: %s" % ex, file=sys.stderr)
        return 2
    print("usage: check-photo-metadata.py check [PATH...] | strip IN OUT", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
