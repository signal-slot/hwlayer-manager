# host-demo

The WP5 host-only demo of the composition engine (SoW WP5). It builds the
workspace, runs the fake-backend scene tests and the `plan_time` example,
runs the two `thumbv7em-none-eabihf` builds of `docs/ci.md`, and prints a
summary with the date and git revision. POSIX sh.

**Its output is a host result and never a hardware result.** It says nothing
about any board, display, scanout or latch timing. The header and footer lines
it prints say so, and the `plan_time` timing is host timing, not hardware.

    sh scripts/host-demo.sh
    sh scripts/host-demo.sh -h | --help    # prints the usage comment, runs nothing

The script parses one option, `-h` / `--help`, and only as the first argument.
Any other argument is ignored; there are no other flags. The only environment
variable it uses is `TMPDIR` (location of the scratch directory, default
`/tmp`). It works from the repository root whatever the current directory is.

## What it runs

In order, stopping at the first failing step (`set -e`):

1. `cargo build --locked --workspace`
2. `cargo test --locked -p composition-fake --test advise_scenes`
3. `cargo test --locked -p composition-fake --test engine`
4. `cargo test --locked -p composition-fake --test event_order`
5. `cargo run --locked -q --release -p composition-fake --example plan_time 50`
   (first 8 lines shown, then the total line count)
6. `cargo build --locked -p composition-core --target thumbv7em-none-eabihf`
7. `cargo build --locked -p composition-ffi-staticlib --no-default-features --features panic-handler --target thumbv7em-none-eabihf --release`

Before step 1 it checks that the thumbv7em target is installed. Nothing is
installed for you; if the target is missing it prints
`rustup target add thumbv7em-none-eabihf` and exits 1 without running anything.
It writes only under the cargo target directory and a temporary directory that
is removed on exit.

## What it prints

- A header: `host demo: HOST RUN, NOT A HARDWARE RESULT`, then `date:` (UTC),
  `rev:` (12-character git revision, with `(+uncommitted changes)` if the tree
  differs from HEAD, or `unknown` outside git) and `rustc:`.
- Per step: `==> name: command`, the command's own output, and `PASS  name`.
- A footer: `summary: N steps passed, 0 failed`, `date:`, `rev:` and
  `This was a host run. It says nothing about any board or display.`

A passing run prints 7 `PASS` lines.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | all 7 steps passed (or `--help`) |
| 1 | the thumbv7em target is not installed; nothing was run |
| other non-zero | the exit status of the first failing command; later steps are not run and no summary is printed |

## Test

    bash scripts/host-demo.test.sh

Uses stub `cargo`, `rustup`, `rustc` and `git`, so nothing is built and it runs
in well under a second. It checks the usage output, the missing-target failure,
the 7 `PASS` lines, the summary, revision and date lines, the invoked cargo
commands, the "not a hardware result" wording, and that a failing step stops
the run. A real full run needs cargo and the thumbv7em target and takes
minutes; that is not covered by the test.

## See also

- [docs/host-demo.md](../docs/host-demo.md): what the demo does and does not show
- [docs/wp5-evidence.md](../docs/wp5-evidence.md): the WP5 evidence checklist
- [docs/ci.md](../docs/ci.md): the CI checks the builds mirror
