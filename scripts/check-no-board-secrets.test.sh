#!/bin/sh
# Self-test for check-no-board-secrets.sh.
# Run: scripts/check-no-board-secrets.test.sh
# Builds a throwaway git repository under $TMPDIR (mktemp default directory
# if unset) and never touches the real repository.

here=$(cd "$(dirname "$0")" && pwd)
script=$here/check-no-board-secrets.sh
root=$(mktemp -d "${TMPDIR:-.}/nbs-test.XXXXXX") || exit 2
case $root in /*) ;; *) root=$(cd "$root" && pwd) ;; esac
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
repo=$root/r

# Fake values are assembled from pieces so this file itself stays clean.
ip="10.1.2.""3"
mac="aa:bb:cc:dd:ee:""ff"
tty="/dev/tty""USB0"
byid="/dev/serial/by-""id/usb-x"
uh="root@""board7"
key="-----BEGIN OPENSSH ""PRIVATE KEY-----"
pw="pass""word=hunter2"
tk="to""ken=abc123"

# run NAME EXPECTED_STATUS FILE CONTENT [COMMIT_MESSAGE]
run() {
    name=$1 want=$2 file=$3 content=$4 msg=${5:-change}
    rm -rf "$repo"
    mkdir -p "$repo"
    (
        cd "$repo" || exit 2
        git init -q -b main . &&
            git config user.email t@example.invalid &&
            git config user.name t &&
            echo base >README && git add README && git commit -q -m base &&
            git checkout -q -b topic &&
            mkdir -p "$(dirname "$file")" &&
            printf '%s\n' "$content" >"$file" &&
            git add . && git commit -q -m "$msg" &&
            sh "$script" main >"$root/out" 2>&1
    )
    got=$?
    if [ "$got" -eq "$want" ]; then
        echo "ok   $name"
    else
        echo "FAIL $name: want $want got $got"
        cat "$root/out" 2>/dev/null
        fail=1
    fi
}

run clean 0 a.txt "nothing to see; version 1.2.3"
run ipv4 1 a.txt "board at $ip"
run ipv4-loopback 0 a.txt "bind 127.0.0.1 and 0.0.0.0"
run ipv4-cargo-lock 0 Cargo.lock "version = \"1.2.3.4\""
run ipv4-vendor 0 vendor/x/a.txt "v 1.2.3.4"
run ipv4-marker 0 a.txt "example $ip secrets-ok"
run tty 1 a.txt "console $tty"
run serial-by-id 1 a.txt "console $byid"
run mac 1 a.txt "eth $mac"
run user-at-host 1 a.txt "ssh $uh"
run user-at-host-allowed 0 a.txt "clone git@github.com:org/repo.git"
run ssh-key 1 a.txt "$key"
run password 1 a.txt "$pw"
run token 1 a.txt "$tk"
run placeholder 0 a.txt 'token=$TOKEN password=<secret>'
run msg-ipv4 1 a.txt ok "subject

board $ip"
run msg-trailer 0 a.txt ok "subject

Co-Authored-By: Name <noreply@anthropic.com>"

# Hits are reported as file:line.
run line 1 d/a.txt "x
$ip"
if grep -q 'd/a.txt:2' "$root/out"; then echo "ok   file:line"; else echo "FAIL file:line"; cat "$root/out"; fail=1; fi

# Unknown base is a usage error.
(cd "$repo" && sh "$script" nosuchref >/dev/null 2>&1)
if [ $? -eq 2 ]; then echo "ok   bad-base"; else echo "FAIL bad-base"; fail=1; fi

exit $fail
