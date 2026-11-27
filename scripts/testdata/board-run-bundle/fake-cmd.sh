#!/bin/sh
# Fake board program for board-run-bundle.test.sh: prints plausible output
# with things the scrubber must remove.
echo "plane 33 TEST_ONLY ok"
echo "ssh target 10.9.8.7"
echo "serial number: SERIALXYZ"
echo "console /dev/ttyUSB3"
echo "mac aa:bb:cc:dd:ee:ff"
echo "stderr line" >&2
