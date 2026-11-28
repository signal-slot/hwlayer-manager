# check-feasibility-doc.sh

Mechanical check of `docs/imx95-zephyr-feasibility.md`. POSIX `sh` and `awk`
only. It checks form, not truth: it cannot tell whether a tag is honest or a
date is right.

    scripts/check-feasibility-doc.sh            # checks docs/imx95-zephyr-feasibility.md
    scripts/check-feasibility-doc.sh FILE       # checks another file

## What it requires

1. The file exists.
2. Each of the three evidence tags appears at least once, in any case:
   `observed`, `read from source`, `inferred`.
3. At least one date of the form `YYYY-MM-DD` appears.
4. Every `http(s)` URL to a git host is pinned. Hosts: github.com,
   githubusercontent.com (also `raw.` and `codeload.`), git.toradex.com,
   source.codeaurora.org, git.codelinaro.org, git.kernel.org, gitlab.com,
   googlesource.com, bitbucket.org. The URL, or the same line, must contain a
   commit SHA or a tag.
   - SHA: 7 to 40 lowercase hex characters with at least one digit.
   - Tag: `v1.2`, `name-1.2` (or `_`), `refs/tags/`, or `/releases/tag/`.

The tag and date checks are whole-file: one match anywhere is enough. They do
not check that each observation carries its own tag or date. Only the URL check
is per line.

## Output and exit codes

Prints `ok`, or one line per finding:

    fail <line>: <reason>     # line 0 = the whole file

| exit | meaning |
|---|---|
| 0 | pass (`ok`) |
| 1 | at least one `fail` line, or the file was not found |
| 2 | usage error (an option such as `--bogus`, or more than one argument) |

## Self-test

    bash scripts/check-feasibility-doc.test.sh

It runs the script on synthetic files in `scripts/testdata/check-feasibility-doc/`
(good, upper-case tags, a missing tag, no date, unpinned URLs, a missing file)
and checks the exit codes and messages, then prints `all tests passed`. The
fixtures are not board data.
