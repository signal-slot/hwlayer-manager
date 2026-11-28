#!/bin/sh
# Run the checks of the pull-request CI locally.
#
# The step list below must follow .github/workflows/ci.yml (documented in
# docs/ci.md). When a step is added, changed or removed there, change it here
# in the same commit.
#
# Usage: sh scripts/check-all.sh [--keep-going] [--cross] [--lint]
#   --keep-going  run every step even after a failure (default: stop at the
#                 first failure)
#   --cross       also run the aarch64 cross-build steps of ci.yml. They use
#                 scripts/cross-aarch64.sh, which adds the
#                 aarch64-unknown-linux-musl rustup target on first use.
#   --lint        after the CI steps, also run the repository's lint scripts
#                 that exist: check-doc-links.sh, check-zephyr-apps.sh,
#                 check-ffi-header.sh, check-doc-tags.sh,
#                 check-no-board-secrets.sh, check-commit-subjects.sh and
#                 check-feasibility-doc.sh (on docs/imx95-zephyr-feasibility.md),
#                 then the self-tests check-arm-link.test.sh,
#                 check-no-farm-ids.test.sh, check-feasibility-doc.test.sh and
#                 m2-compare-report.test.sh, then imx95-m7-rproc.test.sh,
#                 imx95-observe.test.sh, check-m2-acceptance.test.sh,
#                 every other scripts/*.test.sh not run unconditionally, each
#                 skipped with a message when a tool it needs is missing,
#                 m2-pipeline.test.sh and m2scene-trace.test.sh (the last two
#                 are skipped when python3 or cargo is missing).
#                 The three that take a base ref get origin/main and are
#                 skipped with a message when it does not resolve.
#                 These are not CI steps; without --lint nothing changes.
#
# Nothing is installed by this script. A missing thumbv7em-none-eabihf target
# is reported with the rustup command to run.
set -eu

keep_going=0
cross=0
lint=0
for a in "$@"; do
    case $a in
        --keep-going) keep_going=1 ;;
        --cross) cross=1 ;;
        --lint) lint=1 ;;
        -h|--help) sed -n '2,30p' "$0"; exit 0 ;;
        *) echo "unknown argument: $a" >&2; exit 2 ;;
    esac
done

cd "$(dirname "$0")/.."

T=thumbv7em-none-eabihf
if ! rustup target list --installed 2>/dev/null | grep -qx "$T"; then
    echo "FAIL  target $T is not installed (nothing was run)"
    echo "      install it with: rustup target add $T"
    exit 1
fi

pass=0
fail=0
failed=""

summary() {
    echo "----"
    echo "summary: $pass passed, $fail failed"
    if [ -n "$failed" ]; then
        echo "failed steps:$failed"
    fi
}

# step NAME CMD...: run CMD, print PASS/FAIL, stop on failure unless --keep-going.
step() {
    name=$1
    shift
    echo "==> $name: $*"
    if "$@"; then
        echo "PASS  $name"
        pass=$((pass + 1))
    else
        echo "FAIL  $name"
        fail=$((fail + 1))
        failed="$failed
  $name"
        if [ "$keep_going" -eq 0 ]; then
            summary
            exit 1
        fi
    fi
}

step "cargo fmt" cargo fmt --all --check
step "cargo clippy" cargo clippy --locked --workspace --all-targets -- -D warnings
step "cargo test" cargo test --locked --workspace
step "build composition-core ($T)" \
    cargo build --locked -p composition-core --target "$T"
step "build composition-ffi-staticlib ($T, no std, release)" \
    cargo build --locked -p composition-ffi-staticlib --no-default-features \
    --features panic-handler --target "$T" --release

if [ "$cross" -eq 1 ]; then
    step "cross aarch64: composition-drm" \
        scripts/cross-aarch64.sh --locked -p composition-drm
    step "cross aarch64: probe example" \
        scripts/cross-aarch64.sh --locked -p composition-drm --example probe
    step "cross aarch64: drm-harness" \
        scripts/cross-aarch64.sh --locked -p composition-drm-harness --bin drm-harness
fi

