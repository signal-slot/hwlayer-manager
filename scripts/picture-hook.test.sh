#!/bin/sh
# Self-test for picture-hook.sh with a mocked capture command.
# Run: sh scripts/picture-hook.test.sh
# Builds fixtures under a mktemp directory inside $TMPDIR.

here=$(cd "$(dirname "$0")" && pwd)
hook=$here/picture-hook.sh
root=$(mktemp -d "${TMPDIR:-.}/pichook-test.XXXXXX") || exit 2
trap 'rm -rf "$root"' EXIT HUP INT TERM
fail=0
ok() { echo "ok: $1"; }
bad() { echo "FAIL: $1"; fail=1; }

# Mock capture: a solid colour 8x8 binary PPM (camera-check reads P6).
cat > "$root/capture.sh" <<'M'
#!/bin/sh
printf 'P6\n8 8\n255\n' > "$1"
i=0
while [ $i -lt 64 ]; do printf '\310\024\024' >> "$1"; i=$((i + 1)); done
M
printf '#!/bin/sh\nexit 3\n' > "$root/capture-fail.sh"
mkdir "$root/exp"
# The picture is red (200,20,20): PASS against red, FAIL against blue.
mk_exp() {
	cat > "$root/exp/$1.json" <<J
{"rotation": 0, "panel": {"corners": [[0,0],[7,0],[7,7],[0,7]]},
 "regions": [{"name": "all", "rect": [0.2, 0.2, 0.8, 0.8], "expect": [$2]}]}
J
}
mk_exp PASSROW "200, 20, 20"
mk_exp FAILROW "20, 20, 200"

export PICTURE_EXPECT_DIR=$root/exp PICTURE_DELAY=0.3
# run_hold NAME ROW HOLD_SECONDS CAPTURE: start, hold, end; status of the end.
run_hold() {
	export PICTURE_DIR=$root/$1 PICTURE_CAPTURE=$4
	sh "$hook" "$2" hold-start || echo "hold-start status $?"
	sleep "$3"
	sh "$hook" "$2" hold-end
}
last() { tail -n 1 "$root/$1/pictures.log"; }

run_hold pass PASSROW 1 "sh $root/capture.sh"; rc=$?
[ "$rc" -eq 0 ] && ok "PASS exit 0" || bad "PASS exit $rc"
case $(last pass) in
"PASSROW hold-start "[0-9]*Z" $root/pass/PASSROW-hold-start-1.png PASS") ok "PASS logged" ;;
*) bad "PASS line: $(last pass)" ;;
esac
[ -s "$root/pass/PASSROW-hold-start-1.png" ] && ok "picture kept" || bad "no picture"

run_hold fail FAILROW 1 "sh $root/capture.sh"; rc=$?
[ "$rc" -eq 1 ] && ok "FAIL exit 1" || bad "FAIL exit $rc"
case $(last fail) in *" FAIL") ok "FAIL logged" ;; *) bad "FAIL line: $(last fail)" ;; esac

run_hold none NOEXP 1 "sh $root/capture.sh"; rc=$?
[ "$rc" -eq 0 ] && ok "no EXPECT exit 0" || bad "no EXPECT exit $rc"
case $(last none) in *" not judged") ok "not judged logged" ;; *) bad "none line: $(last none)" ;; esac

# Hold shorter than the delay: the picture must not appear later.
PICTURE_DELAY=1 run_hold short PASSROW 0 "sh $root/capture.sh"; rc=$?
[ "$rc" -eq 1 ] && ok "missed exit 1" || bad "missed exit $rc"
case $(last short) in *"picture missed"*) ok "picture missed logged" ;; *) bad "short line: $(last short)" ;; esac
sleep 1.5
set -- "$root"/short/*.png
[ -e "$1" ] && bad "picture exists after missed hold" || ok "no picture after missed hold"
[ "$(wc -l < "$root/short/pictures.log")" -eq 1 ] && ok "one log line" || bad "extra log lines"

run_hold cfail PASSROW 1 "sh $root/capture-fail.sh"; rc=$?
[ "$rc" -eq 1 ] && ok "capture failure exit 1" || bad "capture failure exit $rc"
case $(last cfail) in *"capture failed (status 3)") ok "capture failure logged" ;; *) bad "cfail line: $(last cfail)" ;; esac

# Immediate picture on a listed phase; other phases do nothing.
export PICTURE_DIR=$root/phase PICTURE_CAPTURE="sh $root/capture.sh" PICTURE_ON_PHASES="foo,bar"
sh "$hook" PASSROW bar; rc=$?
if [ "$rc" -eq 0 ] && grep -q '^PASSROW bar .* PASS$' "$root/phase/pictures.log"; then
	ok "PICTURE_ON_PHASES picture"; else bad "on-phase picture (rc $rc)"; fi
sh "$hook" PASSROW other
[ "$(wc -l < "$root/phase/pictures.log")" -eq 1 ] && ok "unlisted phase ignored" || bad "unlisted phase logged"

# Environment errors.
PICTURE_CAPTURE= sh "$hook" A1 hold-start 2>/dev/null; rc=$?
[ "$rc" -eq 2 ] && ok "missing capture exit 2" || bad "missing capture exit $rc"

exit $fail
