#!/bin/sh
# check-no-board-secrets.sh - fail if a branch adds board addresses or credentials.
#
# Project rule: no board addresses, host names, IPs, serial numbers, UART
# paths or credentials in any file or commit message.
#
# Run (needs only git, grep and a POSIX sh):
#   scripts/check-no-board-secrets.sh [BASE_REF]      # see "Base" below
#   scripts/check-no-board-secrets.test.sh            # self-test
#
# Scans the ADDED lines of `git diff BASE_REF...HEAD` and every line of the
# commit messages of `git log BASE_REF..HEAD`. Existing content is not scanned.
#
# Base: an explicit BASE_REF is used as given and exit 2 if it does not exist.
# With no argument the first of origin/main, main, refs/remotes/origin/HEAD
# that resolves to a commit is used (a checkout without origin/main, such as
# a plain clone of a topic branch, still works). If none resolves, the scan
# falls back to a FULL scan: the diff base is git's empty tree (two-dot
# `git diff EMPTY HEAD`, since three-dot needs a commit), so every line of
# every tracked file counts as added, and the messages of all commits in
# `git rev-list HEAD` are scanned. The base or mode used is printed to stderr.
# Exit status: 0 clean, 1 hit(s) (printed as "kind: file:line: text"),
# 2 usage error.
#
# Patterns:
#   ipv4      a.b.c.d dotted quad
#   tty       /dev/tty* and /dev/serial/by-id paths
#   mac       xx:xx:xx:xx:xx:xx (also with '-')
#   user@host user@host, host starting with a letter
#   ssh-key   -----BEGIN ... PRIVATE KEY-----
#   secret    password=, passwd=, token=, secret=, api_key= followed by a value
#
# Allowlist (deliberate, kept small):
#   - IPv4: lines containing 0.0.0.0 or 127.0.0.1; any IPv4 in Cargo.lock or
#     under a vendor/ directory (dependency versions such as 1.2.3.4 look
#     like addresses).
#   - user@host: git@github.com, git@gitlab.com, noreply addresses
#     (e.g. noreply@anthropic.com) and Co-Authored-By/Signed-off-by lines.
#   - secret: values starting with $, < or { (placeholders).
#   - Any line containing the marker "secrets-ok" is ignored (use sparingly,
#     only for documented non-board examples such as RFC 5737 addresses).
#   - This script, its test (which also tests check-no-farm-ids.sh) and the
#     sibling scripts/check-no-farm-ids.sh are not scanned (they spell out
#     the patterns). Exact paths, no globs.
#   - scripts/testdata/ (the one directory exception): fake fixture data for
#     the self-tests of the scrubber scripts. Nothing else is excluded by
#     directory; other self-test files use the FIXTURES table below.
#
# False-positive rules added for the user@host pattern (each is narrow; see
# benign_sed and uh_line_ok below):
#   - A YAML step line "uses: owner/repo[/path]@ref" (GitHub Actions) is
#     skipped as a whole. A line that runs ssh to a host is not.
#   - "actions/NAME@vN" anywhere (prose in commit messages, for example).
#   - Literal placeholders: "user@host", "repo@commit", "repo@pin", "a@b.c", and any @example.com,
#     @example.org, @example.net, @example.invalid address.
#   - "name@commit" pins: @ followed only by hex digits ("imx-sm@af1e3702"),
#     device-tree unit addresses ("port@0/1", "phy@c"), the "@N" placeholder and
#     systemd instance names (getty, serial-getty, modprobe). These rules are
#     NOT applied to a line that mentions ssh, scp, sftp or rsync, so
#     an ssh target with a hex-only host name stays flagged.
#   - name@HOST for an exact public mailing-list or archive domain
#     (lists.freedesktop.org, vger.kernel.org, lore.kernel.org,
#     mail-archive.com, lists.infradead.org, lists.linaro.org,
#     lists.debian.org, lists.zephyrproject.org); not on ssh/scp/sftp/rsync
#     lines, and only the user@host check is affected.
#   - A kernel mailing-list archive URL (lore.kernel.*) whose message id
#     contains "@".
# Other false-positive rules:
#   - mac: all six groups must use the same separator, so a time range such as
#     "19:44:22-19:45:12" is not a MAC address.
#   - tty: /dev/ttyRPMSG* is the Linux rpmsg_tty driver node, not a UART.
#   - FIXTURES below: sample values that another script's self-test writes on
#     purpose, allowed per file and per text only.

