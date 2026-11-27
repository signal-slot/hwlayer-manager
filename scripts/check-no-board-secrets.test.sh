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
phuh="user@""host"
hexuh="root@""dead"
macdash="aa-bb-cc-dd-ee-""ff"
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

# False-positive rules: each allowed shape passes, and the nearest real
# pattern is still flagged. Hosts are placeholders (example.invalid).
run uses-line 0 w.yml "        uses: actions/checkout@v4"
run uses-list-item 0 w.yml "      - uses: owner/repo/sub@0123abcd  # pin"
run uses-prose-ref 0 a.txt "the job uses actions/checkout@v4 only"
run run-line-flagged 1 w.yml "        run: ssh $uh true"
run uses-text-in-run-flagged 1 w.yml "        run: echo uses: x/y@v1 && ssh $uh"
run placeholder-user-host 0 a.txt "use ssh $phuh for the board"
run placeholder-example 0 a.txt "mail t@example.invalid"
run commit-pin 0 a.txt "read from source (imx-sm@af1e3702)."
run dt-unit-address 0 a.txt "node port@0/1 and phy@c and channel@N"
run systemd-instance 0 a.txt "getty@tty1 serial-getty@ttyAMA10 modprobe@drm"
run lore-message-id 0 a.txt "see https://lore.kernel.org/list/AbCd1234@some-host/t/"
run hex-host-after-ssh-flagged 1 a.txt "ssh $hexuh true"
run hex-host-with-domain-flagged 1 a.txt "login $hexuh.corp"
run ml-freedesktop 0 a.txt "mail dri-devel@lists.freedesktop.org today"
run ml-mail-archive 0 a.txt "https://www.mail-archive.com/dri-devel@lists.freedesktop.org/msg1.html"
run ml-vger-end 0 a.txt "linux-kernel@vger.kernel.org."
run ml-subdomain-flagged 1 a.txt "root@""lists.freedesktop.org.corp"
run ml-lookalike-flagged 1 a.txt "root@""vger.kernel.org.evil"
run ml-private-flagged 1 a.txt "root@""lists.board7.corp"
run ml-ssh-flagged 1 a.txt "ssh dri-devel@lists.freedesktop.org true"
run ml-ip-still-flagged 1 a.txt "dri-devel@lists.freedesktop.org $ip"
run mac-time-range 0 a.txt "19:44:22-19:45:12 UTC"
run mac-dash 1 a.txt "eth $macdash"
run tty-rpmsg 0 a.txt "cat /dev/ttyRPMSG0"
run tty-fixture-elsewhere-flagged 1 scripts/other.test.sh "echo $tty"
run fixture-zephyr-allowed 0 scripts/check-zephyr-apps.test.sh "echo $tty"

# Hits are reported as file:line.
run line 1 d/a.txt "x
$ip"
if grep -q 'd/a.txt:2' "$root/out"; then echo "ok   file:line"; else echo "FAIL file:line"; cat "$root/out"; fail=1; fi

# Unknown base is a usage error.
(cd "$repo" && sh "$script" nosuchref >/dev/null 2>&1)
if [ $? -eq 2 ]; then echo "ok   bad-base"; else echo "FAIL bad-base"; fail=1; fi

# No-argument base selection. Throwaway repos; the topic branch has no
# origin/main. MAINREF=main creates a local main, anything else none.
nrun() {
    name=$1 want=$2 mainref=$3 content=$4 grepfor=$5
    rm -rf "$repo"
    mkdir -p "$repo"
    (
        cd "$repo" || exit 2
        git init -q -b trunk . &&
            git config user.email t@example.invalid &&
            git config user.name t &&
            echo base >README && git add README && git commit -q -m base &&
            if [ "$mainref" = main ]; then git branch main; fi &&
            git checkout -q -b topic &&
            printf '%s\n' "$content" >a.txt &&
            git add . && git commit -q -m change &&
            sh "$script" >"$root/out" 2>&1
    )
    got=$?
    if [ "$got" -eq "$want" ] && grep -q "$grepfor" "$root/out"; then
        echo "ok   $name"
    else
        echo "FAIL $name: want $want ($grepfor) got $got"
        cat "$root/out" 2>/dev/null
        fail=1
    fi
}
nrun noarg-local-main 0 main "nothing to see" "using base main"
nrun noarg-local-main-hit 1 main "board at $ip" "ipv4: a.txt:1"
nrun noarg-full-scan-clean 0 none "nothing to see" "full scan of HEAD"
nrun noarg-full-scan-hit 1 none "board at $ip" "ipv4: a.txt:1"
# Full scan also covers content that predates the branch.
(
    cd "$repo" && echo "console $tty" >old.txt && git add . && git commit -q -m old &&
        sh "$script" >"$root/out" 2>&1
)
if [ $? -eq 1 ] && grep -q 'tty: old.txt:1' "$root/out"; then echo "ok   full-scan-old-line"; else echo "FAIL full-scan-old-line"; cat "$root/out"; fail=1; fi

# check-no-farm-ids.sh: it scans tracked files, so each case builds a
# throwaway repository holding a copy of the script plus one file.
farm=$here/check-no-farm-ids.sh
frun() {
    name=$1 want=$2 file=$3 content=$4
    rm -rf "$repo"
    mkdir -p "$repo/scripts" "$repo/$(dirname "$file")"
    cp "$farm" "$repo/scripts/"
    printf '%s\n' "$content" >>"$repo/$file"
    (
        cd "$repo" || exit 2
        git init -q . && git add . && bash scripts/check-no-farm-ids.sh >"$root/out" 2>&1
    )
    got=$?
    if [ "$got" -eq "$want" ]; then
        echo "ok   farm-$name"
    else
        echo "FAIL farm-$name: want $want got $got"
        cat "$root/out" 2>/dev/null
        fail=1
    fi
}
uh2="root@""board7.example.invalid"
frun clean 0 a.txt "nothing to see; version 1.2.3"
frun uses-line 0 .github/workflows/w.yml "        uses: actions/checkout@v4"
frun ssh-target 1 a.txt "ssh $uh2"
frun ipv4 1 a.txt "board at $ip"
frun serial 1 a.txt "console $tty"
frun mac 1 a.txt "eth $mac"
frun credential 1 a.txt "$pw"
frun loopback 0 a.txt "bind 127.0.0.1"
frun noreply-trailer 0 a.txt "Co-Authored-By: N <noreply@anthropic.com>"
frun unit-instance 0 a.txt "systemd[1]: modprobe@drm.service: Deactivated"
# A fixture allowance is per file: the same text elsewhere is still reported.
frun fixture-other-file 1 a.txt "echo 192.168.""1.5"
frun fixture-own-file 0 scripts/check-zephyr-apps.test.sh "echo 192.168.""1.5"

exit $fail
