#!/bin/sh
# Check that every public declaration in the C API header has a doc comment.
#
# Usage: sh scripts/check-header-docs.sh [header]
#
# Declarations checked: functions (ce_* and any other), typedefs, and enums
# (named, or anonymous: reported by their first enumerator). A declaration is
# documented when a /* ... */ comment (including /** ... */) ends directly
# above it: no blank line, no preprocessor line and no other code in between.
# A comment trailing code on the same line does not count for the next one.
# Not checked: #define macros, struct/enum bodies, variables.
#
# Prints "<kind> <name> (line N)" for each undocumented declaration and exits
# 1 if there are any; exits 0 when all are documented; exits 2 on usage errors.
# POSIX sh and awk only (dash, busybox). The header is only read.
set -u
here=$(cd "$(dirname "$0")" && pwd)
if [ $# -ge 1 ]; then
    hdr=$1
elif [ -f "$here/../include/composition_engine.h" ]; then
    hdr=$here/../include/composition_engine.h
else
    hdr=$here/../crates/composition-ffi/include/composition_engine.h
fi
if [ ! -r "$hdr" ]; then
    echo "check-header-docs: cannot read $hdr" >&2
    exit 2
fi

awk '
function isid(c) { return c ~ /[A-Za-z0-9_]/ }
function firstid(t,    q) {
	q = 0
	while (q < length(t) && isid(substr(t, q + 1, 1))) q++
	return substr(t, 1, q)
}
# identifier ending at position p (skipping spaces back from p)
function identend(s, p,    e) {
	while (p > 0 && substr(s, p, 1) == " ") p--
	e = p
	while (p > 0 && isid(substr(s, p, 1))) p--
	return substr(s, p + 1, e - p)
}
function finish(s, line, hasdoc,    name, kind, p, t) {
	gsub(/[ \t]+/, " ", s)
	sub(/^ /, "", s); sub(/ $/, "", s)
	name = ""
	if (s ~ /^typedef( |$)/) {
		kind = "typedef"
		p = index(s, "(*")
		if (p > 0) {
			t = substr(s, p + 2); sub(/^ +/, "", t)
			name = firstid(t)
		} else name = identend(s, length(s))
	} else if (s ~ /^(enum|struct|union)( |\{|$)/) {
		kind = s; sub(/[ {].*/, "", kind)
		t = s; sub(/^[a-z]+ */, "", t)
		if (t ~ /^[A-Za-z_]/) name = firstid(t)
		else if (kind == "enum") {
			t = body; sub(/^[ \t,]+/, "", t)
			name = "{" firstid(t) "...}"
		}
	} else if ((p = index(s, "(")) > 0) {
		kind = "function"
		name = identend(s, p - 1)
	} else return
	if (name == "") return
	total++
	if (!hasdoc) { printf "%s %s (line %d)\n", kind, name, line; bad++ }
}
BEGIN { incm = 0; doc = 0; depth = 0; stmt = ""; ppcont = 0 }
{
	line = $0
	startcm = incm
	code = ""; sawcm = 0; n = length(line)
	for (i = 1; i <= n; i++) {
		c = substr(line, i, 1)
		if (incm) {
			if (c == "*" && substr(line, i + 1, 1) == "/") { incm = 0; i++ }
		} else if (c == "/" && substr(line, i + 1, 1) == "*") { incm = 1; sawcm = 1; i++; code = code " " }
		else if (c == "/" && substr(line, i + 1, 1) == "/") { sawcm = 1; break }
		else code = code c
	}
	t = code; gsub(/[ \t\r]/, "", t)
	if (ppcont) { ppcont = (code ~ /\\[ \t\r]*$/); next }
	if (t == "") {
		if (sawcm) doc = (depth == 0 && stmt == "")
		else if (!incm && !startcm) doc = 0
		next
	}
	if (t ~ /^#/ && stmt == "" && depth == 0) { doc = 0; ppcont = (code ~ /\\[ \t\r]*$/); next }
	code = code " "
	m = length(code)
	for (i = 1; i <= m; i++) {
		c = substr(code, i, 1)
		if (depth == 0 && stmt == "") {
			if (c ~ /[ \t\r]/) continue
			if (c == "}" || c == ";") continue
			sdoc = doc; sline = NR; doc = 0; body = ""
		}
		if (c == "{") {
			if (depth == 0 && stmt ~ /^extern *"C" *$/) { stmt = ""; continue }
			if (depth == 0) hdr = stmt
			depth++
			if (depth == 1) stmt = stmt " {"
			continue
		}
		if (c == "}") {
			if (depth > 0) {
				depth--
				if (depth == 0) {
					stmt = hdr " {}"
					if (hdr ~ /\(/ && hdr !~ /^(typedef|struct|enum|union)/) { finish(hdr, sline, sdoc); stmt = "" }
				}
			}
			continue
		}
		if (depth > 0) { body = body c; continue }
		if (c == ";") { finish(stmt, sline, sdoc); stmt = ""; continue }
		stmt = stmt c
	}
}
END {
	if (bad > 0) {
		printf "check-header-docs: %d of %d declarations undocumented\n", bad, total > "/dev/stderr"
		exit 1
	}
	printf "check-header-docs: ok, %d declarations documented\n", total
}
' "$hdr"
