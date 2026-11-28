#!/bin/sh
# Self-test of check-busybox-compat.sh. Fixtures are generated under $TMPDIR.
set -u
here=$(cd "$(dirname "$0")" && pwd)
chk=$here/check-busybox-compat.sh
tmp=$(mktemp -d "${TMPDIR:-/tmp}/bbct.XXXXXX") || exit 2
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
rc=0
bad() {
    echo "FAIL: $*" >&2
    rc=1
}

# hit NAME LINE PATTERN-SUBSTRING : LINE must be reported as PATTERN
hit() {
    printf '#!/bin/sh\n%s\n' "$2" >"$tmp/$1.sh"
    out=$(sh "$chk" "$tmp/$1.sh" 2>&1)
    st=$?
    [ "$st" -eq 1 ] || bad "$1: exit $st, want 1"
    case $out in
    "$tmp/$1.sh:2:$3"*) ;;
    *) bad "$1: got '$out', want ...:2:$3" ;;
    esac
}
# clean NAME LINE : LINE must pass
clean() {
    printf '#!/bin/sh\n%s\n' "$2" >"$tmp/$1.sh"
    out=$(sh "$chk" "$tmp/$1.sh" 2>&1) || bad "$1: flagged: $out"
}

hit strtonum 'awk "{print strtonum(\$1)}" f' "awk strtonum"
hit gensub 'awk "{print gensub(/a/,\"b\",\"g\")}" f' "awk gensub"
hit printfx "printf '%x\\n' 'A'" "printf %x of a string"
hit printfx2 'printf "%02x" "abc"' "printf %x of a string"
hit statc 'stat -c %s /x' "stat -c"
hit statc2 'sz=$(stat -L -c %s /x)' "stat -c"
hit arr 'a=(1 2 3)' "bash array assignment"
hit arrx 'echo ${a[0]}' "bash array expansion"
hit decl 'declare -A m' "declare -a/-A"
hit dbr '[[ -n $x ]] && echo y' "[[ ]]"
hit localn 'local -n ref=$1' "local -n"
hit mapf 'mapfile -t l <f' "readarray/mapfile"
hit rdarr 'readarray l <f' "readarray/mapfile"
hit sede "sed -E 's/a+/b/' f" "sed -E/-r"
hit sedr "sed -r 's/a+/b/' f" "sed -E/-r"
hit grepp 'grep -P "\d" f' "grep -P"
hit grepp2 'grep -oP "a" f' "grep -P"
hit procsub 'diff <(ls a) <(ls b)' "process substitution"

clean plain 'echo hello; ls | grep -E "a|b"'
clean sedn "sed -n 's/a/b/p' f"
clean statf 'stat /x'
clean dollar 'x=${y:-1}; echo "$((1+2))"'
clean comment '# strtonum is not allowed, stat -c neither'
clean indented '    # mapfile here is only a comment'
clean printfd "printf '%x\\n' 255"
clean printfv 'printf "%x\n" "$n"'
clean okreason 'stat -c %s /x # busybox-ok: host-only helper'
clean okspace 'a=(1 2) # busybox-ok:   documented bash-only branch'
clean cat_ok 'cat file | sort > out'
clean cmp_ok 'if [ -n "$x" ] && [ -z "$y" ]; then :; fi'

# opt-out needs a reason
printf '#!/bin/sh\nstat -c %%s /x # busybox-ok:\n' >"$tmp/noreason.sh"
sh "$chk" "$tmp/noreason.sh" >/dev/null 2>&1 && bad "opt-out without reason accepted"

# several hits, ordered by line, across files
printf '#!/bin/sh\nmapfile -t l <f\necho ok\nstat -c %%s /x\n' >"$tmp/multi.sh"
out=$(sh "$chk" "$tmp/multi.sh" "$tmp/plain.sh" 2>&1)
want="$tmp/multi.sh:2:readarray/mapfile
$tmp/multi.sh:4:stat -c"
[ "$out" = "$want" ] || bad "multi: got '$out'"

# usage errors
sh "$chk" "$tmp/nope.sh" >/dev/null 2>&1
[ $? -eq 2 ] || bad "missing file must exit 2"

# default file set: am62p-*.sh minus *.test.sh; must run and exit 0 or 1
sh "$chk" >/dev/null 2>&1
st=$?
[ "$st" -le 1 ] || bad "default run exit $st"

# runs under dash when available
if command -v dash >/dev/null 2>&1; then
    dash "$chk" "$tmp/plain.sh" >/dev/null 2>&1 || bad "dash: clean file rejected"
    dash "$chk" "$tmp/statc.sh" >/dev/null 2>&1
    [ $? -eq 1 ] || bad "dash: hit not exit 1"
fi

[ $rc -eq 0 ] && echo "check-busybox-compat.test.sh: ok"
exit $rc
