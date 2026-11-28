# Weston's built-in DRM lease support (`wp_drm_lease_device_v1`): source-read status and board check (WP5)

Written 2026-11-28 for plan #470 item `weston-lease-src-read`. Companion to
`docs/weston-release-read.md` (plane assignment) and `docs/weston-imx-lease.md`
(NXP and AGL lease search). Neither file is edited here.

## 0. Status: only the 12.0.5 tree could be fetched

The item asked for a clone of `gitlab.freedesktop.org/wayland/weston` at a
release tag and a read of its DRM lease support. At first **no clone was
possible**; later one was (section 0a), so `12.0.5` is read first hand and
`16.0.2` is not.

- `git ls-remote --tags` on the Weston URL worked on 2026-11-28 and gave the
  tag list in section 1.
- Every command that writes a Weston tree locally was refused by the worker's
  permission layer on 2026-11-28 (`git clone --depth 1`, `git clone --filter`,
  `git fetch --depth 1` into a fresh `git init` directory, and `curl` of an
  archive or a raw file). The same refusal hit a `git fetch` from GitHub. This
  is a worker permission block, not a server error.
- A fetch of raw files over HTTPS through the web-fetch agent returned an
  Anubis bot-challenge page ("Access Denied") for all nine URLs tried, not
  file content.

So no line below is "read from source (2026-11-28, tag)" from this item. What
is stated comes from three places, each tagged:

- **read from source (other doc)**: a fact another doc of this repository
  recorded from its own read on 2026-11-28, cited by row. Not re-read here.
- **observed (2026-11-28)**: a command this item ran. Only `git ls-remote`.
- **not read** / **inferred**: as written.

Nothing in this item touched a board.

### 0a. Update: tag 12.0.5 was then read (2026-11-28)

After the permission layer allowed one clone, `git clone --depth 1 --branch
12.0.5` of the Weston URL succeeded (checkout `3d2cd9978c68c6338e902e9369266dbe8c4766c3`).
A second clone for `16.0.2` and a `git fetch` of that tag into the first clone
were still refused, so `16.0.2` stays "other doc" only. Read in the `12.0.5`
tree on 2026-11-28:

| # | Finding | Tag |
|---|---------|-----|
| T1 | A recursive case-insensitive whole-word search for `lease`, `leases`, `leasing`, `leased`, `drm-lease`, `drm_lease` over the tree (`.git` and `po` excluded) has **no hit** | read from source (2026-11-28, 12.0.5) |
| T2 | A search for `drm.?lease` and `wp_drm` has **no hit**. So neither `wp_drm_lease_device_v1` nor any `wp_drm_*` name occurs anywhere in the tree | read from source (2026-11-28, 12.0.5) |
| T3 | `protocol/` holds only Weston's own XML files (`ivi-application`, `ivi-hmi-controller`, `text-cursor-position`, `weston-content-protection`, `weston-debug`, `weston-desktop-shell`, `weston-direct-display`, `weston-output-capture`, `weston-test`, `weston-touch-calibration`) and `meson.build`. No lease XML. `protocol/meson.build:4` takes `wayland-protocols` `>= 1.31` as a dependency | read from source (2026-11-28, 12.0.5) |
| T4 | `libweston/backend-drm/` has `drm-gbm.c drm-internal.h drm-virtual.c drm.c fb.c kms-color.c kms.c libbacklight.[ch] meson.build modes.c state-helpers.c state-propose.c vaapi-recorder.[ch]`; no lease file | read from source (2026-11-28, 12.0.5) |
| T5 | The DRM backend reads the connector `non-desktop` property (`kms.c:168`, `modes.c:149-154`) and calls `weston_head_set_non_desktop` (`modes.c:608`). This marks a head; it does not lease anything | read from source (2026-11-28, 12.0.5) |
| T6 | `meson_options.txt` has 36 `option(` entries; no `lease` word in the file (T1) | read from source (2026-11-28, 12.0.5) |

T1 to T6 confirm K1 for `12.0.5` first hand. `16.0.2` and newer trees were
not read here.

## 1. Tag and commit

| Item | Value | Tag |
| --- | --- | --- |
| Tag chosen | `12.0.5`, tag object `0a82a10b7a7ce9f8d53353bd882cff45ad83ce01`, target commit `3d2cd9978c68c6338e902e9369266dbe8c4766c3` | observed (2026-11-28), `git ls-remote --tags https://gitlab.freedesktop.org/wayland/weston.git` |
| Why `12.0.5` | NXP's `weston-imx-12.0.5` is based on it (`docs/weston-imx-lease.md` rows L2, L3, S3), and the Verdin iMX95 image most likely builds that recipe (row L8, inferred there) | read from source (other doc) |
| Newest release tag | `16.0.2`, tag object `41d888493b03573ee455d3079bef57d8a1502f29`, target commit `530ae6c769d2fd55ea5157542654810be81a3193` | observed (2026-11-28), `git ls-remote`; matches `docs/weston-release-read.md` |
| Other releases listed | `13.0.4` (`6ae49e1d…`, target `d522890e…`), `14.0.3` (`e02e5157…`, target `d3038814…`), `15.0.2` (`c3c97480…`, target `de09947f…`). The AGL patches target 13.0.x (`docs/weston-imx-lease.md` row S7) | observed (2026-11-28), `git ls-remote` |

