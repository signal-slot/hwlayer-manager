# Fixture matrix

| Item | AM62P | i.MX95 |
| --- | --- | --- |
| Planes | 2 planes (observed (2026-12-01), `docs/observed/am62p.md`) | 7 (source, `PC`) |
| Formats | RGB565 and ARGB8888 (SoW §5.2) | many |
| Scaling | not recorded | upscale only (inferred) |
| Core | R5F (read from source, `TP`) | M7 (repo) |

Text between tables is ignored.

| Item | AM62P (R5F) | i.MX95 (M7) |
| --- | --- | --- |
| Latch | follows the GO bit (`TP`) | sync busy |
