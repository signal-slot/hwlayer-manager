#!/bin/sh
# check-no-board-secrets.sh - fail if a branch adds board addresses or credentials.
#
# Project rule: no board addresses, host names, IPs, serial numbers, UART
# paths or credentials in any file or commit message.
#
# Run (needs only git, grep and a POSIX sh):
#   scripts/check-no-board-secrets.sh [BASE_REF]      # BASE_REF defaults to origin/main
#   scripts/check-no-board-secrets.test.sh            # self-test
#
# Scans the ADDED lines of `git diff BASE_REF...HEAD` and every line of the
# commit messages of `git log BASE_REF..HEAD`. Existing content is not scanned.
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
#   - This script and its test are not scanned (they spell out the patterns).

if [ "$#" -gt 1 ]; then
    echo "usage: $0 [BASE_REF]" >&2
    exit 2
fi
base=${1:-origin/main}
tab=$(printf '\t')

git rev-parse --verify --quiet "$base^{commit}" >/dev/null || {
    echo "check-no-board-secrets: unknown base ref: $base" >&2
    exit 2
}

work=$(mktemp -d "${TMPDIR:-.}/nbs.XXXXXX") || exit 2
trap 'rm -rf "$work"' EXIT HUP INT TERM
raw=$work/raw
stream=$work/stream
hits=$work/hits

# Stream format: "<where>:<line><TAB><text>"; where is a path or commit:<sha>.
# Added lines of the diff.
git -c core.quotepath=off diff -U0 --no-color --no-ext-diff "$base...HEAD" |
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
        scripts/check-no-board-secrets.sh | scripts/check-no-board-secrets.test.sh) ;;
        *) printf '%s:%s\t%s\n' "$file" "$n" "${l#+}" ;;
        esac
        n=$((n + 1))
        ;;
    esac
done >"$raw"

# Commit messages.
for c in $(git rev-list "$base..HEAD"); do
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

# report KIND: reads matching stream lines, appends "KIND: where: text".
report() { sed "s/^/$1: /; s/$tab/: /" >>"$hits"; }

# ipv4 (dependency files excluded, loopback/unspecified lines allowed)
grep -E '(^|[^0-9.])([0-9]{1,3}\.){3}[0-9]{1,3}($|[^0-9])' "$stream" |
    grep -vE '^(.*/)?(Cargo\.lock|vendor/)' |
    grep -vE "${tab}(.*[^0-9.])?(0\.0\.0\.0|127\.0\.0\.1)([^0-9.].*)?\$" |
    report ipv4
grep -E '/dev/(tty[A-Za-z0-9_]+|serial/by-id)' "$stream" | report tty
grep -E '(^|[^0-9A-Fa-f:-])([0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}($|[^0-9A-Fa-f:-])' "$stream" |
    report mac
grep -E '[A-Za-z0-9_][A-Za-z0-9._-]*@[A-Za-z][A-Za-z0-9-]*' "$stream" |
    grep -viE 'co-authored-by:|signed-off-by:|git@github\.com|git@gitlab\.com|noreply' |
    report user@host
grep -E -e '-----BEGIN ([A-Z0-9]+ )*PRIVATE KEY' "$stream" | report ssh-key
grep -iE '(password|passwd|token|secret|api_key) *= *["'\'']?[A-Za-z0-9+/_.-]' "$stream" |
    report secret

if [ -s "$hits" ]; then
    echo "check-no-board-secrets: possible board secrets added since $base:" >&2
    sort -u "$hits" >&2
    exit 1
fi
echo "check-no-board-secrets: clean ($base...HEAD)"
