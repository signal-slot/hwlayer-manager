# scrub-kms-dump

`scripts/scrub-kms-dump.py` removes blob contents and board identifiers from a
`props` or `probe` dump (see `crates/composition-drm/examples/props.rs`)
before the dump is committed, e.g. under `crates/composition-drm/tests/data/`.
Python 3, standard library only.

## Use

```
props /dev/dri/cardN > props.txt                       # on the board
python3 scripts/scrub-kms-dump.py [--hostname NAME[,NAME...]] props.txt > props.scrubbed.txt
```

Without a file argument it reads stdin. `--hostname` may be repeated or take a
comma-separated list; each name is replaced as a whole token, case-insensitive.
Exit 0 on success, 2 on a usage error or an unreadable file. A summary of the
replacements goes to stderr.

## Replacements

| What | Replaced by |
| --- | --- |
| lines of hex bytes only (EDID and other blob dumps, 16 hex digits or more per line); a run of them becomes one line | `<blob N bytes removed>` |
| a run of 16 or more hex byte pairs inside a line | `<blob N bytes removed>` |
| MAC addresses (six groups, one separator) | `<mac>` |
| IPv6 and IPv4 addresses | `<ipv6>`, `<ipv4>` |
| the value after `serial`, `Serial number` and similar | `<serial>` |
| `/dev/tty*` (not `/dev/ttyRPMSG*`) and `/dev/serial/by-id/*` | `<tty-path>` |
| `user@host` (not git@github.com, noreply, example addresses) | `<user@host>` |
| the value of `password=`, `passwd=`, `token=`, `secret=`, `api_key=` | `<redacted>` |
| names given with `--hostname` | `<host>` |

Everything else is kept byte-identical, including line endings.

## Limits

- The `props` example prints only the blob id of `EDID` and `PATH`, not the
  contents; the blob rules matter for dumps from other tools. Its decoded
  `IN_FORMATS` lines are kept.
- Host names are only found when listed with `--hostname`. Read the scrubbed
  file before committing; the script lowers the risk, it does not prove a dump
  clean. `scripts/check-no-board-secrets.sh` is the project check.
- Any dotted quad is replaced, also a four-part version number.

## Self-test

`scripts/scrub-kms-dump.test.sh` scrubs `scripts/testdata/scrub-kms-dump/input.txt`,
compares it with `expected.txt` and applies the grep patterns of
`check-no-board-secrets.sh` to the result. **The fixture is made up** (RFC 5737
and RFC 3849 addresses, locally administered MACs, invented names); its lines
carry the `secrets-ok` marker so the project check accepts them.
