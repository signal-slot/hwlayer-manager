# check-photo-metadata

Host checker for metadata inside committed photographs (board reports commit
camera pictures, e.g. `docs/m1-imx95/pictures`). `check-no-board-secrets.sh`
scans only added text lines, so EXIF/XMP/IPTC in a JPEG or text/EXIF chunks in
a PNG are unchecked there. They can carry camera or device serial numbers, owner
names, host names or GPS. Python 3, standard library only, no network.

```
python3 -I scripts/check-photo-metadata.py check [PATH...]
python3 -I scripts/check-photo-metadata.py strip IN OUT
sh scripts/check-photo-metadata.test.sh
```

Exit status: 0 = clean, 1 = metadata found, 2 = usage or input error
(including a PATH that is not a parseable JPEG or PNG).

## check

Without PATH it takes every `*.jpg`, `*.jpeg` and `*.png` tracked by git
(`git ls-files`). A tracked file of that name that is not a JPEG or PNG at all
(for example a text placeholder) is skipped with a `note` on stderr. For each
metadata block it prints

```
meta <file>: <segment or chunk> <short reason>
```

Kept, because they are needed to display the image:

- JPEG: SOI, DQT, SOF, DHT, SOS and scan data, restart markers, EOI, JFIF APP0,
  ICC profile APP2, Adobe APP14 (colour transform flag).
- PNG: IHDR, PLTE, IDAT, IEND, gAMA, cHRM, sRGB, iCCP, pHYs, plus tRNS (alpha)
  and the APNG chunks acTL, fcTL, fdAT.

Reported: every other JPEG APPn (EXIF and XMP in APP1, IPTC in APP13, MPF, ...),
COM comments, bytes after EOI; every other PNG chunk (tEXt, iTXt, zTXt, eXIf,
tIME, bKGD, sBIT, unknown ancillary ones), bytes after IEND. PNG chunk CRCs are
verified.

## strip

Writes OUT without the reported blocks. Kept segments are copied byte for byte
(no decoding, no re-encoding), so the scan data / IDAT bytes are identical. OUT
is parsed again and checked to have no metadata and the same pixel bytes before
it is written. Review the result before replacing a committed picture: stripping
changes the file, so any checksum of the picture in a report must be updated.

## Limits

It looks at container structure only. It does not read pixels, so a serial
number visible in the photograph itself is not found, and it does not look inside
JFIF thumbnails or an ICC profile's text.
