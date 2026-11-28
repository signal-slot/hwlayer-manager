# profile-check-expect

Judges the `CHECK ... agree=` lines printed by the `profile_check` example,
so a board run gets a mechanical verdict. Python 3, standard library only.

```sh
cargo run -p composition-drm --example profile_check ... | python3 -I scripts/profile-check-expect.py -
python3 -I scripts/profile-check-expect.py run.log --allow planner-stricter
python3 -I scripts/profile-check-expect.py run.log --markdown
```

Options:

- `--allow VALUE[,VALUE...]` tolerates those disagreement values
  (`no`, `planner-stricter`, `planner-looser`).
- `--allow-disagree N` tolerates N further disallowed disagreements (default 0).
- `--markdown` prints a table of the counts (and of the disallowed lines).

## Line format

```
CHECK <name> planner=<plan:planes|refuse:reasons> test_only=<ok|errno|not reached|error> agree=<yes|no|planner-stricter|planner-looser> via=<plan|forced|none> candidates=N [note=...]
```

`agree=yes` is agreement. `planner-stricter` (planner refused, kernel accepts),
`planner-looser` (planner plans, kernel refuses) and `no` are disagreements.
Other lines (including `AGREEMENT`) are skipped, so mixed logs work. The CHECK
lines carry no profile name; run once per profile.

## Verdict and exit status

The summary ends with one `VERDICT: PASS` or `VERDICT: FAIL (reasons)` line.

| exit | meaning |
|---|---|
| 0 | PASS |
| 1 | FAIL: more disallowed disagreements than allowed, or no CHECK line |
| 2 | usage or input error (bad option or value, unreadable file) |

Self-test: `bash scripts/profile-check-expect.test.sh` (sample lines inline).
