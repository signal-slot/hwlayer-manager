#!/usr/bin/env bash
# Host-side helper for short sessions on the Verdin iMX95 board.
# See scripts/imx95-session-README.md.
#
#   IMX95_SSH=<ssh destination> scripts/imx95-session.sh [--dry-run] [--out DIR] <subcommand>
#
#   deploy                          cross-build probe + drm-harness, scp to /tmp on the board
#   display-owner stop [--then CMD...]   stop the compositor (record whether it was active)
#   display-owner start             restore it only if it was active, wait until active
#   display-owner status
#   run <name> -- <cmd...>          run a command on the board, save header/stdout/stderr
#   cleanup                         remove only the files this script put in /tmp
#
# The ssh destination comes only from $IMX95_SSH (never written to a file).
# The script writes only under /tmp on the board, never installs packages,
# never touches the U-Boot environment or the eMMC boot partitions.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

# Files on the board (all under /tmp).
MANIFEST=/tmp/imx95-session.files      # paths this script put in /tmp
WSTATE=/tmp/imx95-session.display      # unit and prior state of the display owner
DEPLOY_TARGETS=(probe drm-harness)

dry=0
out=./board-logs

usage() {
    sed -n '2,15p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//' >&2
    exit "${1:-2}"
}
die() { echo "imx95-session: $*" >&2; exit 1; }
log() { echo "imx95-session: $*" >&2; }

# Single-quote one word for the board's shell.
sq() { printf "'%s'" "${1//\'/\'\\\'\'}"; }

ssh_opts=(-o BatchMode=yes -o ConnectTimeout=10)
# shellcheck disable=SC2206
[ -n "${IMX95_SSH_OPTS:-}" ] && ssh_opts+=(${IMX95_SSH_OPTS})

dest() {
    if [ -n "${IMX95_SSH:-}" ]; then printf '%s' "$IMX95_SSH"
    elif [ "$dry" = 1 ]; then printf '<IMX95_SSH>'
    else die "IMX95_SSH is not set (ssh destination of the board, from the lease)"; fi
}

# remote <shell script text>: run on the board (stdout passed through).
remote() {
    # dest dies inside $(...), which only ends that subshell: check here, in
    # the main shell, so ssh is never started with an empty destination.
    [ "$dry" = 1 ] || [ -n "${IMX95_SSH:-}" ] || dest
    if [ "$dry" = 1 ]; then
        echo "DRY ssh ${ssh_opts[*]} $(dest) $(sq "$1")"
    else
        ssh "${ssh_opts[@]}" "$(dest)" "$1"
    fi
}

# --- display owner ---------------------------------------------------------

# Remote shell fragment: find the unit (override with $IMX95_DISPLAY_UNIT).
# Prefers weston.service, then any weston*.service known to systemd.
unit_sh() {
    cat <<EOS
unit=$(sq "${IMX95_DISPLAY_UNIT:-}")
if [ -z "\$unit" ]; then
  if systemctl cat weston.service >/dev/null 2>&1; then unit=weston.service
  else unit=\$(systemctl list-unit-files --type=service --no-legend 2>/dev/null | awk '{print \$1}' | grep -i '^weston' | head -n1); fi
fi
[ -n "\$unit" ] || { echo 'no weston systemd unit found' >&2; exit 3; }
EOS
}

