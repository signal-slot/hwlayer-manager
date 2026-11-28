#!/bin/sh
# board-run-bundle.sh - run a board program and store its output as dated,
# scrubbed, checked board data. See board-run-bundle-README.md.
# Usage: board-run-bundle.sh --soc rk3588|bcm2711|bcm2712|imx95 --kind sweep|verify|caps|scenarios
#            [--name NAME] [--hostname NAME]... --out DIR -- CMD...
# Env: BOARD_RUN_BUNDLE_UNAME (replaces `uname -r`), BOARD_RUN_BUNDLE_DATE
#      (replaces today's date), BOARD_RUN_BUNDLE_SCRUB, BOARD_RUN_BUNDLE_SECRETS
#      (replace the scripts used, for tests).
# Exit: 0 ok, 1 scrub or secrets check failed, 2 usage, 3 CMD failed (the file
# is still written and checked).

here=$(cd "$(dirname "$0")" && pwd) || exit 2
scrub=${BOARD_RUN_BUNDLE_SCRUB:-$here/scrub-kms-dump.py}
secrets=${BOARD_RUN_BUNDLE_SECRETS:-$here/check-no-board-secrets.sh}

usage() {
    echo "usage: $0 --soc rk3588|bcm2711|bcm2712|imx95 --kind sweep|verify|caps|scenarios [--name NAME] [--hostname NAME]... --out DIR -- CMD..." >&2
    exit 2
}

soc= kind= name= out=
hostargs=
while [ $# -gt 0 ]; do
    case $1 in
        --soc) [ $# -ge 2 ] || usage; soc=$2; shift 2 ;;
        --kind) [ $# -ge 2 ] || usage; kind=$2; shift 2 ;;
        --name) [ $# -ge 2 ] || usage; name=$2; shift 2 ;;
        --hostname) [ $# -ge 2 ] || usage; hostargs="$hostargs${hostargs:+,}$2"; shift 2 ;;
        --out) [ $# -ge 2 ] || usage; out=$2; shift 2 ;;
        --) shift; break ;;
        *) usage ;;
    esac
done
case $soc in rk3588|bcm2711|bcm2712|imx95) ;; *) usage ;; esac
case $kind in sweep|verify|caps|scenarios) ;; *) usage ;; esac
[ -n "$out" ] && [ $# -gt 0 ] || usage
[ -n "$name" ] || name=$soc
case $name in
    *[!A-Za-z0-9_.-]*|.*) echo "board-run-bundle: bad --name" >&2; exit 2 ;;
esac
[ -d "$out" ] || { echo "board-run-bundle: no output directory" >&2; exit 2; }

date=${BOARD_RUN_BUNDLE_DATE:-$(date +%Y-%m-%d)}
kernel=${BOARD_RUN_BUNDLE_UNAME:-$(uname -r)}
file=$out/$kind-$date-$name.txt

work=$(mktemp -d "${TMPDIR:-/tmp}/board-run-bundle.XXXXXX") || exit 2
trap 'rm -rf "$work"' EXIT HUP INT TERM

# Print the argument, or <host> if it looks like a host or address.
redact1() {
    case $1 in
        *.sh|*.py|*.txt|*.md|*.rs) printf '%s' "$1"; return ;;
    esac
    if printf '%s\n' "$1" | grep -Eq \
        -e '^([^/ ]*@)[^/ ]+$' \
        -e '^[0-9]{1,3}(\.[0-9]{1,3}){3}(:[0-9]+)?$' \
        -e '^[0-9A-Fa-f]{2}([:-][0-9A-Fa-f]{2}){5}$' \
        -e '^\[?[0-9A-Fa-f]*:[0-9A-Fa-f:]*:[0-9A-Fa-f:.]*\]?$' \
        -e '^[A-Za-z0-9_-]+(\.[A-Za-z0-9_-]+)+(:[0-9]+)?$' \
        -e '^/dev/(tty|serial/)'; then
        printf '<host>'
    else
        printf '%s' "$1"
    fi
}
# Same, also for the value of an --opt=value argument.
redact() {
    case $1 in
        --*=*)
            opt=${1%%=*}
            val=${1#*=}
            printf '%s=%s' "$opt" "$(redact1 "$val")"
            ;;
        *) redact1 "$1" ;;
    esac
}
cmdline=
for a in "$@"; do
    cmdline="$cmdline${cmdline:+ }$(redact "$a")"
done

"$@" >"$work/raw" 2>&1 </dev/null
cmd_rc=$?

{
    echo "# observed"
    echo "# date: $date"
    echo "# kernel: $kernel"
    echo "# command: $cmdline"
    cat "$work/raw"
} >"$work/full"

rc=0
if python3 -I "$scrub" ${hostargs:+--hostname "$hostargs"} "$work/full" >"$work/scrubbed" 2>"$work/scrub.err"; then
    cp "$work/scrubbed" "$file" || exit 2
else
    cat "$work/scrub.err" >&2
    echo "board-run-bundle: scrub failed; no file written" >&2
    exit 1
fi

# check-no-board-secrets.sh scans the git diff, so give it a throwaway repo.
repo=$work/repo
mkdir "$repo" && cp "$file" "$repo/" &&
    git -C "$repo" init -q &&
    git -C "$repo" -c user.name=bundle -c user.email=bundle@example.invalid \
        commit -q --allow-empty -m "base" &&
    base=$(git -C "$repo" rev-parse HEAD) &&
    git -C "$repo" add . &&
    git -C "$repo" -c user.name=bundle -c user.email=bundle@example.invalid \
        commit -q -m "board run bundle" || { echo "board-run-bundle: cannot set up secrets check" >&2; exit 1; }
# An explicit base, so the checker never picks a branch name of its own and
# scans an empty range.
if ! (cd "$repo" && sh "$secrets" "$base" >"$work/sec.out" 2>&1); then
    cat "$work/sec.out" >&2
    echo "board-run-bundle: board secrets found in $file" >&2
    rc=1
fi

echo "board-run-bundle: wrote $file" >&2
echo "suggested README line: - $(basename "$file"): $kind run on $soc, kernel $kernel, observed $date"
[ "$rc" = 0 ] || exit "$rc"
if [ "$cmd_rc" != 0 ]; then
    echo "board-run-bundle: command exited $cmd_rc" >&2
    exit 3
fi
exit 0
