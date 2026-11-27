#!/usr/bin/env bash
# Self-test for check-no-farm-ids.sh.
# Run: bash scripts/check-no-farm-ids.test.sh
# Each case builds a throwaway git repository under $TMPDIR holding a copy of
# the script plus one file, and never touches the real repository.  Fake values
# are assembled from pieces so this file itself stays clean.

here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-no-farm-ids.sh
root=$(mktemp -d "${TMPDIR:-/tmp}/nfi-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
repo=$root/r
fail=0

ip="10.1.2.3"
tsnet="board7.tail1234.ts"".net"
tsup=$(printf '%s' "$tsnet" | tr a-z A-Z)
tty="/dev/tty""USB0"
acm="/dev/tty""ACM1"
byid="/dev/serial/by-""id/usb-x"
uh="root@""board7.corp"
sshuh="ssh -p 22 root@""board7"
mac="aa:bb:cc:dd:ee:""ff"
pw="pass""word=hunter2"
tk="to""ken: abc123"

# frun NAME WANT FILE CONTENT [PATTERN-EXPECTED-IN-OUTPUT]
frun() {
    local name=$1 want=$2 file=$3 content=$4 pat=${5:-} got
    rm -rf "$repo"
    mkdir -p "$repo/scripts" "$repo/$(dirname "$file")"
    cp "$script" "$repo/scripts/"
    printf '%s\n' "$content" >>"$repo/$file"
    (cd "$repo" && git init -q . && git add . &&
        bash scripts/check-no-farm-ids.sh >"$root/out" 2>&1)
    got=$?
    if [ "$got" -eq "$want" ] && { [ -z "$pat" ] || grep -q -- "$pat" "$root/out"; }; then
        echo "ok   $name"
    else
        echo "FAIL $name: want $want ($pat) got $got"
        cat "$root/out"
        fail=1
    fi
}

frun clean 0 a.txt "nothing to see; version 1.2.3" "check-no-farm-ids: clean"
frun ipv4 1 a.txt "board at $ip" "== IPv4 address =="
frun ipv4-location 1 d/a.txt "x
$ip" "d/a.txt:2:"
frun tsnet 1 a.txt "host $tsnet" "== .ts.net name =="
frun tsnet-case 1 a.txt "host $tsup"
frun serial-usb 1 a.txt "console $tty" "== serial/UART path =="
frun serial-acm 1 a.txt "console $acm"
frun serial-by-id 1 a.txt "console $byid"
frun ssh-target 1 a.txt "$sshuh" "== user@host ssh target =="
frun user-at-fqdn 1 a.txt "mail $uh" "== user@host ssh target =="
frun mac 1 a.txt "eth $mac" "== MAC address =="
frun credential-password 1 a.txt "$pw" "== credential =="
frun credential-token 1 a.txt "$tk"
frun credential-quoted 1 a.txt "Pass""word = \"hunter2\""

# Allowlist: each allowed shape passes ...
frun loopback 0 a.txt "bind 127.0.0.1 and 0.0.0.0"
frun doc-range 0 a.txt "use 192.0.2.7 or 198.51.100.9 or 203.0.113.4"
frun version-like 0 a.txt "kernel 6.6.157 and BSP 7.8.0"
frun example-domain 0 a.txt "mail t@example.org"
frun noreply-trailer 0 a.txt "Co-Authored-By: N <noreply@anthropic.com>"
frun unit-instance 0 a.txt "systemd[1]: modprobe@drm.service: Deactivated"
frun mailing-list 0 a.txt "dri-devel@lists.freedesktop.org"
frun git-remote 0 a.txt "git@github.com:org/repo.git"
frun uses-line 0 .github/workflows/w.yml "        uses: actions/checkout@v4"
frun placeholder-cred 0 a.txt 'password=changeme token=null'
frun env-cred 0 a.txt 'token=$TOKEN password=<secret>'
frun rpmsg-tty 0 a.txt "cat /dev/ttyRPMSG0"
# ... but never hides another identifier on the same line.
frun allow-not-hiding-ip 1 a.txt "bind 127.0.0.1 then $ip"
frun allow-not-hiding-ssh 1 a.txt "dri-devel@lists.freedesktop.org $sshuh"

# Fixtures are per file and per string.
frun fixture-other-file 1 a.txt "echo 192.168.""1.5"
frun fixture-own-file 0 scripts/check-zephyr-apps.test.sh "echo 192.168.""1.5"
frun fixture-wrong-string 1 scripts/check-zephyr-apps.test.sh "echo $ip"

# Exclusions: the lockfile is never scanned.
frun excluded-lockfile 0 Cargo.lock "version = \"1.2.3.4\""

# Several categories in one file: every category is reported, exit is 1.
frun multi 1 a.txt "$ip
$tty" "== serial/UART path =="
if grep -q '== IPv4 address ==' "$root/out"; then echo "ok   multi-ipv4"; else echo "FAIL multi-ipv4"; fail=1; fi

# Only tracked files are scanned.
rm -rf "$repo"
mkdir -p "$repo/scripts"
cp "$script" "$repo/scripts/"
echo "board at $ip" >"$repo/untracked.txt"
(cd "$repo" && git init -q . && git add scripts && bash scripts/check-no-farm-ids.sh >"$root/out" 2>&1)
if [ $? -eq 0 ]; then echo "ok   untracked-ignored"; else echo "FAIL untracked-ignored"; fail=1; fi

# Runs from a subdirectory of the repository.
(cd "$repo/scripts" && bash check-no-farm-ids.sh >"$root/out" 2>&1)
if [ $? -eq 0 ]; then echo "ok   from-subdir"; else echo "FAIL from-subdir"; fail=1; fi

exit $fail
