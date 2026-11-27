# board-run-bundle.sh

Runs a board program and stores its output as dated, scrubbed board data in the
format `scripts/check-test-data.sh` expects.

```
scripts/board-run-bundle.sh --soc rk3588|bcm2711|bcm2712 \
    --kind sweep|verify|caps [--name NAME] [--hostname NAME]... --out DIR -- CMD...
```

1. Runs `CMD` (stdin closed) and captures stdout and stderr together.
2. Writes `DIR/<kind>-<YYYY-MM-DD>-<name>.txt` (`NAME` defaults to the SoC).
   The first lines are the header:

   ```
   # observed
   # date: 2026-11-28
   # kernel: <uname -r>
   # command: <CMD with host-like arguments replaced by <host>>
   ```

   Arguments replaced by `<host>`: `user@host`, IPv4 and IPv6 addresses, MAC
   addresses, dotted host names (one or more dots; bare file names ending in
   `.sh .py .txt .md .rs` are kept), `/dev/tty*` and
   `/dev/serial/*` paths, also as the value of `--opt=value`.
   Plain host names (no dot) cannot be recognised: pass each one with the
   repeatable `--hostname NAME`; it is replaced by `<host>` everywhere in the
   file, header and output (passed on to `scrub-kms-dump.py --hostname`).
3. Runs `python3 -I scripts/scrub-kms-dump.py` over the file (header included).
   If the scrub fails, no file is written.
4. Runs `scripts/check-no-board-secrets.sh` on the file. That script only scans
   git diffs, so the file is committed to a throwaway repository under
   `$TMPDIR` (after an empty base commit, which is passed to the checker
   explicitly) and scanned there.
5. Prints `suggested README line: - ...` on stdout. It never writes the board
   folder's `README.md`; paste the line there yourself (check-test-data.sh
   requires every file to be named in it).

It does nothing else on the board: no service handling, no configuration
changes. Stop and restart any display manager yourself, per the board rules.

Exit status: 0 ok; 1 scrub or secrets check failed; 2 usage error; 3 `CMD`
failed (the file is still written and checked).

Environment (for tests): `BOARD_RUN_BUNDLE_UNAME` replaces `uname -r`,
`BOARD_RUN_BUNDLE_DATE` replaces today's date, `BOARD_RUN_BUNDLE_SCRUB` and
`BOARD_RUN_BUNDLE_SECRETS` replace the scripts used.

Test: `sh scripts/board-run-bundle.test.sh` (fake CMD in
`scripts/testdata/board-run-bundle/`).
