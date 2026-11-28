#!/bin/sh
# Tests for check-am62p-board-env.sh using fake directories under $TMPDIR
# (falls back to the repository's target/ dir, never /tmp).
set -u
here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-am62p-board-env.sh
shbin=$(command -v sh)
base=${TMPDIR:-$here/../target}
mkdir -p "$base" || exit 1
tmp=$(mktemp -d "$base/am62penv.XXXXXX") || exit 1
trap 'rm -rf "$tmp"' EXIT
fails=0

mkdir "$tmp/bin" "$tmp/empty" "$tmp/cm" "$tmp/sdkv"
for t in west cmake; do
    printf '#!/bin/sh\n' > "$tmp/bin/$t"
    chmod +x "$tmp/bin/$t"
done
cp "$tmp/bin/west" "$tmp/cm/west"
: > "$tmp/sdkv/sdk_version"

g=$tmp/good
mkdir -p "$g/show/zephyr" "$g/ws/.west" "$g/ws/zephyr" "$g/sdk/cmake"
: > "$g/show/zephyr/module.yml"
: > "$g/ws/zephyr/VERSION"
P=$tmp/bin

# run name want_rc pattern show ws sdk path   (empty value = variable unset)
run() {
    name=$1 want=$2 pat=$3 show=$4 ws=$5 sdk=$6 pth=$7
    out=$(
        unset MCUI_SHOWCASE_DIR ZEPHYR_WORKSPACE ZEPHYR_SDK_INSTALL_DIR
        [ -n "$show" ] && export MCUI_SHOWCASE_DIR="$show"
        [ -n "$ws" ] && export ZEPHYR_WORKSPACE="$ws"
        [ -n "$sdk" ] && export ZEPHYR_SDK_INSTALL_DIR="$sdk"
        PATH=$pth "$shbin" "$script"
    )
    rc=$?
    ok=1
    [ "$rc" -eq "$want" ] || ok=0
    printf '%s\n' "$out" | grep -qE -- "$pat" || ok=0
    # no value or path may leak
    for leak in "$tmp" "$here" "$show" "$ws" "$sdk" "$pth"; do
        [ -n "$leak" ] || continue
        if printf '%s\n' "$out" | grep -qF -- "$leak"; then ok=0; fi
    done
    if [ "$ok" -eq 1 ]; then
        echo "pass $name"
    else
        echo "FAIL $name (rc=$rc): $out"
        fails=$((fails + 1))
    fi
}

run all-ok 0 '^BOARD-ENV OK$' "$g/show" "$g/ws" "$g/sdk" "$P"
run sdk-version-file 0 '^ENV ZEPHYR_SDK_INSTALL_DIR ok$' "$g/show" "$g/ws" "$tmp/sdkv" "$P"

run show-missing 1 '^ENV MCUI_SHOWCASE_DIR missing$' "" "$g/ws" "$g/sdk" "$P"
run show-missing-verdict 1 '^BOARD-ENV MISSING MCUI_SHOWCASE_DIR$' "" "$g/ws" "$g/sdk" "$P"
run ws-missing 1 '^BOARD-ENV MISSING ZEPHYR_WORKSPACE$' "$g/show" "" "$g/sdk" "$P"
run sdk-missing 1 '^BOARD-ENV MISSING ZEPHYR_SDK_INSTALL_DIR$' "$g/show" "$g/ws" "" "$P"
run all-missing 1 '^BOARD-ENV MISSING MCUI_SHOWCASE_DIR ZEPHYR_WORKSPACE ZEPHYR_SDK_INSTALL_DIR west cmake$' "" "" "" "$tmp/empty"

run show-unusable 1 '^ENV MCUI_SHOWCASE_DIR unusable zephyr/module.yml$' "$tmp/empty" "$g/ws" "$g/sdk" "$P"
run ws-unusable 1 '^ENV ZEPHYR_WORKSPACE unusable .west/; zephyr/VERSION$' "$g/show" "$tmp/empty" "$g/sdk" "$P"
run sdk-unusable 1 '^ENV ZEPHYR_SDK_INSTALL_DIR unusable cmake.sdk_version$' "$g/show" "$g/ws" "$tmp/empty" "$P"
run not-a-dir 1 '^ENV MCUI_SHOWCASE_DIR unusable not a directory$' "$g/show/zephyr/module.yml" "$g/ws" "$g/sdk" "$P"
run west-missing 1 '^TOOL west missing$' "$g/show" "$g/ws" "$g/sdk" "$tmp/empty"
run cmake-missing 1 '^BOARD-ENV MISSING cmake$' "$g/show" "$g/ws" "$g/sdk" "$tmp/cm"

# read-only: the fake tree is unchanged
before=$(cd "$g" && find . | sort)
run again 0 '^BOARD-ENV OK$' "$g/show" "$g/ws" "$g/sdk" "$P"
after=$(cd "$g" && find . | sort)
if [ "$before" != "$after" ]; then
    echo "FAIL tree changed"
    fails=$((fails + 1))
fi

if [ "$fails" -ne 0 ]; then
    echo "$fails failure(s)"
    exit 1
fi
echo "all passed"
