# sweep-to-fixture.py

Turns `scale_probe` and `kms_sweep` output lines into Rust source: a
provenance comment and `const` arrays of structs. Tests paste the output into a
test file; code never reads `docs/` or the run file at runtime.
Python 3, standard library only.

    python3 -I scripts/sweep-to-fixture.py --source PATH --date YYYY-MM-DD [FILE...]

`--source` (the recorded file the lines came from) and `--date` (observed date)
are required. Without FILE (or with `-`) it reads stdin. Output starts with

    // source: <PATH>, observed <YYYY-MM-DD>

Exit codes: 0 ok; 1 a recognised line could not be parsed (each is named as
`file:line: cannot parse (reason): text` on stderr; nothing is printed) or no
recognised line at all; 2 usage error or unreadable input.

Line grammar is that of `sweep-summary.py` (`key=value` fields, a value may
contain spaces up to the next ` key=`). `#` lines, blank lines and other
keywords (`COMBO_MAX`, `CHECK`, `VERDICT`, `DATE`, …) are ignored.

| line | const | struct fields |
|---|---|---|
| `RESULT` | `SCALE_RESULTS: &[ScaleResult]` | `plane: u32`, `kind`, `format`, `max_upscale_h/v/both`, `downscale`, `min_downscale` (strings, ratios kept as text such as `"4.0000"` or `"none"`) |
| `COMBO` | `COMBOS: &[Combo]` | `k: u32`, `planes: &'static [u32]`, `scale`, `result` |
| `MODIFIER` | `MODIFIERS: &[ModifierResult]` | `plane: u32`, `format`, `modifier: u64`, `listed: bool` (`yes`/`no`), `addfb`, `test_only` |
| `REPLACE` | `REPLACES: &[Replace]` | `second`, `events: u32` |

An array and its struct are emitted only when the input has such lines. Items
carry `#[allow(dead_code)]`, so a test may use only some. Names are fixed, so
paste one run per module. `RESULT`, `COMBO`, `MODIFIER` and `REPLACE` lines must
carry every field listed; extra fields are ignored.

Test: `sh scripts/sweep-to-fixture.test.sh` (runs `rustfmt --check` on the
output when rustfmt exists). Fixtures in `scripts/testdata/sweep-to-fixture/`
are synthetic, not board data.
