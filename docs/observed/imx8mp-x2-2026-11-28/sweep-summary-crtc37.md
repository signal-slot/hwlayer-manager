# kms_sweep and scale_probe summary, CRTC 37 (LCDIF3, HDMI-A-1)

Output of `python3 -I scripts/sweep-summary.py sweep-crtc37.txt
sweep-scale-crtc37.txt`, pasted unchanged below the line. Observed
2026-11-28 on the Armadillo-X2 (`imx-drm`, kernel `6.12.105-0-at`). SoW §5.3
evidence only; it counts toward none of M1, M2 or M3. A `TEST_ONLY`
acceptance is not proof of on-screen composition.

Reading notes (observed 2026-11-28):

- The two rows of "Scale limits per plane" are two `scale_probe` runs on the
  same plane. The first is the default destination (960x540 at 0,0): even
  factor 1 is refused (`EINVAL`), so no limit could be measured. The second
  is `--dst 1920x1080`: only factor 1 is accepted, every up- and downscale is
  refused (`ERANGE`).
- "Combination results per k" has only k=1, because CRTC 37 has one
  candidate plane (the primary, 35). No overlay could be tried.
- In "Modifier acceptance" every `addfb=EINVAL` is ADDFB2 refusing the
  `DRM_MODE_FB_MODIFIERS` flag that `kms_sweep modifiers` always sets for
  the linear modifier. The driver reports `DRM_CAP_ADDFB2_MODIFIERS` = 0
  (x2-probe). These rows say nothing about the formats themselves:
  `plane_commit` (no modifier flag) had `XR24` and `AR24` accepted, see
  `sweep-plane-commit-testonly-crtc37.txt`. That the EINVAL comes from the
  flag is inferred from the example's source and the cap, not read from
  the kernel.

CRTC 34 (LCDIF2, LVDS-1) has no summary: `kms_sweep` needs a connected
connector with modes and LVDS-1 is forced off (`sweep-crtc34.txt`).

---

### Scale limits per plane

| plane | kind | format | max upscale h | max upscale v | max upscale both | downscale | min downscale |
|---|---|---|---|---|---|---|---|
| 35 | Primary | XR24 | none | none | none | rejected | none |
| 35 | Primary | XR24 | 1.0000 | 1.0000 | 1.0000 | rejected | none |

### Combination results per k

| k | planes | scale | result |
|---|---|---|---|
| 1 | 35 | 1.0000x1.0000 | ok |

### Accepted combination maximum

| accepted k |
|---|
| 1 |

### Modifier acceptance

| plane | format | modifier | listed | addfb | test_only | flag |
|---|---|---|---|---|---|---|
| 35 | AB24 | 0x0000000000000000 | no | EINVAL | not reached |  |
| 35 | AR15 | 0x0000000000000000 | no | EINVAL | not reached |  |
| 35 | AR24 | 0x0000000000000000 | no | EINVAL | not reached |  |
| 35 | RG16 | 0x0000000000000000 | no | EINVAL | not reached |  |
| 35 | XB24 | 0x0000000000000000 | no | EINVAL | not reached |  |
| 35 | XR15 | 0x0000000000000000 | no | EINVAL | not reached |  |
| 35 | XR24 | 0x0000000000000000 | no | EINVAL | not reached |  |