# --c7 BASE_REF [PATH...]: opt-in, reproduces the plan's acceptance grep c7
#   git diff BASE_REF -- PATHS | grep -Ei '<c7 pattern>'
# over the added lines only. Default PATHS: README.md CONTRIBUTING.md
# docs/imx95-zephyr-feasibility.md docs/imx95-zephyr tests/board.
# Prints "fail <file>: <line>" per hit, "ok" when clean. Exit 0/1/2 as usual.
if [ "${1:-}" = "--c7" ]; then
    if [ "$#" -lt 2 ]; then
        echo "usage: $0 --c7 BASE_REF [PATH...]" >&2
        exit 2
    fi
    c7base=$2
    shift 2
    git rev-parse --verify --quiet "$c7base^{commit}" >/dev/null || {
        echo "check-no-board-secrets: unknown base ref: $c7base" >&2
        exit 2
    }
    if [ "$#" -eq 0 ]; then
        set -- README.md CONTRIBUTING.md docs/imx95-zephyr-feasibility.md docs/imx95-zephyr tests/board
    fi
    c7out=$(mktemp "${TMPDIR:-.}/nbs-c7.XXXXXX") || exit 2
    trap 'rm -f "$c7out"' EXIT HUP INT TERM
    git -c core.quotepath=off diff --no-color --no-ext-diff "$c7base" -- "$@" |
    while IFS= read -r l; do
        case $l in
        "+++ /dev/null") file= ;;
        "+++ b/"*) file=${l#"+++ b/"} ;;
        "+"*) printf '%s\t%s\n' "$file" "${l#+}" ;;
        esac
    done |
    grep -Ei '([0-9]{1,3}\.){3}[0-9]{1,3}|/dev/tty|password|@[a-z0-9-]+\.' |
    sed 's/^/fail /; s/\t/: /' >"$c7out"
    if [ -s "$c7out" ]; then
        cat "$c7out"
        exit 1
    fi
    echo ok
    exit 0
fi
if [ "$#" -gt 1 ]; then
    echo "usage: $0 [BASE_REF]" >&2
    exit 2
fi
tab=$(printf '\t')
full=0
if [ "$#" -eq 1 ]; then
    base=$1
    git rev-parse --verify --quiet "$base^{commit}" >/dev/null || {
        echo "check-no-board-secrets: unknown base ref: $base" >&2
        exit 2
    }
else
    base=
    for cand in origin/main main refs/remotes/origin/HEAD; do
        if git rev-parse --verify --quiet "$cand^{commit}" >/dev/null; then
            base=$cand
            break
        fi
    done
    if [ -z "$base" ]; then
        full=1
        base=$(git hash-object -t tree /dev/null) || exit 2
        echo "check-no-board-secrets: no base ref found (origin/main, main, origin/HEAD); full scan of HEAD" >&2
    else
        echo "check-no-board-secrets: no argument; using base $base" >&2
    fi
fi
if [ "$full" -eq 1 ]; then
    log_range=HEAD
    desc="full scan of HEAD"
else
    log_range="$base..HEAD"
    desc="$base...HEAD"
fi
added_diff() {
    if [ "$full" -eq 1 ]; then
        git -c core.quotepath=off diff -U0 --no-color --no-ext-diff "$base" HEAD
    else
        git -c core.quotepath=off diff -U0 --no-color --no-ext-diff "$base...HEAD"
    fi
}

work=$(mktemp -d "${TMPDIR:-.}/nbs.XXXXXX") || exit 2
trap 'rm -rf "$work"' EXIT HUP INT TERM
raw=$work/raw
stream=$work/stream
hits=$work/hits

