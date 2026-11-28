#!/bin/sh
# am62p-board-info.sh - read-only board/BSP/kernel/panel identity as a markdown table.
# Run ON the board (Torizon OS). See am62p-board-info-README.md.
#   sh am62p-board-info.sh [--root DIR]
# --root DIR reads /etc, /proc and /sys (and /lib/firmware) below DIR (tests).
# Never printed: serial numbers, MAC/IP addresses, host name, machine-id.
root=
while [ $# -gt 0 ]; do
  case $1 in
    --root)
      [ $# -ge 2 ] || { echo "--root needs a directory" >&2; exit 2; }
      root=${2%/}
      shift 2 ;;
    -h|--help) sed -n 2,6p "$0"; exit 0 ;;
    *) echo "usage: $0 [--root DIR]" >&2; exit 2 ;;
  esac
done

HN=
for f in "$root/etc/hostname" "$root/proc/sys/kernel/hostname"; do
  if [ -r "$f" ]; then
    HN=$(head -n 1 "$f" 2>/dev/null)
    break
  fi
done
HNPAT=
[ -n "$HN" ] && HNPAT=$(printf '%s' "$HN" | sed 's/[][\.*^$/]/\\&/g')

# Remove anything that looks like a MAC, an IPv4/IPv6 address, a machine-id
# (32 hex) or a serial token, then the host name; escape table pipes.
scrub() {
  sed -e 's/\([0-9A-Fa-f][0-9A-Fa-f][:-]\)\{5\}[0-9A-Fa-f][0-9A-Fa-f]/<redacted>/g' \
      -e 's/[0-9]\{1,3\}\(\.[0-9]\{1,3\}\)\{3\}/<redacted>/g' \
      -e 's/\([0-9A-Fa-f]\{1,4\}:\)\{3,7\}[0-9A-Fa-f]\{1,4\}/<redacted>/g' \
      -e 's/[0-9A-Fa-f]\{32\}/<redacted>/g' \
      -e 's/\([Ss]erial[^ =:]*[ =:]\{1,2\}\)[^ ]*/\1<redacted>/g' \
      -e 's/|/\\|/g' |
    if [ -n "$HNPAT" ]; then sed -e "s/$HNPAT/<redacted>/g"; else cat; fi
}

# row KEY VALUE
row() { printf '| %s | %s |\n' "$1" "$(printf '%s' "$2" | tr -d '\000' | tr '\n' ' ' | scrub)"; }
first() { [ -r "$1" ] && head -n 1 "$1" 2>/dev/null | tr -d '\000'; }

osr=$root/etc/os-release
[ -r "$osr" ] || osr=$root/usr/lib/os-release
osval() { [ -r "$osr" ] && sed -n "s/^$1=//p" "$osr" | head -n 1 | sed -e 's/^"//' -e 's/"$//'; }

echo "| Item | Value |"
echo "| --- | --- |"
for k in PRETTY_NAME NAME VERSION VERSION_ID BUILD_ID; do
  v=$(osval $k)
  [ -n "$v" ] && row "os-release $k" "$v"
done
row "kernel release (uname -r)" "$(first "$root/proc/sys/kernel/osrelease")"
row "kernel version (uname -v)" "$(first "$root/proc/sys/kernel/version")"
# device-tree strings are NUL-separated; serial-number is never read
row "device-tree model" "$(first "$root/proc/device-tree/model")"
if [ -r "$root/proc/device-tree/compatible" ]; then
  row "device-tree compatible" "$(tr '\000' ';' < "$root/proc/device-tree/compatible" | sed 's/;$//; s/;/; /g')"
fi

# monitor name (descriptor tag 0xFC) from an EDID blob; serial (0xFF) is ignored
edid_name() {
  [ -s "$1" ] || return 0
  od -An -v -tx1 -w1 "$1" 2>/dev/null | tr -d ' ' | awk '
    { b[NR-1] = $1 }
    END {
      for (o = 54; o <= 108; o += 18)
        if (b[o] == "00" && b[o+1] == "00" && b[o+2] == "00" && b[o+3] == "fc") {
          s = ""
          for (i = 5; i < 18; i++) {
            c = b[o+i]
            if (c == "0a" || c == "") break
            s = s sprintf("%c", strtonum("0x" c))
          }
          print s
          exit
        }
    }' 2>/dev/null
}

n=0
for c in "$root"/sys/class/drm/card*-*; do
  [ -d "$c" ] || continue
  n=$((n+1))
  base=${c##*/}
  st=$(first "$c/status")
  modes=$(head -n 3 "$c/modes" 2>/dev/null | tr '\n' ' ')
  name=$(edid_name "$c/edid")
  row "drm $base" "status: ${st:-n/a}; modes: ${modes:-none}; edid name: ${name:-none}"
done
[ $n -gt 0 ] || row "drm connectors" "none found"

n=0
for r in "$root"/sys/class/remoteproc/remoteproc*; do
  [ -d "$r" ] || continue
  n=$((n+1))
  base=${r##*/}
  nm=$(first "$r/name")
  stt=$(first "$r/state")
  fw=$(first "$r/firmware")
  row "remoteproc $base" "name: ${nm:-n/a}; state: ${stt:-n/a}; firmware: ${fw:-n/a}"
  case $nm in
    *r5f*|*R5F*)
      if [ -n "$fw" ] && [ -L "$root/lib/firmware/$fw" ]; then
        row "remoteproc $base firmware link" "$fw -> $(readlink "$root/lib/firmware/$fw")"
      elif [ -n "$fw" ]; then
        row "remoteproc $base firmware link" "$fw (not a symlink or absent)"
      fi ;;
  esac
done
[ $n -gt 0 ] || row "remoteproc" "none found"
