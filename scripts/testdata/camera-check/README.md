# camera-check test fixtures (2026-11-28)

Used by `scripts/camera-check.test.sh`. All synthetic; no camera capture, no board data.

- `gen.py`: writes the 200x100 synthetic pictures (PPM or PNG) at test time.
- `zorder-b-top.json`: two solid planes, blue on top in the overlap.
- `move.json`: two-picture compare (plane B moved right).
- `no-checks.json`: expectation with nothing to check (input error).
- `bad.json`: not valid JSON (input error).
