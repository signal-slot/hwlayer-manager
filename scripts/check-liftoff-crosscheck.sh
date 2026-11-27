#!/usr/bin/env bash
# Checks of the libliftoff cross-check crate (tools/liftoff-crosscheck), run by
# hand.  NOT part of CI and not called from scripts/check-all.sh: the crate sits
# outside the workspace and this script is for it only (docs/ci.md unchanged).
#
# Usage: scripts/check-liftoff-crosscheck.sh        (run from anywhere in the repo)
# Exit:  0 all checks pass (or the crate is not present), 1 a check failed.
#
# Checks: cargo fmt --check, cargo clippy -D warnings and cargo test on the
# crate (CARGO_TARGET_DIR under the crate's target/), the crate's Cargo.toml has
# an empty [workspace] table, the root Cargo.toml does not list the crate as a
# member, and no libliftoff source file is tracked by git.
#
# LIFTOFF_XC_STRUCTURE_ONLY=1 skips the cargo steps (used by the self-test).
set -uo pipefail
cd "$(git rev-parse --show-toplevel)" || exit 1

crate=tools/liftoff-crosscheck
if [ ! -d "$crate" ]; then
  echo 'liftoff-crosscheck: crate not present, skipped'
  exit 0
fi

fail=0
bad() { echo "FAIL: $*"; fail=1; }

# Empty [workspace] table: the header is present and no key follows it before
# the next table header.
manifest=$crate/Cargo.toml
if [ ! -f "$manifest" ]; then
  bad "$manifest is missing"
else
  ws=$(awk '
    /^[[:space:]]*\[workspace\][[:space:]]*(#.*)?$/ { in_ws = 1; found = 1; next }
    /^[[:space:]]*\[/ { in_ws = 0 }
    in_ws && !/^[[:space:]]*(#.*)?$/ { extra = 1 }
    END { print (found ? "found" : "missing") (extra ? " nonempty" : "") }
  ' "$manifest")
  case $ws in
    missing*) bad "$manifest has no [workspace] table" ;;
    *nonempty) bad "$manifest: the [workspace] table must be empty" ;;
  esac
fi

# The root manifest must not list the crate as a member.
if [ -f Cargo.toml ]; then
  members=$(awk '
    /^[[:space:]]*members[[:space:]]*=/ { on = 1 }
    on { print }
    on && /\]/ { on = 0 }
  ' Cargo.toml)
  case $members in
    *"$crate"*) bad "root Cargo.toml lists $crate as a workspace member" ;;
  esac
fi

# No libliftoff source may be tracked.
src=$(git ls-files | grep -E '(^|/)(alloc\.c|plane\.c|layer\.c|device\.c|output\.c|libliftoff\.h)$' || true)
[ -z "$src" ] || bad "libliftoff source is tracked: $(printf '%s' "$src" | tr '\n' ' ')"

if [ -z "${LIFTOFF_XC_STRUCTURE_ONLY:-}" ] && [ -f "$manifest" ]; then
  CARGO_TARGET_DIR=$(pwd)/$crate/target
  export CARGO_TARGET_DIR
  cargo fmt --manifest-path "$manifest" --check || bad 'cargo fmt --check'
  cargo clippy --manifest-path "$manifest" --all-targets -- -D warnings || bad 'cargo clippy'
  cargo test --manifest-path "$manifest" || bad 'cargo test'
fi

[ "$fail" -eq 0 ] && echo 'liftoff-crosscheck: ok'
exit "$fail"
