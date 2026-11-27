# Calibration frames for the libliftoff cross-check

Small hand-made inputs in the `xcheck 1` format of `docs/libliftoff-crosscheck.md` section 5. Each `<name>.xcheck` has a sibling `<name>.expect`.

## What these are

- **Synthetic.** Every file says `origin synthetic 2026-11-28 ...`. They imitate generic plane stacks; they describe no board and give **no hardware claim**. They never count toward M1, M2 or M3.
- **Hand-written.** The topologies (2 to 4 planes, one output) and frames were written for this directory. No libliftoff test code was copied.
- **Inside the output.** Every layer lies inside the 640x480 output, since v1 does not clip.
- **The `.expect` files are predictions (inferred), not results.** Written 2026-11-28 before any run. They give, per frame, the predicted outcome class (section 6 classes: `both-accept`, `both-reject`, `core-only`, `liftoff-only`, ...) for `model=structural` and `model=full`, and name the section 7 row each file calibrates. `a|b` means the design cannot decide between two classes; the run is to settle it. A frame marked `control` should agree on both sides.

## Files

| File | Section 7 row | Predicted interest |
| --- | --- | --- |
| `zpos-tie.xcheck` | zpos tie rule | equal `z` stacks in frame order; both accept |
| `zpos-disjoint-inverted.xcheck` | zpos and intersection | non-overlapping layers needing out-of-order planes: liftoff-only |
| `zpos-programmable.xcheck` | Programmable zpos | range planes with `zpos_now`: core-only |
| `alpha-zero.xcheck` | Invisible layers | alpha-0 layers dropped by libliftoff |
| `format-absent.xcheck` | Format/modifier absent; Format check skipped | both reject |
| `all-planes.xcheck` | Search effort (smallest case) | a frame that uses every plane; both accept |
| `more-layers.xcheck` | Contract | more layers than planes: partial allocation counts as rejected |
| `scaled-group.xcheck` | Knowledge gap, scaling/groups | explicit `scaling` and `group` lines under `profile generic` |

## Use

Later items (xc-sweep, xc-report) run these through the cross-check and compare the observed class with the `.expect` line. A mismatch is a lead to triage (expected, artefact or lead), not a failure of the file. Nothing here has been run.

The `.expect` format is plain text for people: one line per frame, `<frame> structural=<class> full=<class>`, `#` comments. It is not part of `xcheck 1`; the reader need not parse it.

The files contain no addresses or host names.
