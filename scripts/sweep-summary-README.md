# sweep-summary.py

Turns text lines from board runs into markdown tables (or JSON with `--json`).
Python 3, standard library only. Exit 0 ok, 2 input error (unreadable file).

    python3 -I scripts/sweep-summary.py run1.txt run2.txt   # or stdin
    python3 -I scripts/sweep-summary.py --json run.txt

Lines starting with `#` and unknown lines are ignored. Recognised lines:

| line | from | table |
|---|---|---|
| `RESULT plane=… kind=… max_upscale_h=…` | `scale_probe` | scale limits per plane |
| `COMBO k=… planes=… scale=… result=…`, `COMBO_MAX accepted_k=…` | `kms_sweep` | results per k, maximum |
| `MODIFIER plane=… format=… modifier=… listed=… addfb=… test_only=…` | `kms_sweep` | modifier acceptance; `flag` column marks `listed=yes` but refused, and `listed=no` but accepted |
| `REPLACE second=… events=…` | `kms_sweep` | pending-commit replacement |
| `CHECK <name> planner=… test_only=… agree=…`, `AGREEMENT total=…` | `profile_check` | agreement; `planner-looser` rows first, then `planner-stricter`, `no`, others |

Values may contain spaces (`test_only=not reached`); a field runs to the next
` key=`. Self-test: `scripts/sweep-summary.test.sh`; fixtures in
`scripts/testdata/sweep-summary/` are synthetic, not board data.
