# libliftoff: licence, version and use in AGL / Weston (source read)

Status: **partly done, blocked on network** (2026-11-28). The libliftoff
source itself could **not** be fetched in this run, so the licence, release
and contribution-route rows below are **not read from source** in this item.
Only the Weston-tree and Toradex-layer greps are read from source. The
checklist at the end lists the reads still to do.

This doc states facts only. It draws no licensing conclusion; the owner decides.

## What was tried

- `git clone` of `https://gitlab.freedesktop.org/emersion/libliftoff.git`
  into the project dependency cache: refused (outbound network access needed
  approval, which was not granted in this unattended run).
- `curl` of the snapshot archive of the pinned commit: refused the same way.
- No libliftoff checkout exists in the dependency cache from earlier runs.
- The AGL cache directory exists but is **empty** (read from source,
  2026-11-28: listing shows no entries), so the AGL layers were not grepped.

## Pin (from existing docs, not re-checked)

| Item | Value | Tag |
|---|---|---|
| Repository | `https://gitlab.freedesktop.org/emersion/libliftoff.git` | named in `docs/libliftoff-crosscheck.md` |
| Pinned commit | `c4226a79b7a52f59bb51789b1eda112658609125` (`c4226a7`), committed 2026-08-24, `meson.build` version `0.6.0-dev` | recalled from `docs/libliftoff-crosscheck.md` (read there 2026-11-28); not re-read here |
| Licence | MIT | recalled from `docs/libliftoff-crosscheck.md`; the LICENSE file, SPDX headers and the meson `license:` field were not read in this item |
| Current release tag | unknown | not measured (no tag listing obtained) |
| Contribution route (CONTRIBUTING, README) | unknown | not read |

## Weston and Toradex layers: read from source (2026-11-28)

- Tree: `weston-imx-nxp` in the dependency cache. Its `meson.build` says
  `version: '12.0.5'` (read from source, 2026-11-28). Its commit id was not
  recorded here because `git` against the cache needed approval; the pin is
  in `docs/weston-imx-lease.md` and the item that reads NXP's changes.
- A recursive, case-insensitive search for `liftoff` over the whole
  `weston-imx-nxp` tree gave **no match** (read from source, 2026-11-28).
  That Weston 12.0.5 NXP tree neither builds against nor mentions libliftoff.
- The same search over the `meta-toradex-nxp` tree in the cache gave **no
  match** (read from source, 2026-11-28): that layer neither has a libliftoff
  recipe nor patches it. Scope: that one clone only, not other Toradex layers.
- Upstream Weston (other versions) and AGL's layers (`meta-agl-drm-lease`,
  the AGL Weston recipe): **not read**. Whether AGL uses libliftoff at all is
  **not measured** here.

## What the licence facts mean for an offer from this project

Only what the existing docs state: libliftoff is recorded there as MIT, and
this project is Apache-2.0/MIT, so the recorded licence of the target is one
this project already uses. Whether an offer is made under MIT, Apache-2.0 or
both, and whether libliftoff's own LICENSE or SPDX text adds anything, is
**not decided here**; it needs the reads below. See
`docs/wp5-licence-options.md` for the compatibility note and
`docs/upstream-contribution-policy-read.md` for DCO and AI policy (other
items; not repeated).

## Reads still to do (needs network)

1. Clone libliftoff; the checked-out id must be `c4226a7...`; read `LICENSE`,
   the SPDX headers, and the `license:` field of `meson.build`.
2. List tags, newest first: current release, and whether the pin is a tag or
   a dev commit.
3. Read `README.md` and `CONTRIBUTING*` only for licensing lines.
4. Clone AGL's layers and the Weston recipe they build; search for `liftoff`.
