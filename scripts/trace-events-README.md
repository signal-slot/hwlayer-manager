# trace-events.sh

Reports which X-03 event kinds appear in CETRACE trace logs
(`docs/trace-format.md`). `scripts/check-trace-log.sh` checks only headers;
this script counts the `ev=` kinds, so a board results page can state which
of `validate`, `commit`, `latched`, `released` and `dropped` were seen.

    sh scripts/trace-events.sh [--require <kind,...>] <log>...

Output, one line per log:

    <file>: validate=N commit=N latched=N released=N dropped=N other=N

`other` counts every other `ev=` name (`hdr`, `check`, `latch_timeout`, ...).
Then two lines over all logs together:

    SEEN validate,commit,...
    MISSING released,dropped        (or: MISSING none)

Lines that do not start with `CETRACE` are ignored. CRLF line endings are
accepted. `SEEN none` is printed when no kind was seen.

## Exit codes

- 0: ok
- 1: `--require` was given and one of those kinds is missing in all logs
- 2: usage error (also an unknown kind in `--require`) or unreadable file

## Test

    TMPDIR=<dir> sh scripts/trace-events.test.sh

Fixtures are generated inline in a temporary directory under `$TMPDIR`,
which is removed on exit. The script is POSIX sh plus awk and is not part of
`scripts/check-all.sh`.
