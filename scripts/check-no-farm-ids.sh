#!/usr/bin/env bash
# Fail if a tracked file contains a farm identifier: board address, host name,
# serial/UART path, ssh target, MAC address or credential.
#
# Usage: scripts/check-no-farm-ids.sh        (run from anywhere in the repo)
# Exit:  0 clean, 1 matches remain (printed as file:line:text).
#
# Allowlist: each ALLOW entry is "extended regex|reason".  Allowed substrings
# are blanked out of a matched line, then the category regex is tested again
# on what is left, so an allowed string never hides another identifier on the
# same line.  Keep each regex narrow and give every entry a reason.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

SQ="'"
IPV4='(^|[^0-9.])([0-9]{1,3}\.){3}[0-9]{1,3}($|[^0-9])'
TSNET='[A-Za-z0-9-]+\.ts\.net'
SERIAL='/dev/(ttyUSB|ttyACM)[0-9]*|/dev/serial/by-(id|path)/'
SSHTGT='ssh[[:space:]]+([^[:space:]@]+[[:space:]]+)*[a-z_][a-z0-9_-]*@[A-Za-z0-9]|(^|[^A-Za-z0-9_.@/-])[a-z_][a-z0-9_-]*@[A-Za-z0-9][A-Za-z0-9.-]*\.[A-Za-z]{2,}'
MAC='(^|[^0-9A-Fa-f:])([0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}($|[^0-9A-Fa-f:])'
CRED='(password|passwd|token)[[:space:]]*[=:][[:space:]]*["'"$SQ"']?[^[:space:]"'"$SQ"'$<{(]{3,}'

# Allowlist (regex|reason).  The reason is everything after the last '|'.
ALLOW=(
  '(^|[^0-9.])(127\.0\.0\.1|0\.0\.0\.0)($|[^0-9])|loopback / unspecified address, not a farm host'
  '(^|[^0-9.])(192\.0\.2|198\.51\.100|203\.0\.113)\.[0-9]+|RFC 5737 documentation ranges'
  '(^|[^0-9.])(6\.6\.157|12\.0\.4|7\.8\.0)($|[^0-9.])|board kernel / Weston / BSP versions that look like IPs'
  '@(example\.(com|org|net)|users\.noreply\.github\.com)|documentation or noreply addresses'
  '@lists\.freedesktop\.org|public upstream mailing-list address in kernel patch-series URLs'
  'git@github\.com|public GitHub git remote user, not a farm host'
  '(password|passwd|token)[[:space:]]*[=:][[:space:]]*.?(none|null|true|false|xxx+|redacted|changeme)|placeholder value'
)

# Never scanned: this script (holds the patterns) and the lockfile.
EXCLUDE=(':(exclude)scripts/check-no-farm-ids.sh' ':(exclude)Cargo.lock')

allow_re=""
for e in "${ALLOW[@]}"; do allow_re+="${allow_re:+|}(${e%|*})"; done

fail=0
scan() { # name regex [grep flags]
  local name=$1 re=$2 out
  shift 2
  out=$(git grep -nIE "$@" -e "$re" -- . "${EXCLUDE[@]}" || true)
  if [ -n "$out" ]; then
    out=$(printf '%s\n' "$out" | while IFS= read -r l; do
            t=${l#*:}; t=${t#*:}
            # blank allowed substrings (twice: a match consumes its boundary
            # character), then re-test the category on what is left
            t=$(printf '%s\n' "$t" | sed -E "s/$allow_re/ /gI; s/$allow_re/ /gI")
            if printf '%s\n' "$t" | grep -qE "$@" -e "$re"; then printf '%s\n' "$l"; fi
          done)
  fi
  [ -n "$out" ] || return 0
  printf '== %s ==\n%s\n' "$name" "$out"
  fail=1
}

scan 'IPv4 address'         "$IPV4"
scan '.ts.net name'         "$TSNET" -i
scan 'serial/UART path'     "$SERIAL"
scan 'user@host ssh target' "$SSHTGT"
scan 'MAC address'          "$MAC"
scan 'credential'           "$CRED" -i

[ "$fail" -eq 0 ] && echo "check-no-farm-ids: clean"
exit "$fail"
