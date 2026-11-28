#!/bin/sh
# Hook for board-scenarios.sh --hook: delayed KVM picture, judged by
# camera-check.py. Called as `picture-hook.sh ROW PHASE`. See
# scripts/picture-hook-README.md.
#
# Environment:
#   PICTURE_CAPTURE     command called as `$PICTURE_CAPTURE OUTFILE` (required;
#                       the lease provides it, nothing device-specific here)
#   PICTURE_DIR         output directory (required)
#   PICTURE_DELAY       seconds into a hold before the picture (default 20)
#   PICTURE_EXPECT_DIR  directory of ROW.json EXPECT files (optional)
#   PICTURE_ON_PHASES   other phases (space or comma separated) that get an
#                       immediate picture (optional)
#   PICTURE_SHOTS       capture runs per picture, back to back; only the last
#                       image is kept and judged (default 2; a KVM can return
#                       a stale first frame). A positive integer.
#   PICTURE_WAIT       seconds hold-end waits for a running capture
#                       (default 120)
#
# Exit status: 0 hold-start started, or the picture was taken and judged PASS
# or not judged; 1 picture missed, capture failed or verdict FAIL/error;
# 2 usage or environment error.

here=$(cd "$(dirname "$0")" && pwd)
row=$1
phase=$2
if [ $# -ne 2 ] || [ -z "$row" ] || [ -z "$phase" ]; then
	echo "usage: picture-hook.sh ROW PHASE" >&2
	exit 2
fi
case $row$phase in *[!A-Za-z0-9_-]*)
	echo "picture-hook: ROW and PHASE may use letters, digits, _ and - only" >&2
	exit 2 ;;
esac
dir=${PICTURE_DIR:-}
if [ -z "$dir" ] || [ -z "${PICTURE_CAPTURE:-}" ]; then
	echo "picture-hook: PICTURE_DIR and PICTURE_CAPTURE must be set" >&2
	exit 2
fi
delay=${PICTURE_DELAY:-20}
wait_max=${PICTURE_WAIT:-120}
expdir=${PICTURE_EXPECT_DIR:-}
shots=${PICTURE_SHOTS:-2}
case $shots in ''|*[!0-9]*|0*)
	echo "picture-hook: PICTURE_SHOTS must be a positive integer" >&2
	exit 2 ;;
esac
mkdir -p "$dir" || exit 2

now() { date -u +%Y-%m-%dT%H:%M:%SZ; }
log() { # ROW PHASE FILE VERDICT
	printf '%s %s %s %s %s\n' "$1" "$2" "$(now)" "$3" "$4" >> "$dir/pictures.log"
}
state=$dir/.$row

# shoot ROW PHASE FILE: capture and judge, log one line. Status as exit.
shoot() {
	set -f
	shot=0
	while [ "$shot" -lt "$shots" ]; do
		# Each shot replaces the last one, so only the final image stays.
		rm -f "$3"
		# shellcheck disable=SC2086
		$PICTURE_CAPTURE "$3" >/dev/null 2>&1
		crc=$?
		shot=$((shot + 1))
		[ "$crc" -eq 0 ] || break
	done
	set +f
	if [ "$crc" -ne 0 ] || [ ! -s "$3" ]; then
		rm -f "$3"
		log "$1" "$2" - "capture failed (status $crc)"
		return 1
	fi
	if [ -n "$expdir" ] && [ -f "$expdir/$1.json" ]; then
		python3 -I "$here/camera-check.py" "$3" "$expdir/$1.json" \
			> "${3%.png}.check.txt" 2>&1
		jrc=$?
		case $jrc in
		0) log "$1" "$2" "$3" PASS; return 0 ;;
		1) log "$1" "$2" "$3" FAIL; return 1 ;;
		*) log "$1" "$2" "$3" "error (camera-check status $jrc)"; return 1 ;;
		esac
	fi
	log "$1" "$2" "$3" "not judged"
}

next_name() { # ROW PHASE
	n=0
	[ -f "$dir/.count" ] && read -r n < "$dir/.count"
	n=$((n + 1))
	echo "$n" > "$dir/.count"
	echo "$dir/$1-$2-$n.png"
}

case $phase in
hold-start)
	rm -rf "$state.claim" "$state.done" "$state.rc" "$state.pid"
	file=$(next_name "$row" "$phase")
	(
		trap '' HUP
		sleep "$delay"
		mkdir "$state.claim" 2>/dev/null || exit 0
		shoot "$row" "$phase" "$file"
		echo $? > "$state.rc"
		: > "$state.done"
	) >/dev/null 2>&1 </dev/null &
	echo $! > "$state.pid"
	exit 0 ;;
hold-end)
	if [ ! -f "$state.pid" ]; then
		log "$row" "$phase" - "picture missed (no hold-start)"
		exit 1
	fi
	if mkdir "$state.claim" 2>/dev/null; then
		# The job has not started its capture: the hold was shorter than
		# the delay. Claiming first means it can never fire later.
		read -r pid < "$state.pid"
		kill "$pid" 2>/dev/null
		rm -f "$state.pid"
		log "$row" "$phase" - "picture missed (hold shorter than ${delay}s)"
		exit 1
	fi
	waited=0
	while [ ! -f "$state.done" ]; do
		if [ "$waited" -ge $((wait_max * 10)) ]; then
			log "$row" "$phase" - "picture missed (capture still running after ${wait_max}s)"
			exit 1
		fi
		sleep 0.1
		waited=$((waited + 1))
	done
	rm -f "$state.pid"
	read -r rc < "$state.rc"
	exit "$rc" ;;
*)
	case " $(echo "${PICTURE_ON_PHASES:-}" | tr ',' ' ') " in
	*" $phase "*) shoot "$row" "$phase" "$(next_name "$row" "$phase")"; exit $? ;;
	esac
	exit 0 ;;
esac
