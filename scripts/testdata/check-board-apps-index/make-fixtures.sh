#!/bin/sh
# Regenerates the fixture trees next to this script (not used by the test).
cd "$(dirname "$0")" || exit 1
mk() { mkdir -p "$1/tests/board/$2"; : > "$1/tests/board/$2/CMakeLists.txt"; }
mk all app_a; mk all app_b
mkdir -p all/tests/board/boards/x; : > all/tests/board/boards/x/.gitkeep
printf '[app_a](app_a/README.md)\n`app_b`\nSee boards/README.md\n' > all/tests/board/README.md
mk missing app_a; mk missing app_b
printf 'app_a only\n' > missing/tests/board/README.md
mk prefix app; mk prefix app_long
printf '[app_long](app_long/)\n' > prefix/tests/board/README.md
mk noboards app_a
mkdir -p noboards/tests/board/boards/x; : > noboards/tests/board/boards/x/.gitkeep
printf 'app_a\nonboards here\n' > noboards/tests/board/README.md
mk nocmake app_a
mkdir -p nocmake/tests/board/other; : > nocmake/tests/board/other/.gitkeep
printf 'app_a\n' > nocmake/tests/board/README.md
mk noreadme app_a
mkdir -p nodir; : > nodir/.gitkeep
