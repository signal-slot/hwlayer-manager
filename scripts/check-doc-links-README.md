# check-doc-links.sh

Finds relative links in the repository's Markdown that point at a file that
does not exist. POSIX `sh`, needs `git`, `awk`, `sed`, `grep`.

    scripts/check-doc-links.sh             # missing files only
    scripts/check-doc-links.sh --anchors   # also check #fragments

It takes no file argument. It always scans every tracked `*.md` file of the
whole repository (`git ls-files '*.md'`), and can be run from any
subdirectory of the work tree. Untracked files are not scanned.

## What counts as a link

- Inline links `[text](target)`, including `<target with spaces>` and an
  optional `"title"`.
- Reference definitions `[label]: target`.
- Skipped: `http://`, `https://` and `mailto:` targets, links inside fenced
  code blocks (`` ``` `` or `~~~`) and inline code spans.
- A `?query` is dropped, `%20` is read as a space, and a target starting with
  `/` is taken from the repository root. A target may be a file or a
  directory; it only has to exist.
- Pure `#anchor` links are ignored unless `--anchors` is given.

## `--anchors`

For a link with a `#fragment` (including a pure `#anchor` link to the same
file) whose target is a `.md` file, the fragment is compared with the slugs of
that file's headings. Lowercased fragments are matched exactly. Headings in
fenced code blocks are ignored. A heading becomes a slug the GitHub way:

1. strip the leading `#`s and any trailing `#`s;
2. remove backticks, and the `(url)` part of links, and square brackets;
3. lowercase;
4. delete everything except letters, digits, spaces, `_` and `-`;
5. turn spaces into `-`;
6. for a repeated slug, append `-1`, `-2`, … to the later ones.

Example: `## Hello, World! (v2)` gives `hello-world-v2`. Fragments into
non-Markdown files are not checked.

## Output and exit codes

Each problem prints one line, `file:line: missing target <target>` or
(with `--anchors`) `file:line: missing anchor <target>`.

| exit | meaning |
|---|---|
| 0 | no problems |
| 1 | at least one missing target or anchor (all are listed) |
| 2 | bad option, or not run inside a git work tree |

## Self-test

`sh scripts/check-doc-links.test.sh` builds a throwaway git repository under
`$TMPDIR` and checks missing inline and reference links, code and fence
skipping, `<…>` and `%20` targets, the anchor rules (punctuation, duplicate
headings, self links), exit 2 on a bad option, and running from a
subdirectory. It never touches the real repository and prints
`all tests passed` on success.