The pinned commit for the read plan (section 5) is `3d2cd9978c68c6338e902e9369266dbe8c4766c3`
(`12.0.5`) with `530ae6c769d2fd55ea5157542654810be81a3193` (`16.0.2`) as the
second tree, the same two trees as `docs/weston-release-read.md`.

## 2. What is known about built-in lease support

| # | Question | Answer | Tag |
|---|----------|--------|-----|
| K1 | Does Weston `12.0.5` or `16.0.2` contain lease code (whole-word search for `lease`, `leases`, `leasing`, `leased`, `drm-lease`, `drm_lease`, `*.po` excluded)? | **No hit in either tree.** So no `wp_drm_lease_device_v1` global, no lease option and no lease-protocol XML in either release | read from source (2026-11-28, 12.0.5: T1 to T6); `16.0.2` read from source (other doc): `docs/weston-release-read.md` row R1 |
| K2 | Does NXP `weston-imx` have lease code? | No, at `weston-imx-12.0.5` (`a29bbd0f…`), `14.0.2` and `15.0.1` | read from source (other doc): `docs/weston-imx-lease.md` rows S3 to S5 |
| K3 | What lease support is around Weston? | AGL's `meta-agl-drm-lease` patches add a `--drm-lease=<name>` **client** option to a 13.0.x Weston. It takes an fd from `drm-lease-manager` through `libdlmclient`. `Upstream-Status: Pending` | read from source (other doc): `docs/weston-imx-lease.md` rows S7, S8 |
| K4 | Is there a `wp_drm_lease_device_v1` implementation in the Weston DRM backend that a lease-source mode would use? | K1 and T2 say the string `lease` and any `wp_drm*` name do not occur in `12.0.5` (and, per the other doc, not in `16.0.2`), so the answer for both is "no" (inferred from K1). Trees newer than `16.0.2` (`main`) were not searched | inferred (from K1) |
| K5 | Which libweston version introduced it? | **Not established.** K1 says it is not in `12.0.5` or `16.0.2`; if it exists at all it is in an unreleased tree or on a branch not searched. No version is given here, so none can be quoted | not read |
| K6 | The meson option that enables it? | **Not established**, same reason as K5. Do not use a name from memory | not read |
| K7 | The `weston.ini` connector lease keys? | **Not established**, same reason. `docs/weston-release-read.md` and `docs/weston-imx-lease.md` found no lease key in `12.0.5`, `16.0.2` or `weston-imx` | not read |

The protocol name in the item title, `wp_drm_lease_device_v1`, is the interface
of the `drm-lease-v1` Wayland protocol. That the protocol is a Wayland
protocol and not a Weston-only feature is general knowledge, not read in this
item; its XML was not fetched, so no request or event is quoted.

## 3. The `drm-lease-manager` route, from this repository

| Point | `drm-lease-manager` route | Tag |
| --- | --- | --- |
| Who holds the lease | The daemon opens the card, creates leases with `drmModeCreateLease`, and hands an fd to a client over a Unix socket at `<DLM_RUNTIME_PATH>/<lease name>` | read from source (other doc): `docs/dlm-aarch64.md`, drm-lease-manager `68c48758c46d93c8e83abb2fb1ec168fa13d1a53` |
| Who is the lease client | The harness (`libdlmclient`). Weston is not involved. Weston may be stopped, or may keep other connectors | read from source (other doc): `docs/weston-imx-lease.md` §6, `docs/wp5-gap-analysis.md` |
| What the lease carries | Configured connectors and, with the TOML `planes` key, a set of plane IDs. A wrong plane ID makes lease creation fail | read from source (other doc): `docs/dlm-aarch64.md` |
| Weston change needed | None | inferred (K1, K3) |

## 4. What a lease-route offer to Weston would add or duplicate

All rows are **inferred** from K1 to K3 and section 3; nothing here was read in
a Weston lease implementation.

- **Adds (to Weston, if it were offered):** a way to hand a connector with a
  chosen plane set to a client. Weston `12.0.5` and `16.0.2` have none (K1),
  and the AGL patches are the only lease code seen near Weston, as a client
  (K3). A lease source would be new code in Weston's DRM backend, not a
  configuration.
