# am62p-console

- host-run.txt: console (SCENE and RESULT lines) of the AM62P board test
  run on the host behind a fake panel, recorded 2026-11-28. HOST RUN, not a
  board result. Source: `cargo test -p composition-twoplane board_scenes --
  --nocapture`, filtered to lines starting with SCENE or RESULT. The first
  line is a comment with the date and commit; the check ignores it.
  `scripts/am62p-host-console.test.sh` regenerates it and diffs.
