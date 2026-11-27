# kms-dump-diff

`kms-dump-diff.py` compares two text dumps of the same kind and reports the
drift, so a new dated board dump can be checked against an older one.

```
python3 -I scripts/kms-dump-diff.py [--markdown] [--kind probe|props] OLD NEW
```

Python 3 standard library only. File paths come only from the arguments; the
script never reads a default location.

## Input kinds

- **probe**: output of the `probe` example, with or without `--raw`. Read are
  the `output`/`plane ... outputs ... zpos` summary lines, their `formats`
  lines, and, with `--raw`, the `connector`/`crtc`/`plane ... type ... possible
  crtcs` lines, `IN_FORMATS (n):` lines and the property lines of planes.
- **props**: output of the `props` example: `crtc N (index I)`,
  `connector N`, `plane N: possible_crtcs ...`, property lines
  `name (id N): ... = value` and the decoded `IN_FORMATS` modifier lines.

The kind is detected from the content; both files must be of the same kind
(`--kind` forces it). Comparing a plain probe with a `--raw` one reports the
raw-only sections as added or removed, so compare like with like. The format
list of the plain probe summary is cut at 32 pairs by the probe itself and is
reported separately as `format (summary)`.

## Report

In sorted order (planes removed/added by id, then per-plane changes by plane
id, then connectors, then CRTCs):

- plane added / removed;
- per plane: type, `outputs` / `possible crtcs` / `possible_crtcs` (reach),
  zpos range, the `rotation` property, scaling properties (any name containing
  `scal`), and `IN_FORMATS` format:modifier pairs added or removed (a format
  without a modifier is `FOURCC:0x0`);
- connector and CRTC added / removed (CRTCs also from `output N: crtc ID`).

Lines the parser does not know are counted and listed as `unparsed: N`
followed by `old:LINE:` / `new:LINE:` entries. They do not change the exit
status. A recorded dump may fold identical format lists as
`(identical to plane N)`; that is resolved.

`--markdown` prints the same items as a bullet list under a heading.

## Exit status

0 nothing differs, 1 something differs, 2 usage error (wrong arguments,
unreadable file, different kinds, undetectable kind).

## Test

`bash scripts/kms-dump-diff.test.sh` uses the small hand-written fixtures in
`scripts/testdata/kms-dump-diff/`; each states in a comment which source and
date its line formats come from.
