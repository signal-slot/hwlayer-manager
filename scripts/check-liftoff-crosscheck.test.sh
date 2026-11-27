#!/bin/sh
# Self-test for check-liftoff-crosscheck.sh (structural checks only; the cargo
# steps are skipped).  Run: sh scripts/check-liftoff-crosscheck.test.sh
# Builds throwaway git repositories under $TMPDIR; never touches the real one.

here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-liftoff-crosscheck.sh
root=$(mktemp -d "${TMPDIR:-.}/lxc-test.XXXXXX") || exit 2
case $root in /*) ;; *) root=$(cd "$root" && pwd) ;; esac
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
n=0

# fresh repo: root manifest without the crate, crate manifest with empty [workspace]
fresh() {
    n=$((n + 1))
    repo=$root/r$n
    mkdir -p "$repo/tools/liftoff-crosscheck" || exit 2
    cd "$repo" || exit 2
    git init -q . || exit 2
    printf '[workspace]\nmembers = ["crates/a", "tools/vm-init"]\n' > Cargo.toml
    printf '[package]\nname = "x"\n\n[workspace]\n' > tools/liftoff-crosscheck/Cargo.toml
}
check() { # NAME WANT_RC PATTERN
    git add -A >/dev/null 2>&1
    got=$(LIFTOFF_XC_STRUCTURE_ONLY=1 bash "$script" 2>&1)
    rc=$?
    if [ "$rc" != "$2" ]; then
        echo "FAIL $1: exit $rc, want $2"; printf '%s\n' "$got"; fail=1
    elif ! printf '%s\n' "$got" | grep -Fq -- "$3"; then
        echo "FAIL $1: output lacks '$3'"; printf '%s\n' "$got"; fail=1
    else
        echo "ok   $1"
    fi
}

fresh
check good 0 'liftoff-crosscheck: ok'

fresh; rm -rf tools/liftoff-crosscheck
check crate-absent 0 'liftoff-crosscheck: crate not present, skipped'

fresh; printf '[package]\nname = "x"\n' > tools/liftoff-crosscheck/Cargo.toml
check no-workspace-table 1 'has no [workspace] table'

fresh; printf '[package]\nname = "x"\n[workspace]\nmembers = []\n' > tools/liftoff-crosscheck/Cargo.toml
check nonempty-workspace 1 'must be empty'

fresh; printf '[workspace]\n# comment\n\n[dependencies]\ny = "1"\n' > tools/liftoff-crosscheck/Cargo.toml
check workspace-comment-then-table 0 'liftoff-crosscheck: ok'

fresh; printf '[workspace]\nmembers = [\n  "crates/a",\n  "tools/liftoff-crosscheck",\n]\n' > Cargo.toml
check root-member 1 'lists tools/liftoff-crosscheck'

fresh; printf '[workspace]\nmembers = ["crates/a"]\nexclude = ["tools/liftoff-crosscheck"]\n' > Cargo.toml
check root-exclude-ok 0 'liftoff-crosscheck: ok'

for f in alloc.c plane.c layer.c device.c output.c libliftoff.h; do
    fresh; mkdir -p third/sub; : > "third/sub/$f"
    check "tracked-$f" 1 'libliftoff source is tracked'
done

fresh; : > tools/liftoff-crosscheck/planes.c
check other-c-file-ok 0 'liftoff-crosscheck: ok'

[ "$fail" = 0 ] && echo "all tests passed"
exit "$fail"