# Stream format: "<where>:<line><TAB><text>"; where is a path or commit:<sha>.
# Added lines of the diff.
added_diff |
while IFS= read -r l; do
    case $l in
    "+++ /dev/null") file= ;;
    "+++ b/"*) file=${l#"+++ b/"} ;;
    "@@ "*)
        n=${l#*+}
        n=${n%% *}
        n=${n%%,*}
        ;;
    "+"*)
        case $file in
        scripts/check-no-board-secrets.sh | scripts/check-no-board-secrets.test.sh | \
            scripts/check-no-farm-ids.sh) ;;
        # scripts/testdata/ holds deliberately fake secrets (fake IPs, MACs,
        # UART paths) that the self-tests of the scrubbers must contain.
        # Excluded as one directory; patterns are not loosened. Never put
        # real data there.
        scripts/testdata/*) ;;
        *) printf '%s:%s\t%s\n' "$file" "$n" "${l#+}" ;;
        esac
        n=$((n + 1))
        ;;
    esac
done >"$raw"

# Commit messages.
for c in $(git rev-list "$log_range"); do
    s=$(git rev-parse --short "$c")
    git log -1 --format=%B "$c" |
    {
        i=1
        while IFS= read -r l; do
            printf 'commit:%s:%s\t%s\n' "$s" "$i" "$l"
            i=$((i + 1))
        done
    }
done >>"$raw"

grep -v "secrets-ok" "$raw" >"$stream"
: >"$hits"

# Fixture allowlist: "path|extended regex|reason". A stream line of exactly
# this file whose text matches the regex is dropped before every check. These
# are sample secrets that the Zephyr-app check's self-test must contain to
# prove that check rejects them. The same text in any other file is reported.
FIXTURES='scripts/check-zephyr-apps.test.sh|192\.168\.1\.5|private-range sample the Zephyr-app check must reject
scripts/check-zephyr-apps.test.sh|/dev/ttyUSB0|serial path the Zephyr-app check must reject
scripts/check-zephyr-apps.test.sh|Password=x|credential sample the Zephyr-app check must reject
scripts/board-run-bundle.test.sh|10\.9\.8\.7|fake board address the bundle scrubber must remove
scripts/board-run-bundle.test.sh|aa:bb:cc:dd:ee:ff|fake MAC the bundle scrubber must remove
scripts/check-m2-acceptance.test.sh|10\.20\.30\.40|fake address the M2 acceptance check must reject
scripts/check-m2-acceptance.test.sh|/dev/ttyUSB0|fake UART path the M2 acceptance check must reject
scripts/check-no-farm-ids.test.sh|ip="10\.1\.2\.3"|fake IPv4 the farm-id check must reject
scripts/check-no-farm-ids.test.sh|192\.0\.2\.7|RFC 5737 documentation range sample
scripts/check-no-farm-ids.test.sh|1\.2\.3\.4|lockfile version that must not be flagged
scripts/check-no-farm-ids.test.sh|password=changeme|placeholder credential sample
scripts/check-no-farm-ids.test.sh|let token = abc123|credential sample that must stay flagged
scripts/check-no-farm-ids.test.sh|dri-devel@lists|mailing-list address next to an ssh target sample'
printf '%s\n' "$FIXTURES" | while IFS='|' read -r fpath fre freason; do
    printf '%s\n' "$fpath" >>"$work/fx-paths"
    printf '%s\n' "$fre" >>"$work/fx-res"
done
n_fx=$(wc -l <"$work/fx-paths")
i=1
while [ "$i" -le "$n_fx" ]; do
    fpath=$(sed -n "${i}p" "$work/fx-paths")
    fre=$(sed -n "${i}p" "$work/fx-res")
    grep -vE "^${fpath}:[0-9]+${tab}.*(${fre})" "$stream" >"$work/fx-next" || true
    mv "$work/fx-next" "$stream"
    i=$((i + 1))
done

# --- user@host false-positive filtering (see header) ---
# Always safe, even on a line that mentions ssh: placeholders and action refs.
# B: what may follow a blanked name: not a further name character, and a '.'
# only at the end of a sentence (so a hex-looking host with a domain is still a host).
B='(\.?$|\.?[^A-Za-z0-9_.-])'
safe_sed='s#actions/[A-Za-z0-9_.-]+@v[0-9][A-Za-z0-9.]*# #g;
s#(^|[^A-Za-z0-9_])(user@host|repo@commit|repo@pin)'"$B"'# #g;
s#[A-Za-z0-9_.-]+@example\.(com|org|net|invalid)# #g;
s#[A-Za-z0-9_.-]+@b\.c# #g;
s#https?://lore\.kernel\.[A-Za-z0-9./-]*/[^ ]*@[A-Za-z0-9.-]+# #g'
# Public mailing-list and archive hosts (exact domains; a sub-domain or a
# longer name does not match). Like shape_sed, not applied to ssh lines.
ml_hosts='lists\.freedesktop\.org|vger\.kernel\.org|lore\.kernel\.org|mail-archive\.com|lists\.infradead\.org|lists\.linaro\.org|lists\.debian\.org|lists\.zephyrproject\.org'
# Only for lines without ssh/scp/sftp/rsync: pins, DT unit addresses, units.
shape_sed='s#@[0-9a-f]{1,40}'"$B"'# #g;
s#@N'"$B"'# #g;
s#(serial-getty|getty|modprobe)@[A-Za-z0-9]+# #g;
s#[A-Za-z0-9_.-]+@('"$ml_hosts"')'"$B"'# #g'
uh_re='[A-Za-z0-9_][A-Za-z0-9._-]*@[A-Za-z][A-Za-z0-9-]*'
uh_yaml_uses='^[[:space:]]*(-[[:space:]]+)?uses:[[:space:]]*[A-Za-z0-9_.-]+/[A-Za-z0-9_./-]+@[A-Za-z0-9_./-]+[[:space:]]*(#.*)?$'
remote_cmd='(^|[^A-Za-z])(ssh|scp|sftp|rsync)([^A-Za-z]|$)'

