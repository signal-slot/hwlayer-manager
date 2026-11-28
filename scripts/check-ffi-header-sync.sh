#!/bin/sh
# Compare the function names exported by composition-ffi with the prototypes
# in the C header. Usage: check-ffi-header-sync.sh [--rust FILE] [--header FILE]
# Prints FFI-HEADER MISSING-IN-HEADER / MISSING-IN-RUST lines, then
# FFI-HEADER PASS (exit 0) or FFI-HEADER FAIL (exit 1; exit 2 on bad usage).
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
rust=$root/crates/composition-ffi/src/lib.rs
header=$root/crates/composition-ffi/include/composition_engine.h
while [ $# -gt 0 ]; do
	case $1 in
	--rust | --header)
		[ $# -ge 2 ] || { echo "missing value for $1" >&2; exit 2; }
		if [ "$1" = --rust ]; then rust=$2; else header=$2; fi
		shift 2
		;;
	*) echo "usage: $0 [--rust FILE] [--header FILE]" >&2; exit 2 ;;
	esac
done
[ -r "$rust" ] && [ -r "$header" ] || { echo "cannot read input files" >&2; exit 2; }

python3 -I -c '
import re, sys
rust = open(sys.argv[1], encoding="utf-8").read()
hdr = open(sys.argv[2], encoding="utf-8").read()

r = re.sub(r"/\*.*?\*/", "", rust, flags=re.S)
r = re.sub(r"//[^\n]*", "", r)
rnames = set(re.findall(r"\bextern\s+\"C\"\s+fn\s+([A-Za-z_]\w*)", r))
rnames |= set(re.findall(
    r"#\[\s*(?:unsafe\s*\(\s*)?no_mangle\s*\)?\s*\]\s*(?:pub(?:\([^)]*\))?\s+)?(?:unsafe\s+)?(?:extern\s+\"C\"\s+)?fn\s+([A-Za-z_]\w*)", r))

h = re.sub(r"/\*.*?\*/", "", hdr, flags=re.S)
h = re.sub(r"//[^\n]*", "", h)
h = re.sub(r"^[ \t]*#(?:[^\n]*\\\n)*[^\n]*", "", h, flags=re.M)
h = re.sub(r"extern\s+\"C\"\s*\{", "", h)
while True:
    n = re.sub(r"\{[^{}]*\}", "", h)
    if n == h:
        break
    h = n
h = h.replace("}", "")
hnames = set()
for stmt in h.split(";"):
    stmt = " ".join(stmt.split())
    if not stmt or stmt.startswith("typedef"):
        continue
    m = re.match(r"^[\w\s\*]*?\b([A-Za-z_]\w*)\s*\(", stmt)
    if m:
        hnames.add(m.group(1))

mh = sorted(rnames - hnames)
mr = sorted(hnames - rnames)
if mh:
    print("FFI-HEADER MISSING-IN-HEADER: " + " ".join(mh))
if mr:
    print("FFI-HEADER MISSING-IN-RUST: " + " ".join(mr))
ok = not mh and not mr and rnames
print("FFI-HEADER PASS" if ok else "FFI-HEADER FAIL")
sys.exit(0 if ok else 1)
' "$rust" "$header"