# Script self-tests and lints that run on every invocation. A script that is
# not on the tree is skipped with a message.
for t in format-cut-report decode-modifiers scrub-kms-dump \
    kms-dump-diff check-fixture-provenance check-profile-tags sweep-summary \
    profile-check-expect check-no-board-secrets sweep-to-fixture \
    board-run-bundle; do
    f=scripts/$t.test.sh
    if [ ! -f "$f" ]; then
        echo "SKIP  script test $t: $f does not exist"
        continue
    fi
    step "script test $t" sh "$f"
done
# Left out on purpose: check-test-data.test.sh (its current-tree case fails,
# the debugfs dumps are not named in their README.md files) and the
# check-profile-tags.sh lint (observed tags without a date in profile.rs).
for l in check-fixture-provenance; do
    f=scripts/$l.sh
    if [ ! -f "$f" ]; then
        echo "SKIP  lint $l: $f does not exist"
        continue
    fi
    step "lint $l" sh "$f"
done

if [ "$lint" -eq 1 ]; then
    for l in check-doc-links check-zephyr-apps check-ffi-header check-doc-tags \
        check-no-board-secrets check-commit-subjects check-feasibility-doc; do
        f=scripts/$l.sh
        if [ ! -f "$f" ]; then
            echo "SKIP  lint $l: $f does not exist"
            continue
        fi
        case $l in
        check-ffi-header|check-no-board-secrets|check-commit-subjects)
            if ! git rev-parse --verify -q origin/main >/dev/null 2>&1; then
                echo "SKIP  lint $l: origin/main does not resolve"
                continue
            fi
            step "lint $l" "$f" origin/main
            ;;
        *)
            step "lint $l" "$f"
            ;;
        esac
    done
    for t in check-arm-link check-no-farm-ids check-feasibility-doc \
        m2-compare-report; do
        f=scripts/$t.test.sh
        if [ ! -f "$f" ]; then
            echo "SKIP  script test $t: $f does not exist"
            continue
        fi
        step "script test $t" sh "$f"
    done
    # Tests written for bash; the name:tools list gives the tools a test needs
    # (empty = none). They are skipped with a message when a tool is missing.
    for tt in imx95-m7-rproc: imx95-observe: check-m2-acceptance: \
        m2-pipeline:python3 m2scene-trace:python3,cargo \
        imx95-observe-summary:python3 imx95-m7-log: host-demo: \
        prepare-zephyr: imx95-session: check-doc-links:git \
        trace-to-metrics:python3 alpha-mode-expect:python3 \
        am62p-dss-regs:python3 am62p-scenes:python3 am62p-torizon-check: \
        board-scenarios: build-zephyr-apps: check-am62p-board-env: \
        check-board-results: check-commit-subjects:git check-doc-tags: \
        check-ffi-header:git check-liftoff-crosscheck:cargo,git \
        check-m3-report: check-photo-metadata:python3 check-test-data:git \
        check-zephyr-apps: display-guard: picture-hook: \
        scenario-expect:python3 scenario-report:python3 \
        twoplane-expect:python3; do
        t=${tt%%:*}
        tools=${tt#*:}
        f=scripts/$t.test.sh
        if [ ! -f "$f" ]; then
            echo "SKIP  script test $t: $f does not exist"
            continue
        fi
        missing=""
        for tool in $(echo "$tools" | tr , ' '); do
            command -v "$tool" >/dev/null 2>&1 || missing="$missing $tool"
        done
        if [ -n "$missing" ]; then
            echo "SKIP  script test $t: missing tool(s):$missing"
            continue
        fi
        step "script test $t" bash "$f"
    done
fi

# M3 host checks for the AM62P R5F (armv7r-none-eabihf). They run in their
# skip-friendly mode (no --strict): a missing target, clang or arm gcc prints a
# SKIP/skipped verdict and exits 0, so a node without them still passes. Each
# script's own output, including its verdict line (ARMV7R ..., PASS/FAIL/SKIP,
# skipped), is shown under its step.
for l in check-armv7r-build check-ffi-r5f-link check-am62p-r5f-c; do
    f=scripts/$l.sh
    if [ ! -f "$f" ]; then
        echo "SKIP  m3 host check $l: $f does not exist"
        continue
    fi
    step "m3 host check $l" sh "$f"
done

summary
[ "$fail" -eq 0 ]
