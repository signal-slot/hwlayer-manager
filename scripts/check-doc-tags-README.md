# check-doc-tags.sh

Lints the evidence tags, dates and source pins in the hardware docs. POSIX
`sh` and `awk`, no other dependencies. It is a text check only: it cannot tell
whether a claim is true, only whether the claim carries the markers the rule
asks for. The rule itself is in `CONTRIBUTING.md`, section "Measurements and
hardware claims": a claim is a dated measurement on the named hardware or is
marked *not measured*; the tags *observed* (seen on a device, with the date),
*read from source* (with source and revision) and *inferred* (reasoned, not
seen) stay honest.

    scripts/check-doc-tags.sh                 # docs/imx95-*.md
    scripts/check-doc-tags.sh docs/a.md b.md  # named files only

## What it scans

With no arguments: every `docs/imx95-*.md` of the repository the script lives
in. With arguments: exactly those files (any path). A path that is not a file
is a usage error, not a finding. Other docs are not scanned unless named.

## What it reports

Each finding is printed as `file:line: reason`. Three checks per file:

| reason | what triggers it |
|---|---|
| `no evidence tag` | the file contains none of `observed`, `read from source`, `inferred` (any case) anywhere. Reported at line 1 |
| `observed tag without a YYYY-MM-DD date` | a line uses *observed* as a tag and neither it, nor an earlier line in the same heading section, has a `YYYY-MM-DD` date. A heading line holding a date counts for the section below it; a new heading resets the date |
| `git-host URL without a 40-hex SHA or tag ref nearby` | an `https` URL to `github.com`, `git.toradex.com`, `codeload.github.com` or `raw.githubusercontent.com` has no 40-hex commit SHA and no tag-looking ref (`v1.2`, `name-1.2`, `refs/tags/`, `/releases/tag/`) on its line or within two lines either side |

"Used as a tag" means *observed* written as `[observed]`, `(observed`,
`**observed**`, `` `observed` ``, `| observed |`, or `observed:` at the start
of a line. The word in ordinary prose is not a tag and is not checked. Only
*observed* needs a date for the check; *read from source* and *inferred* need
none, but a source claim needs its pin (the URL rule).

## Fixing a reported line honestly

Never invent a date, a SHA or a measurement to silence the check.

- Undated *observed*: add the date the observation was really made, taken
  from the snapshot or log it came from. If you do not know it, the line is
  not an observation: re-tag it *inferred*, or write *not measured*.
- A line with no source behind it: tag it *inferred*.
- URL without a pin: replace the branch (`main`, `?h=branch`) with the commit
  or tag you actually read, and say which. If you did not read a pinned
  revision, do not add one; read the source first, or tag the claim
  *inferred*.
- File with no tag: add tags where claims are made, using the three words
  above; do not append an empty "tags" line.
- Host, CI and vkms runs are never *observed* for hardware behaviour.

## Exit codes

| code | meaning |
|---|---|
| 0 | no findings |
| 1 | at least one finding (all are printed) |
| 2 | usage error: an option (`-h` included), a named path that is not a file, or no `docs/imx95-*.md` found |

## Self-test

    sh scripts/check-doc-tags.test.sh

Builds small fixture files under `$TMPDIR` and removes them. The fixtures are
synthetic text, not board data. The check is listed in `CONTRIBUTING.md` under
local checks.
