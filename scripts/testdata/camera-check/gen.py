"""Synthetic pictures for scripts/camera-check.test.sh (stdlib only).
usage: gen.py OUT.(ppm|png) LAYOUT   LAYOUT = b-top | a-top | b-moved
200x100, panel = whole picture (use rotation 0, panel full). Not a camera capture."""
import struct, sys, zlib
RED, BLUE = (220, 20, 20), (20, 20, 220)
A = (0.10, 0.30, 0.60, 0.90, RED)
B = (0.40, 0.10, 0.90, 0.70, BLUE)
Bm = (0.45, 0.10, 0.95, 0.70, BLUE)
layers = {"b-top": [A, B], "a-top": [B, A], "b-moved": [A, Bm]}[sys.argv[2]]
w, h = 200, 100
rows = []
for y in range(h):
    row = b""
    for x in range(w):
        c = (0, 0, 0)
        for u0, v0, u1, v1, col in layers:
            if u0 * w <= x < u1 * w and v0 * h <= y < v1 * h:
                c = col
        row += bytes(c)
    rows.append(row)
out = sys.argv[1]
if out.endswith(".png"):
    def ch(t, b):
        return struct.pack(">I", len(b)) + t + b + struct.pack(">I", zlib.crc32(t + b) & 0xFFFFFFFF)
    raw = b"".join(b"\0" + r for r in rows)
    data = (b"\x89PNG\r\n\x1a\n" + ch(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
            + ch(b"IDAT", zlib.compress(raw)) + ch(b"IEND", b""))
else:
    data = b"P6\n%d %d\n255\n" % (w, h) + b"".join(rows)
open(out, "wb").write(data)
