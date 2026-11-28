#!/bin/sh
# Self-test of check-header-docs.sh with fixture headers written under $TMPDIR.
set -u
here=$(cd "$(dirname "$0")" && pwd)
chk=$here/check-header-docs.sh
tmp=$(mktemp -d "${TMPDIR:-.}/hdocs.XXXXXX") || exit 2
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
rc=0
bad() {
    echo "FAIL: $*" >&2
    rc=1
}

# expect NAME WANT_RC WANT_NAMES...: run on $tmp/NAME.h; every listed name
# must be in the output (WANT_RC 1) and no other declaration may be reported.
expect() {
    name=$1 want=$2
    shift 2
    out=$(sh "$chk" "$tmp/$name.h" 2>/dev/null)
    got=$?
    [ "$got" -eq "$want" ] || bad "$name: exit $got, wanted $want"
    for n in "$@"; do
        case $out in
        *" $n (line "*) ;;
        *) bad "$name: $n not reported" ;;
        esac
    done
    cnt=$(printf '%s\n' "$out" | grep -c '(line ')
    [ "$cnt" -eq "$#" ] || bad "$name: reported $cnt names, wanted $#: $out"
}

cat >"$tmp/good.h" <<'EOF'
/* file comment */
#ifndef X_H_
#define X_H_
#ifdef __cplusplus
extern "C" {
#endif
#define MAXV 4 \
	/* continued */

/** A status code. */
typedef int32_t ce_status;

/*
 * A struct.
 */
typedef struct ce_rect {
	int32_t x; /* trailing */
	int32_t y;
} ce_rect;

/* Opaque. */
typedef struct ce_engine ce_engine;

/* Callback. */
typedef int (*ce_cb)(void *user,
		     uint32_t id);

/* Kinds. */
enum {
	K_A = 0,
	K_B = 1,
};

/* Named. */
enum ce_named { N_A, N_B };

/** Does a thing. */
ce_status ce_init(ce_engine *engine,
		  uint32_t flags);

// line comment doc
int ce_two(void);
#ifdef __cplusplus
}
#endif
#endif
EOF
expect good 0

cat >"$tmp/bad.h" <<'EOF'
/* Documented. */
int ce_ok(void);
int ce_none(void);

/* Blank line after the comment. */

int ce_gap(int a,
	   int b);
/* Documented after an undocumented one. */
int ce_after(void);
int ce_trailing(void); /* trailing comment does not document the next */
int ce_next(void);
typedef struct ce_s {
	int a;
} ce_s;
/* a */
#define M 1
typedef int ce_pp;
enum { E_FIRST, E_SECOND };
typedef void (*ce_fn)(int);
/* a */
enum ce_ok_enum { A };
EOF
expect bad 1 ce_none ce_gap ce_next ce_s ce_pp '{E_FIRST...}' ce_fn ce_trailing

cat >"$tmp/empty.h" <<'EOF'
#define ONLY_MACROS 1
EOF
expect empty 0

out=$(sh "$chk" "$tmp/missing.h" 2>&1)
got=$?
[ "$got" -eq 2 ] || bad "missing file: exit $got, wanted 2"

# dash and busybox, when installed, agree with sh on the bad fixture
for sh_ in dash busybox; do
    command -v "$sh_" >/dev/null 2>&1 || continue
    if [ "$sh_" = busybox ]; then runner="busybox sh"; else runner=$sh_; fi
    a=$($runner "$chk" "$tmp/bad.h" 2>/dev/null)
    b=$(sh "$chk" "$tmp/bad.h" 2>/dev/null)
    [ "$a" = "$b" ] || bad "$sh_ output differs"
done

if [ "$rc" -eq 0 ]; then echo "check-header-docs.test.sh: ok"; fi
exit "$rc"