- **Duplicates:** the lease creation and plane-set selection that
  `drm-lease-manager` already does for the standalone route (section 3). For
  the Verdin iMX95 demo, a Weston lease source would give the same result as
  the daemon, with a larger change surface (a compositor patch and a protocol
  implementation against a daemon that needs no Weston change).
- **Does not touch:** plane assignment inside Weston. Weston cannot be asked
  for a plane or be refused a frame (`docs/weston-release-read.md` §1); a lease
  does not change that.
- **Not measured:** any size, latency or maintenance advantage of one route
  over the other. No such figure exists in this repository for a Weston lease
  source.
- **Not decided:** whether to offer anything to Weston. That is an owner
  decision (`docs/advisory-planner-design.md` §6). No upstream acceptance is
  promised or implied.

## 5. How a build shows whether lease support is in (check for a later item)

Because K1 found no lease word in the source of `12.0.5`, `16.0.2` and
`weston-imx`, the expected result on the Verdin iMX95 image (inferred: Weston
`12.0.5.imx`) is **absent**. The checks below test that. All are read-only,
use no display and no lease, and need no write on the board.

Run them in this order. Each is a pass if it prints nothing (absent) and a
finding if it prints a line. A finding means K1 does not hold for that build
and the build's own source must be read.

1. **Version**: `weston --version` (also records WP5-01 for the board item).
2. **Find the DRM backend and the compositor binary**:
   `find / -name 'drm-backend.so' -o -name 'weston' -type f 2>/dev/null`
   (typical paths are `/usr/lib/libweston-*/drm-backend.so` and
   `/usr/bin/weston`; not read, the board item records the real paths).
3. **Strings test on the three binaries** (the DRM backend `.so`, the `weston`
   binary, `libweston-*.so`):
   `strings <file> | grep -iE 'wp_drm_lease|drm_lease|drm-lease|create_lease'`
   An empty result means no compiled lease protocol or lease code. A hit on
   `wp_drm_lease_device_v1` means the protocol is built in.
4. **Symbol test** (if `nm` or `readelf` exists on the image, else step 3):
   `readelf -Ws --dyn-syms <file> | grep -i lease`
5. **Option test**: `weston --help 2>&1 | grep -i lease`. A build with the AGL
   patches would list `--drm-lease` (K3). That is the client option, not a
   lease source, and must be reported as such.
6. **Config test**: `grep -ri lease /etc/xdg/weston/ /etc/weston* ~/.config/weston.ini 2>/dev/null`
   (read-only). No key name is given in this doc because K7 is not read.
7. **Recipe version**: the package manager's record of the `weston` package
   (for example `opkg status weston` or `rpm -q weston`, whichever the image
   has), dated as observed.

How to read the result:

- Steps 3 to 6 empty, step 1 equal to `12.0.5`: lease support is **not in**
  this build (record it on the board item with its date; the source link is K1).
- A hit only in step 5 (`--drm-lease`): the AGL client patch set is applied.
  Weston is a lease client, still not a lease source.
- A hit in step 3 or 4 on `wp_drm_lease_device_v1`: a lease-source feature is
  built in. Then read that build's source before anything is claimed about the
  option name or the version that introduced it.

A `strings` miss is weaker than a source read: the protocol interface name is
the string most likely compiled into a binary that implements it, but a
stripped or differently named build could hide it (inferred). Step 5 and
section K1 back it from two other sides.

## 6. Plan to complete the read

When a worker is allowed to fetch the tree (or an owner supplies a checkout),
the read is a short job:

1. `git clone` or fetch the tags `12.0.5` and `16.0.2`, and `main` if the
   owner wants to know what is unreleased (record the `main` commit and date).
2. In each tree run the same search as `docs/weston-release-read.md` R1, then
   `git grep -n -i 'drm.lease\|wp_drm_lease'`, `git ls-tree -r --name-only
   <tag> | grep -i lease`, and read `meson_options.txt`,
   `libweston/backend-drm/meson.build`, `protocol/meson.build` and
   `man/weston-drm.man` for any lease option or key.
3. If `main` or another branch has the feature, `git log -S wp_drm_lease_device_v1
   --reverse` finds the introducing commit, and `git tag --contains <commit>`
   gives the first release (K5). Fill K4 to K7 with file and line, tagged
   read from source (date, tag).
4. Re-state section 4 only if the result differs from K1.

## 7. Open items

- K4 to K7 are open (not read). The board check in section 5 does not need
  them.
- This doc is not yet linked from `README.md`; `scripts/check-readme-docs.sh`
  lists every `docs/` entry the README must mention. The README link belongs
  to plan #470 item `wp5-trace`.