# Reads stream lines that matched uh_re; prints those still matching after
# the benign shapes are blanked out.
uh_filter() {
    while IFS= read -r l; do
        t=${l#*"$tab"}
        if printf '%s\n' "$t" | grep -Eq "$uh_yaml_uses"; then continue; fi
        r=$(printf '%s\n' "$t" | sed -E "$safe_sed; $safe_sed")
        if ! printf '%s\n' "$r" | grep -Eq "$remote_cmd"; then
            r=$(printf '%s\n' "$r" | sed -E "$shape_sed; $shape_sed")
        fi
        if printf '%s\n' "$r" | grep -Eq "$uh_re"; then printf '%s\n' "$l"; fi
    done
}

# report KIND: reads matching stream lines, appends "KIND: where: text".
report() { sed "s/^/$1: /; s/$tab/: /" >>"$hits"; }

# ipv4 (dependency files excluded, loopback/unspecified lines allowed)
grep -E '(^|[^0-9.])([0-9]{1,3}\.){3}[0-9]{1,3}($|[^0-9])' "$stream" |
    grep -vE '^(.*/)?(Cargo\.lock|vendor/)' |
    grep -vE "${tab}(.*[^0-9.])?(0\.0\.0\.0|127\.0\.0\.1)([^0-9.].*)?\$" |
    report ipv4
grep -E '/dev/(tty[A-Za-z0-9_]+|serial/by-id)' "$stream" |
    grep -vE '/dev/ttyRPMSG' |
    report tty
grep -E '(^|[^0-9A-Fa-f:-])(([0-9A-Fa-f]{2}:){5}|([0-9A-Fa-f]{2}-){5})[0-9A-Fa-f]{2}($|[^0-9A-Fa-f:-])' "$stream" |
    report mac
grep -E "$uh_re" "$stream" |
    grep -viE 'co-authored-by:|signed-off-by:|git@github\.com|git@gitlab\.com|noreply' |
    uh_filter |
    report user@host
grep -E -e '-----BEGIN ([A-Z0-9]+ )*PRIVATE KEY' "$stream" | report ssh-key
grep -iE '(password|passwd|token|secret|api_key) *= *["'\'']?[A-Za-z0-9+/_.-]' "$stream" |
    report secret

if [ -s "$hits" ]; then
    echo "check-no-board-secrets: possible board secrets added since $desc:" >&2
    sort -u "$hits" >&2
    exit 1
fi
echo "check-no-board-secrets: clean ($desc)"
