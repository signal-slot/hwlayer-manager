#!/bin/bash
# Self-test for check-test-data.sh.
# Run: bash scripts/check-test-data.test.sh
# Builds good and bad data trees under a mktemp directory inside $TMPDIR.

here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-test-data.sh
root=$(mktemp -d "${TMPDIR:-.}/ctd-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0

# Stub for check-no-board-secrets.sh: flags any line "LEAK" under the cwd,
# printed like the real script: "kind: path:line: text" on stderr.
cat > "$root/stub.sh" <<'T'
#!/bin/sh
p=$(git rev-parse --show-prefix 2>/dev/null)
hits=$(grep -rn LEAK . 2>/dev/null | sed "s#^\./#ipv4: $p#; s#:\([0-9]*\):#:\1: #")
[ -z "$hits" ] && exit 0
printf '%s\n' "$hits" >&2
exit 1
T

mkdir -p "$root/good/rk3588" "$root/good/bcm2712"
printf 'board data\n- props-2026-11-27.txt: props\n- uname.txt: 2026-11-27\n- probe.txt\n' > "$root/good/rk3588/README.md"
echo x > "$root/good/rk3588/props-2026-11-27.txt"
echo "Linux 6.18" > "$root/good/rk3588/uname.txt"
printf '# captured 2026-11-27\nx\n' > "$root/good/rk3588/probe.txt"
printf 'r\nkms.txt 2026-11-28\n' > "$root/good/bcm2712/README.md"
echo y > "$root/good/bcm2712/kms.txt"

mkdir -p "$root/bad/a" "$root/bad/b" "$root/bad/c"
echo x > "$root/bad/a/props-2026-11-27.txt"
printf 'r\n' > "$root/bad/b/README.md"
echo x > "$root/bad/b/orphan-2026-11-27.txt"
printf 'r\nundated.txt\nbig-2026-11-27.txt\nleak.txt 2026-11-27\n' > "$root/bad/c/README.md"
echo x > "$root/bad/c/undated.txt"
head -c 1048577 /dev/zero | tr '\0' a > "$root/bad/c/big-2026-11-27.txt"
echo LEAK > "$root/bad/c/leak.txt"
mkdir -p "$root/empty"

check() { # NAME WANT_RC PATTERN [ARGS...]
    name=$1 want=$2 pat=$3
    shift 3
    got=$(CHECK_NO_BOARD_SECRETS=$root/stub.sh bash "$script" "$@" 2>&1)
    rc=$?
    if [ "$rc" != "$want" ]; then
        echo "FAIL $name: exit $rc, want $want"; printf '%s\n' "$got"; fail=1
    elif [ -n "$pat" ] && ! printf '%s\n' "$got" | grep -Fq -- "$pat"; then
        echo "FAIL $name: output lacks '$pat'"; printf '%s\n' "$got"; fail=1
    else
        echo "ok   $name"
    fi
}

check good-clean 0 '' "$root/good"
check missing-root 0 'no data root' "$root/absent"
check no-folders 0 'no data folders' "$root/empty"
check no-readme 1 'a/README.md: missing README.md' "$root/bad"
check not-named 1 'b/orphan-2026-11-27.txt: not named in README.md' "$root/bad"
check undated 1 'c/undated.txt: no YYYY-MM-DD date' "$root/bad"
check too-big 1 'c/big-2026-11-27.txt: over 1 MiB' "$root/bad"
check secrets 1 'c/leak.txt: board secret' "$root/bad"
check bad-option 2 usage --bogus
# Current tree with the real secrets script: must exit 0.
bash "$script" >/dev/null 2>&1
rc=$?
if [ "$rc" = 0 ]; then echo "ok   current-tree"; else echo "FAIL current-tree: exit $rc"; fail=1; fi

[ "$fail" = 0 ] && echo "all tests passed"
exit "$fail"
