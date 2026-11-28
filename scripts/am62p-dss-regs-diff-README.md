# am62p-dss-regs-diff

Host tool: diffs two saved AM62P DSS snapshots, for example the state before
and after a scene. It parses text only and makes no hardware claim.

```
python3 -I scripts/am62p-dss-regs-diff.py [--json] BEFORE AFTER
sh scripts/am62p-dss-regs-diff.test.sh        # self-test
```

Both files are read with the parser of `scripts/am62p-dss-regs.py` (loaded
with importlib from this script's directory; nothing is duplicated). See
`scripts/am62p-dss-regs-README.md` for the input formats. Both files must have
the same format.

## Output

Markdown by default: one table of changed plane fields and one of changed
video-port fields (`plane | field | before | after`); `(no change)` if a
section is empty. `--json` prints `{"format", "planes", "vps"}`, each entry
`{"id", "name", "fields": {field: {"before", "after"}}}` plus
`"presence": "added"|"removed"` if the object exists in only one file.
Markdown values are shown as the parser's table shows them (`n/a` for a
field that is absent).

Compared fields:

| format | planes | video ports |
|---|---|---|
| `regdump` | enabled, format code, size, position, zorder, vp, layer enabled, alpha | enable, GO bit, size |
| `drm-state` | enabled, crtc, fb, format, size, position, zorder, alpha | enable, active, mode, plane mask, GO bit |

The GO bit exists only in `regdump`; `drm-state` has no GO bit, so it never
shows as changed there.

## Exit status

0 on success, whether or not anything changed. 2 for an unreadable file, an
unknown or mixed format, or a usage error (nothing on stdout).
