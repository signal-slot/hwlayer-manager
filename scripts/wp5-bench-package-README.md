# wp5-bench-package.sh

Host benchmark package from a clean checkout (SoW §8 WP5, roadmap item). Every
file it writes is labelled `host only, not a hardware result`.

    sh scripts/wp5-bench-package.sh [--commit REV] [--dry-run] OUTDIR

It clones the current repository into a scratch directory under `$TMPDIR`
(refused if `$TMPDIR` is unset or under `/tmp` or `/var/tmp`), checks out `HEAD`
or `REV`, runs `cargo fetch --locked`, then runs the `composition-fake` examples
`plan_bench`, `plan_time` and `advise_bench` (only those that exist at that
commit) with `cargo run --release --locked`. The scratch directory, including the
target directory, is removed on exit.

Output in `OUTDIR`: `<example>.txt` per example run, and `manifest.txt` with
commit, UTC date, `rustc -V`, `cargo -V`, the CPU model name from
`/proc/cpuinfo`, `dry_run` and the examples run. No host name or IP is written.

`--dry-run` clones and writes the manifest but skips the cargo runs.
Not part of `check-all.sh` or CI. Test: `sh scripts/wp5-bench-package.test.sh`
(dry run, plus one real run if cargo is available).