display_stop() {
    # An existing state file means an earlier stop is still pending: keep its
    # record, do not overwrite "was active" with the now-inactive state.
    remote "set -eu
$(unit_sh)
if [ -f $WSTATE ]; then
  echo 'display owner already stopped by an earlier invocation; keeping its record'
else
  was=\$(systemctl is-active \"\$unit\" || true)
  printf 'unit=%s\nwas=%s\n' \"\$unit\" \"\$was\" > $WSTATE
fi
. $WSTATE
echo \"unit=\$unit was=\$was\"
if systemctl is-active --quiet \"\$unit\"; then systemctl stop \"\$unit\"; fi
echo \"after stop: \$(systemctl is-active \"\$unit\" || true)\"
if command -v fuser >/dev/null 2>&1; then fuser -v /dev/dri/card* 2>&1 || true; fi"
}

display_start() {
    remote "set -eu
if [ ! -f $WSTATE ]; then echo 'no stop recorded (nothing to restore)'; exit 0; fi
. $WSTATE
if [ \"\$was\" != active ]; then
  echo \"\$unit was \$was before the session: not starting it\"
  rm -f $WSTATE; exit 0
fi
systemctl start \"\$unit\" || true
i=0
while [ \$i -lt 30 ]; do
  if systemctl is-active --quiet \"\$unit\"; then
    echo \"\$unit is active again\"; rm -f $WSTATE; exit 0
  fi
  i=\$((i+1)); sleep 1
done
echo \"\$unit NOT active after 30 s: \$(systemctl is-active \"\$unit\" || true)\" >&2
systemctl status \"\$unit\" --no-pager 2>&1 | tail -n 15 >&2 || true
exit 1"
}

display_status() {
    remote "set -eu
$(unit_sh)
echo \"unit=\$unit active=\$(systemctl is-active \"\$unit\" || true)\"
if [ -f $WSTATE ]; then echo 'recorded stop pending:'; cat $WSTATE; else echo 'no stop recorded'; fi"
}

cmd_display_owner() {
    local sub=${1:-}
    [ $# -gt 0 ] && shift
    case $sub in
    stop)
        if [ "${1:-}" = --then ]; then
            shift
            [ $# -gt 0 ] || die "--then needs a command"
            # Restore on any exit of this invocation once stop was called.
            trap 'display_start || log "WARNING: display owner NOT restored; run: display-owner start"' EXIT
            display_stop
            if [ "$dry" = 1 ]; then echo "DRY local: $*"; else "$@"; fi
        else
            [ $# -eq 0 ] || usage
            display_stop
            log "display stays stopped; run 'display-owner start' when done"
        fi ;;
    start)  display_start ;;
    status) display_status ;;
    *) usage ;;
    esac
}

# --- deploy / run / cleanup -------------------------------------------------

cmd_deploy() {
    local f p i=0
    local paths=()
    # The manifest entry goes in before the copy, so a copy that fails half
    # way still leaves a file that cleanup knows about.
    if [ "$dry" = 1 ]; then
        echo "DRY $root/scripts/cross-aarch64.sh -p composition-drm --example probe"
        echo "DRY $root/scripts/cross-aarch64.sh -p composition-drm-harness --bin drm-harness"
        for f in "${DEPLOY_TARGETS[@]}"; do
            remote "grep -qxF /tmp/$f $MANIFEST 2>/dev/null || echo /tmp/$f >> $MANIFEST"
            echo "DRY scp ${ssh_opts[*]} <built $f> $(dest):/tmp/$f"
            remote "chmod 755 /tmp/$f"
        done
        return 0
    fi
    # Build first, so a build failure leaves the board untouched.
    p=$("$root/scripts/cross-aarch64.sh" -p composition-drm --example probe)
    paths+=("$(printf '%s\n' "$p" | grep '/probe$' | tail -n1)")
    p=$("$root/scripts/cross-aarch64.sh" -p composition-drm-harness --bin drm-harness)
    paths+=("$(printf '%s\n' "$p" | grep '/drm-harness$' | tail -n1)")
    for p in "${paths[@]}"; do [ -x "$p" ] || die "build output missing: '$p'"; done
    for f in "${DEPLOY_TARGETS[@]}"; do
        remote "grep -qxF /tmp/$f $MANIFEST 2>/dev/null || echo /tmp/$f >> $MANIFEST"
        scp "${ssh_opts[@]}" "${paths[$i]}" "$(dest):/tmp/$f"
        remote "chmod 755 /tmp/$f"
        i=$((i + 1))
    done
    log "deployed: ${DEPLOY_TARGETS[*]} (in /tmp on the board)"
}

cmd_run() {
    local name=${1:-}
    [ $# -gt 0 ] && shift
    [ "${1:-}" = -- ] || usage
    shift
    [ -n "$name" ] && [ $# -gt 0 ] || usage
    [[ $name =~ ^[A-Za-z0-9._-]+$ ]] || die "bad run name: $name"
    # Coarse guard for the board rules; not a sandbox.
    case " $* " in
    *fw_setenv*|*saveenv*|*mmcblk0boot*|*" opkg "*|*" dnf "*|*" apt "*|*apt-get*|*"pip install"*)
        die "refused: command touches U-Boot env, boot partitions or installs packages" ;;
    esac
    local cmd="" w
    for w in "$@"; do cmd+="$(sq "$w") "; done
    local hdr='echo "date_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"; echo "uname_r=$(uname -r)"
[ -r /etc/os-release ] && . /etc/os-release; echo "os_ID=${ID:-}"; echo "os_VERSION_ID=${VERSION_ID:-}"; echo "os_PRETTY_NAME=${PRETTY_NAME:-}"; echo "os_BUILD_ID=${BUILD_ID:-}"'
    if [ "$dry" = 1 ]; then
        echo "DRY mkdir -p $out"
        remote "$hdr"
        echo "DRY > $out/$name.header (+ command line, exit status)"
        echo "DRY ssh ${ssh_opts[*]} $(dest) $(sq "$cmd") > $out/$name.stdout 2> $out/$name.stderr"
        return 0
    fi
    mkdir -p "$out"
    {
        remote "$hdr"
        echo "command=$*"
    } > "$out/$name.header"
    local rc=0
    ssh "${ssh_opts[@]}" "$(dest)" "$cmd" > "$out/$name.stdout" 2> "$out/$name.stderr" || rc=$?
    echo "exit_status=$rc" >> "$out/$name.header"
    echo "end_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$out/$name.header"
    log "run $name: exit $rc, saved in $out/$name.{header,stdout,stderr}"
    return "$rc"
}

cmd_cleanup() {
    # Only paths this script recorded, and only directly under /tmp.
    remote "set -eu
if [ -f $MANIFEST ]; then
  while IFS= read -r f; do
    case \$f in
      /tmp/*/*|*..*) echo \"skip \$f\" >&2 ;;
      /tmp/?*) rm -f \"\$f\" && echo \"removed \$f\" ;;
    esac
  done < $MANIFEST
  rm -f $MANIFEST
else echo 'nothing recorded'; fi
if [ -f $WSTATE ]; then echo 'NOTE: display owner stop still pending; run display-owner start' >&2; fi"
}

# --- main ---------------------------------------------------------------------

while [ $# -gt 0 ]; do
    case $1 in
    --dry-run) dry=1; shift ;;
    --out) [ $# -ge 2 ] || usage; out=$2; shift 2 ;;
    -h|--help) usage 0 ;;
    *) break ;;
    esac
done
[ $# -gt 0 ] || usage
sub=$1; shift
case $sub in
deploy)        [ $# -eq 0 ] || usage; cmd_deploy ;;
display-owner) cmd_display_owner "$@" ;;
run)           cmd_run "$@" ;;
cleanup)       [ $# -eq 0 ] || usage; cmd_cleanup ;;
*)             usage ;;
esac
