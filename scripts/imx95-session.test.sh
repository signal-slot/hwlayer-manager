#!/usr/bin/env bash
# Host test for imx95-session.sh with a fake ssh/scp/systemctl first on PATH.
# The fake ssh logs its arguments and runs the remote command on the host,
# with the session state files and rm mapped into a fake root under $TMPDIR.
# Nothing leaves the host.
# Run: bash scripts/imx95-session.test.sh
here=$(cd "$(dirname "$0")" && pwd)
script=$here/imx95-session.sh
root=$(mktemp -d "${TMPDIR:-/tmp}/session-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok()  { echo "ok   $1"; }
bad() { echo "FAIL $1"; fail=1; }
check() { local n=$1; shift; if "$@"; then ok "$n"; else bad "$n"; fi; }

bin=$root/bin
fake=$root/fake        # the board's / (only its tmp/ is used)
log=$root/ssh.log
mkdir -p "$bin" "$fake/tmp"

cat >"$bin/ssh" <<EOT
#!/bin/sh
# fake ssh: ... <dest> <command>
for last; do :; done
cmd=\$last
# like real ssh: refuse an empty destination (the argument before the command)
eval "dst=\\\${\$(( \$# - 1 ))}"
[ -n "\$dst" ] || { echo "ssh: empty destination" >&2; exit 255; }
shift \$(( \$# - 1 ))
echo "ssh \$*" >>"$log"
printf 'CMD %s\n----\n' "\$cmd" >>"$log"
PATH=$root/rbin:$bin:\$PATH sh -c "\$(printf '%s' "\$cmd" | sed 's#/tmp/imx95-session#$fake/tmp/imx95-session#g')"
EOT
cat >"$bin/scp" <<EOT
#!/bin/sh
echo "scp \$*" >>"$log"
exit 0
EOT
# fake systemctl: state of the unit is kept in $fake/svc
cat >"$bin/systemctl" <<EOT
#!/bin/sh
st=$fake/svc
[ -f "\$st" ] || echo inactive >"\$st"
echo "systemctl \$*" >>"$root/systemctl.log"
case \$1 in
cat) exit 0 ;;
is-active)
    if [ "\$2" = --quiet ]; then [ "\$(cat \$st)" = active ]; exit \$?; fi
    cat "\$st"; [ "\$(cat \$st)" = active ] ;;
stop) echo inactive >"\$st" ;;
start) echo active >"\$st" ;;
*) exit 0 ;;
esac
EOT
# fake rm: maps /tmp/ arguments into the fake root, so the manifest's literal
# /tmp paths (read at run time) hit the fake tree
realrm=$(command -v rm)
mkdir -p "$root/rbin"   # on PATH only inside the fake ssh
cat >"$root/rbin/rm" <<EOT
#!/bin/sh
for a; do shift; case \$a in /tmp/*) a=$fake\$a ;; esac; set -- "\$@" "\$a"; done
exec $realrm "\$@"
EOT
chmod +x "$bin/ssh" "$bin/scp" "$bin/systemctl" "$root/rbin/rm"

run() { PATH=$bin:$PATH bash "$script" "$@"; }
reset() { : >"$log"; rm -f "$root/systemctl.log";rm -rf "$fake/tmp"; mkdir -p "$fake/tmp"; rm -f "$fake/svc"; }
sshcalls() { grep -c '^ssh ' "$log"; }
no_start_logged() { ! grep -q '^systemctl start' "$root/systemctl.log" 2>/dev/null; }

# --- usage errors -----------------------------------------------------------
export IMX95_SSH=board
for args in "" "bogus" "deploy extra" "cleanup extra" "run" "run x" "run x --" "run x ls" \
            "display-owner" "display-owner bogus" "display-owner stop extra" "--out"; do
    reset
    # shellcheck disable=SC2086
    run $args >/dev/null 2>&1; rc=$?
    check "usage error rc=2: '$args'" [ $rc -eq 2 ]
    check "usage error runs no ssh: '$args'" [ "$(sshcalls)" = 0 ]
done
reset; run -h >/dev/null 2>&1; check "-h exits 0" [ $? -eq 0 ]
run run 'bad name' -- true >/dev/null 2>&1; check "bad run name fails" [ $? -ne 0 ]
run run x -- fw_setenv a b >/dev/null 2>&1; check "run refuses fw_setenv" [ $? -ne 0 ]
run run x -- dd of=/dev/mmcblk0boot0 >/dev/null 2>&1; check "run refuses mmcblk0boot" [ $? -ne 0 ]
check "refused runs send nothing" [ "$(sshcalls)" = 0 ]

# --- missing IMX95_SSH ------------------------------------------------------
reset
env -u IMX95_SSH PATH=$bin:$PATH bash "$script" display-owner status >"$root/o" 2>&1; rc=$?
check "missing IMX95_SSH fails" [ $rc -ne 0 ]
check "missing IMX95_SSH message" grep -q 'IMX95_SSH is not set' "$root/o"
check "missing IMX95_SSH runs no ssh" [ "$(sshcalls)" = 0 ]

# --- dry run ----------------------------------------------------------------
reset
for sub in "deploy" "display-owner stop" "display-owner start" "display-owner status" \
           "run t -- uname -a" "cleanup" "display-owner stop --then true"; do
    # shellcheck disable=SC2086
    env -u IMX95_SSH PATH=$bin:$PATH bash "$script" --dry-run --out "$root/logs" $sub >"$root/o" 2>&1; rc=$?
    check "dry-run '$sub' exits 0" [ $rc -eq 0 ]
    check "dry-run '$sub' prints DRY steps" grep -q '^DRY' "$root/o"
done
check "dry-run runs no ssh or scp" [ ! -s "$log" ]
check "dry-run writes nothing in fake /tmp" [ -z "$(ls -A "$fake/tmp")" ]
check "dry-run run creates no log dir" [ ! -e "$root/logs" ]
env -u IMX95_SSH PATH=$bin:$PATH bash "$script" --dry-run deploy >"$root/o" 2>&1
check "dry-run deploy shows both builds" [ "$(grep -c 'cross-aarch64.sh' "$root/o")" = 2 ]
check "dry-run shows placeholder destination" grep -q '<IMX95_SSH>' "$root/o"

# --- display-owner stop / start ---------------------------------------------
reset; echo active >"$fake/svc"
run display-owner stop >"$root/o" 2>&1; rc=$?
check "stop exits 0" [ $rc -eq 0 ]
check "stop stopped the unit" [ "$(cat "$fake/svc")" = inactive ]
check "stop recorded was=active" grep -qx 'was=active' "$fake/tmp/imx95-session.display"
run display-owner start >"$root/o" 2>&1; rc=$?
check "start exits 0 (was active)" [ $rc -eq 0 ]
check "start restarted the unit" [ "$(cat "$fake/svc")" = active ]
check "start removed the state file" [ ! -e "$fake/tmp/imx95-session.display" ]

reset; echo inactive >"$fake/svc"
run display-owner stop >/dev/null 2>&1
check "stop of inactive unit records was=inactive" grep -qx 'was=inactive' "$fake/tmp/imx95-session.display"
run display-owner start >"$root/o" 2>&1; rc=$?
check "start exits 0 (was inactive)" [ $rc -eq 0 ]
check "start does not start a unit that was inactive" [ "$(cat "$fake/svc")" = inactive ]
check "start of inactive unit never calls systemctl start" no_start_logged
check "state file removed (was inactive)" [ ! -e "$fake/tmp/imx95-session.display" ]

reset; echo active >"$fake/svc"
run display-owner stop >/dev/null 2>&1
run display-owner stop >"$root/o" 2>&1
check "second stop keeps the first record" grep -qx 'was=active' "$fake/tmp/imx95-session.display"
run display-owner start >/dev/null 2>&1
check "start after double stop restarts" [ "$(cat "$fake/svc")" = active ]

reset
run display-owner start >"$root/o" 2>&1; rc=$?
check "start with no record exits 0" [ $rc -eq 0 ]
check "start with no record says nothing to restore" grep -q 'nothing to restore' "$root/o"
check "start with no record does not start" no_start_logged

reset; echo active >"$fake/svc"
run display-owner stop --then bash -c "echo during=\$(cat $fake/svc) >$root/during" >/dev/null 2>&1; rc=$?
check "stop --then exits 0" [ $rc -eq 0 ]
check "--then ran while stopped" [ "$(cat "$root/during")" = during=inactive ]
check "--then restored the display" [ "$(cat "$fake/svc")" = active ]

# --- cleanup and run --------------------------------------------------------
reset
mkdir -p "$fake/tmp/sub"
touch "$fake/tmp/probe" "$fake/tmp/drm-harness" "$fake/tmp/keep-me" "$fake/tmp/sub/x"
printf '%s\n' /tmp/probe /tmp/drm-harness /tmp/sub/x /tmp/../etc/y /etc/passwd >"$fake/tmp/imx95-session.files"
run cleanup >"$root/o" 2>&1; rc=$?
check "cleanup exits 0" [ $rc -eq 0 ]
check "cleanup removed manifest entries" [ ! -e "$fake/tmp/probe" ] && [ ! -e "$fake/tmp/drm-harness" ]
check "cleanup kept a file not in the manifest" [ -e "$fake/tmp/keep-me" ]
check "cleanup skipped nested path" [ -e "$fake/tmp/sub/x" ]
check "cleanup removed the manifest" [ ! -e "$fake/tmp/imx95-session.files" ]
reset
run cleanup >"$root/o" 2>&1
check "cleanup with no manifest says nothing recorded" grep -q 'nothing recorded' "$root/o"

reset
run --out "$root/logs" run t1 -- echo "it's ok" >"$root/o" 2>&1; rc=$?
check "run exits 0" [ $rc -eq 0 ]
check "run saves stdout" grep -q "it's ok" "$root/logs/t1.stdout"
check "run header has exit_status=0" grep -qx 'exit_status=0' "$root/logs/t1.header"
run --out "$root/logs" run t2 -- false >/dev/null 2>&1; rc=$?
check "run propagates failing exit" [ $rc -eq 1 ]

# --- every remote path written is under /tmp --------------------------------
reset; echo active >"$fake/svc"
run display-owner stop >/dev/null 2>&1; run display-owner status >/dev/null 2>&1
run display-owner start >/dev/null 2>&1; run cleanup >/dev/null 2>&1
env -u IMX95_SSH PATH=$bin:$PATH bash "$script" --dry-run deploy >"$root/dry" 2>&1
# targets of redirections, rm, chmod and mkdir in the remote commands
bad_paths=$(cat "$log" "$root/dry" | grep -oE '(>>?|rm -f|chmod [0-9]+|mkdir -p) +"?/[^ ;"]*' \
    | grep -oE '/[^ ;"]*$' | grep -v '^/tmp/' | grep -v '^/dev/null$')
check "all written remote paths are under /tmp" [ -z "$bad_paths" ]
check "wrote at least one /tmp path (sanity)" grep -q '/tmp/imx95-session' "$log"
check "nothing escaped the fake root" [ -z "$(ls -A "$fake" | grep -vE '^(tmp|svc)$')" ]
forbidden() { cat "$log" "$root/dry" | grep -qE 'fw_setenv|saveenv|mmcblk0boot|opkg|apt-get'; }
check "no forbidden board operations issued" bash -c "$(declare -f forbidden); log=$log root=$root; ! forbidden"

echo
if [ $fail -eq 0 ]; then echo "all passed"; else echo "FAILED"; fi
exit $fail
