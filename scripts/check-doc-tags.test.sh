#!/bin/sh
# Self-test for check-doc-tags.sh.
# Run: sh scripts/check-doc-tags.test.sh
# Builds fixture files under a mktemp directory inside $TMPDIR.

here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-doc-tags.sh
root=$(mktemp -d "${TMPDIR:-.}/dt-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
sha=0123456789abcdef0123456789abcdef01234567

cat > "$root/good.md" <<T
# Good
Board state (observed, 2026-11-28): remoteproc0 offline.
Read from source: <https://github.com/o/r/blob/$sha/a.c>.
Inferred: the M7 owns nothing.
T
cat > "$root/section.md" <<'T'
## Run 2026-11-28
First line.
- [observed] clk is on
- **observed** pd is on
T
printf '%s\n' '# No tags' 'plain text only' > "$root/notag.md"
cat > "$root/nodate.md" <<'T'
# Undated
Intro.
- [observed] clk is on
- unrelated: the word observed in prose is fine
T
cat > "$root/url.md" <<T
# Urls
inferred things.

see https://github.com/o/r/blob/main/a.c

far away filler
filler
filler
https://git.toradex.com/cgit/linux.git/tree/x?h=toradex-6.6 and v6.6.52
raw: https://raw.githubusercontent.com/o/r/$sha/f
tag: https://github.com/o/r/releases/tag/v1.2.3
T

check() { # NAME WANT_RC PATTERN ARGS...
    name=$1 want=$2 pat=$3
    shift 3
    got=$(sh "$script" "$@" 2>&1)
    rc=$?
    if [ "$rc" != "$want" ]; then
        echo "FAIL $name: exit $rc, want $want"; printf '%s\n' "$got"; fail=1
    elif [ -n "$pat" ] && ! printf '%s\n' "$got" | grep -Fq -- "$pat"; then
        echo "FAIL $name: output lacks '$pat'"; printf '%s\n' "$got"; fail=1
    else
        echo "ok   $name"
    fi
}
absent() { # NAME PATTERN ARGS...
    name=$1 pat=$2
    shift 2
    if sh "$script" "$@" 2>&1 | grep -Fq -- "$pat"; then
        echo "FAIL $name: unexpected '$pat'"; fail=1
    else
        echo "ok   $name"
    fi
}

check good-clean 0 '' "$root/good.md"
check section-date 0 '' "$root/section.md"
check no-tag 1 "notag.md:1: no evidence tag" "$root/notag.md"
check undated 1 "nodate.md:3: observed tag without" "$root/nodate.md"
absent prose-observed-ignored "nodate.md:4" "$root/nodate.md"
check url-unpinned 1 "url.md:4: git-host URL" "$root/url.md"
absent url-sha-ok "url.md:10" "$root/url.md"
absent url-tag-ok "url.md:11" "$root/url.md"
absent url-near-tag-ok "url.md:9" "$root/url.md"
check multi-file-rc 1 "notag.md:1" "$root/good.md" "$root/notag.md"
check bad-option 2 usage --bogus
check missing-file 2 'not a file' "$root/absent.md"
# Default file set: findings are allowed, a usage error is not.
sh "$script" >/dev/null 2>&1
rc=$?
if [ "$rc" = 0 ] || [ "$rc" = 1 ]; then
    echo "ok   default-set-runs"
else
    echo "FAIL default-set-runs: exit $rc"; fail=1
fi

[ "$fail" = 0 ] && echo "all tests passed"
exit "$fail"
