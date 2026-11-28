#!/usr/bin/env bash
# rk3588-session-run.sh: one-pass NanoPC-T6 board session runner.
#
# Usage: rk3588-session-run.sh [--dry-run] --bin-dir DIR --out DIR
#                              (--step "EXAMPLE ARGS..." | --steps FILE)...
#
# Environment:
#   BOARD_SSH   ssh target (required unless --dry-run). Never printed and
#               never written to a file; the output files only hold what the
#               examples print.
#   BOARD_SUDO  optional command prefix for the board side (e.g. "sudo -n"),
#               applied to display-guard and to each example.
#
# Steps: EXAMPLE is one of probe props kms_sweep scale_probe plane_commit
# pending_probe zpos_probe twoplane mode_test; ARGS are passed through. A
# --steps FILE has one step per line; blank lines and # comments are skipped.
#
# Flow: make a fresh directory under the board's /tmp, copy the binaries used
# and scripts/display-guard.sh there, then for each step run
# `display-guard.sh stop`, the example, `display-guard.sh restore`, and
# capture stdout, stderr and the exit code into OUT/NN-EXAMPLE.txt under a
# host-UTC header. The board directory is removed at the end (also on error)
# and whether the display manager is active again is printed.
#
# It never reboots, loads modules or changes DRM, udev, seat or boot config.
set -u

here=$(cd "$(dirname "$0")" && pwd)
guard_src=$here/display-guard.sh
known=" probe props kms_sweep scale_probe plane_commit pending_probe zpos_probe twoplane mode_test "

dry=0
bindir=
outdir=
steps=()

die() { echo "rk3588-session-run: $*" >&2; exit 2; }

add_step() {
	local name=${1%% *}
	case $known in *" $name "*) ;; *) die "unknown example '$name'" ;; esac
	steps+=("$1")
}

while [ $# -gt 0 ]; do
	case $1 in
	--dry-run) dry=1; shift ;;
	--bin-dir) [ $# -ge 2 ] || die "--bin-dir needs a value"; bindir=$2; shift 2 ;;
	--out) [ $# -ge 2 ] || die "--out needs a value"; outdir=$2; shift 2 ;;
	--step) [ $# -ge 2 ] || die "--step needs a value"; add_step "$2"; shift 2 ;;
	--steps)
		[ $# -ge 2 ] || die "--steps needs a file"
		[ -f "$2" ] || die "no such steps file: $2"
		while IFS= read -r line || [ -n "$line" ]; do
			case $line in '' | '#'*) continue ;; esac
			add_step "$line"
		done < "$2"
		shift 2 ;;
	-h | --help) sed -n '2,24p' "$0"; exit 0 ;;
	*) die "unknown argument '$1'" ;;
	esac
done

[ -n "$bindir" ] || die "--bin-dir is required"
[ -n "$outdir" ] || die "--out is required"
[ ${#steps[@]} -gt 0 ] || die "no steps given"
[ -f "$guard_src" ] || die "display-guard.sh not found next to this script"
if [ "$dry" = 0 ]; then
	[ -n "${BOARD_SSH:-}" ] || die "BOARD_SSH is not set"
fi

# Binaries used, checked on the host (also in dry-run).
used=()
for s in "${steps[@]}"; do
	n=${s%% *}
	[ -x "$bindir/$n" ] || die "missing or not executable: $bindir/$n"
	case " ${used[*]:-} " in *" $n "*) ;; *) used+=("$n") ;; esac
done

sudo_prefix=${BOARD_SUDO:-}

utc() { date -u +%Y-%m-%dT%H:%M:%SZ; }

# Board-side script for one step: $1 = board dir, $2 = example, $3 = args.
remote_step() {
	local d=$1 ex=$2 args=$3
	printf 'cd %s || exit 97\n' "$d"
	printf '%s sh ./display-guard.sh stop %s/dg.state; echo "GUARD-STOP-EXIT=$?"\n' "$sudo_prefix" "$d"
	printf '%s ./%s %s; echo "EXAMPLE-EXIT=$?"\n' "$sudo_prefix" "$ex" "$args"
	printf '%s sh ./display-guard.sh restore %s/dg.state; echo "GUARD-RESTORE-EXIT=$?"\n' "$sudo_prefix" "$d"
}

if [ "$dry" = 1 ]; then
	echo "DRY-RUN plan (nothing is executed, no ssh)"
	echo "ssh \"\$BOARD_SSH\" 'mktemp -d /tmp/rk3588-session.XXXXXX'   # -> RDIR"
	echo "copy (tar over ssh) to \$RDIR: display-guard.sh ${used[*]}"
	i=0
	for s in "${steps[@]}"; do
		i=$((i + 1))
		ex=${s%% *}
		args=
		[ "$s" != "$ex" ] && args=${s#* }
		printf 'step %02d -> %s/%02d-%s.txt\n' "$i" "$outdir" "$i" "$ex"
		remote_step '$RDIR' "$ex" "$args" | sed 's/^/  | /'
	done
	echo "cleanup: ssh \"\$BOARD_SSH\" \"rm -rf \$RDIR\""
	echo "report: display manager active again (from display-guard restore)"
	exit 0
fi

mkdir -p "$outdir" || die "cannot create $outdir"
sshc=(ssh -o BatchMode=yes "$BOARD_SSH")

RDIR=$("${sshc[@]}" 'mktemp -d /tmp/rk3588-session.XXXXXX') || die "cannot reach the board"
case $RDIR in /tmp/rk3588-session.*) ;; *) die "unexpected board directory" ;; esac

cleanup() {
	if "${sshc[@]}" "rm -rf '$RDIR'" >/dev/null 2>&1; then
		echo "cleanup: board session directory removed"
	else
		echo "cleanup: FAILED to remove board session directory" >&2
	fi
}
trap cleanup EXIT

tar -cf - -C "$bindir" "${used[@]}" -C "$here" display-guard.sh \
	| "${sshc[@]}" "tar -xf - -C '$RDIR'" || die "copy to board failed"

rc=0
guard_ok=1
i=0
for s in "${steps[@]}"; do
	i=$((i + 1))
	ex=${s%% *}
	args=
	[ "$s" != "$ex" ] && args=${s#* }
	f=$(printf '%s/%02d-%s.txt' "$outdir" "$i" "$ex")
	{
		echo "# DATE $(utc) (host UTC)"
		echo "# STEP $i: $ex $args"
	} > "$f"
	remote_step "$RDIR" "$ex" "$args" | "${sshc[@]}" 'sh -s' >> "$f" 2>&1
	echo "# END $(utc) (host UTC)" >> "$f"
	ex_rc=$(sed -n 's/^EXAMPLE-EXIT=//p' "$f" | tail -n1)
	gr_rc=$(sed -n 's/^GUARD-RESTORE-EXIT=//p' "$f" | tail -n1)
	printf 'step %02d %s: exit=%s guard-restore=%s -> %s\n' "$i" "$ex" "${ex_rc:-?}" "${gr_rc:-?}" "$f"
	[ "${ex_rc:-1}" = 0 ] || rc=1
	[ "${gr_rc:-1}" = 0 ] || guard_ok=0
done

if [ "$guard_ok" = 1 ]; then
	echo "display manager: active again (every guard restore reported active, or none was running)"
else
	echo "display manager: NOT confirmed active again; check the step files" >&2
	rc=1
fi
exit $rc
