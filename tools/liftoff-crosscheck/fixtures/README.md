# Cross-check fixtures

Inputs of the planner-versus-libliftoff cross-check (`docs/libliftoff-crosscheck.md`).
The tests of this crate read these files and never `docs/`.

Every fixture starts with `# ` comment lines naming its source file, the commit it was copied
from (`git log -1 --format=%h -- <file>`), the observation date and the origin.

| Fixture | Source | Commit | Date | Origin |
|---|---|---|---|---|
| `bcm2711-cm4.probe.txt` | `docs/platform-capabilities.md`, probe block | 472cc75 | 2026-11-28 | observed |
| `bcm2712.probe.txt` | `docs/platform-capabilities.md`, probe block | 5d30922 | 2026-11-28 | observed |
| `rk3588.probe.txt` | `tests/data/rk3588/topology-2026-11-28.md`, probe block | db71077 | 2026-11-28 | observed |
| `stm32mp257.probe.txt` | `docs/platform-capabilities.md`, probe block | 20df182 | 2026-11-28 | observed |
| `imx95-synthetic.xcheck` | `crates/composition-drm/src/profile.rs`, test `dpu95_stream` | 99d965d | 2026-11-28 | synthetic |

The four `.probe.txt` files hold the text of the probe block byte for byte after the header
(the first six lines); a reader skips lines starting with `# `. Truncations in the source
are noted in each header: the probe prints at most 32 formats per plane, the bcm2711-cm4 and
bcm2712 documents give the formats lines once or omit them, and the planes of the Broadcom
dumps are probe's cut view. A converter must refuse or flag a dump whose format lines are
absent rather than invent them.

`imx95-synthetic.xcheck` is in xcheck v1 format. It is not read from a board and carries no
hardware claim; it imitates the plane stream of the `DriverProfile::Imx95Dpu` unit test.
The probe fixtures give topology of boards at the stated date only; none of them is a hardware
claim about any other board or kernel.
