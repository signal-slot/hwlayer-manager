# WP5 reproducible demo: clean checkout to the M1 demo on the Verdin iMX95

SoW §8 WP5 asks for a reproducible demo (issue WP5-08). This page is the path from a clean checkout of this repository to the M1 demo on the Verdin iMX95: pinned versions, cross-build, deploy, run, expected output, and how to put the board back. The demo is the lease route of `docs/m1-imx95/lease-run.md`: AGL drm-lease-manager leases the LVDS output with two planes, and `drm-harness` is its lease client and composes two layers on two planes. The master-descriptor route is a short variant at the end.

Status, written 2026-11-28 at commit `df507ed60b25d5bbaa1b97903ee9674de29fb223`:

- **Host part: verified** from a fresh checkout (host only, 2026-11-28, see "Host verification").
- **Board part: not run from this page.** Every expected line below is **read from source** of `docs/m1-imx95/lease-run.md` (observed there on 2026-11-27 on the board, with an earlier harness build). See "Walk-through on the board" at the end.

The page holds no addresses, host names, users, serial paths or credentials. `<board>` is the ssh destination (`user@host`) from the farm lease; `$IMX95_SSH` holds the same value in the shell and is never written to a file.

## 1. Pinned versions

| Item | Pin | Source of the pin |
| --- | --- | --- |
| This repository | the commit you check out; this page was written at `df507ed60b25d5bbaa1b97903ee9674de29fb223`. Use that commit, or a later commit of `main` where `git log -- docs/wp5-demo.md scripts/cross-aarch64.sh scripts/build-dlm-aarch64.sh` shows no change after it. Record `git rev-parse HEAD` with every run. | `git rev-parse HEAD` |
| Rust, for the cross-build | `Cargo.toml` `rust-version = "1.83"` (the floor; `docs/msrv.md`, observed 2026-11-28: 1.82 fails, 1.83 passes). No `rust-toolchain` file exists. `scripts/cross-aarch64.sh` uses the **active** toolchain if it has the musl target, else `rustup target add`, else a private rustup at the `rust-version` (`HWLAYER_XBUILD_TOOLCHAIN` overrides). Host verification below used stable `rustc 1.99.0 (b940084d7 2026-09-28)`. | `Cargo.toml`, `scripts/cross-aarch64.sh` (read from source) |
| Rust target | `aarch64-unknown-linux-musl`, static, linked by the toolchain's `rust-lld` | `scripts/cross-aarch64.sh` |
| Dependencies of the Rust build | `Cargo.lock` is committed; pass `--locked` | repository |
| drm-lease-manager | `68c48758c46d93c8e83abb2fb1ec168fa13d1a53` (version 0.1.0), `https://gerrit.automotivelinux.org/gerrit/src/drm-lease-manager` | `docs/dlm-aarch64.md`, `scripts/build-dlm-aarch64.sh` |
| libdrm | `25dec5b91fe4d2638787d033a0b22b6c1dc145e0` (tag `libdrm-2.4.123`), `https://gitlab.freedesktop.org/mesa/libdrm.git` | same |
| tomlc99 | `62ce93ceb713dc49909d0fef1411f9d37bef83f2`, `https://github.com/cktan/tomlc99` | same |
| Host tools for the daemon | `git`, `meson`, `ninja`, `pkg-config`, `file`, and a cross toolchain: one of `aarch64-linux-gnu-gcc`, `aarch64-linux-musl-gcc`, `zig`, clang with `ld.lld` and a sysroot, or (route 6) `clang`, `python3`, `ar`, `zstd` and network access to Debian trixie packages, which the script fetches and unpacks without root | `docs/dlm-aarch64.md` |
| Board | Toradex Verdin iMX95 WB on a Mallow carrier (Verdin Development Board), 10.1" LVDS panel on X12 (connector `LVDS-1`), Toradex Reference Multimedia Image 7.8.0 (`TDX Wayland with XWayland 7.8.0+build.17 (scarthgap)`, Yocto, not Torizon), kernel `6.6.157-7.8.0`, driver `imx95-dpu`, Weston 12.0.4 | observed 2026-11-27 and 2026-11-28, `docs/m1-imx95-report.md` |

Every source of drm-lease-manager is pinned by full commit hash; tag and branch names are never used to check anything out (`docs/dlm-aarch64.md`, "Pinned source"). Do not override `DLM_COMMIT`, `LIBDRM_COMMIT` or `TOML_COMMIT` for a reproducible run.

## 2. Clean checkout and host checks

```sh
git clone git@github.com:signal-slot/hwlayer-manager.git
cd hwlayer-manager
git checkout df507ed60b25d5bbaa1b97903ee9674de29fb223   # or the commit you record
git status --short                                      # must print nothing
git rev-parse HEAD                                      # write this into the run record
```

Host checks (they show that host tests and builds pass; they say nothing about the board):

```sh
sh scripts/check-all.sh
# or the parts: cargo fmt --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace --locked
```

If the host's global git configuration sets an https client certificate, the drm-lease-manager fetch can fail with `credential missing host field`; run the daemon build with `GIT_CONFIG_GLOBAL=/dev/null` (`docs/m1-imx95/lease-run.md`, limitation 1, observed 2026-11-27).

## 3. Cross-build

Harness and probe, static aarch64 (read from `scripts/cross-aarch64.sh` and `scripts/cross-aarch64-README.md`):

```sh
scripts/cross-aarch64.sh
# stdout, one path per line:
#   target/aarch64-unknown-linux-musl/release/examples/probe
#   target/aarch64-unknown-linux-musl/release/drm-harness
```

The script ends by running `file` on both and exits non-zero unless both are `ELF 64-bit LSB executable, ARM aarch64 ... statically linked`. The CI form of the harness line is `scripts/cross-aarch64.sh --locked -p composition-drm-harness --bin drm-harness`; add `--locked` to the other build to keep `Cargo.lock` fixed.

drm-lease-manager, `dlm-client-test` and `libdlmclient.a`, from the pinned sources:

```sh
scripts/build-dlm-aarch64.sh <DEPS_DIR>
# <DEPS_DIR>: a directory for the fetched sources, never /tmp or /var/tmp
# (the script refuses those); without the argument it uses $CONNECTED_DEPS_DIR.
# Build tree: $TMPDIR/dlm-aarch64-build, else target/dlm-aarch64-build in the repository.
# Result: <build tree>/out/bin/drm-lease-manager, out/bin/dlm-client-test,
#         out/lib/libdlmclient.a, out/include/libdlmclient/dlmclient.h
```

The last lines of the script print the three pins and the paths. A good result for the daemon reads `ELF 64-bit LSB executable, ARM aarch64, ... statically linked, ... stripped`. If no cross toolchain is found the script prints what is missing and exits 2 before fetching anything (`docs/dlm-aarch64.md`). The compiled-in socket directory is a build-host path, so `DLM_RUNTIME_PATH` must always be set at run time.

`scripts/imx95-session.sh deploy` builds and copies only `probe` and `drm-harness`. The daemon is copied by hand (next section).

## 4. Deploy to `/tmp` on the board

Nothing is installed into the image; only `/tmp` is written (`docs/imx95-board.md`, board rules). With `$B` = the build tree of the daemon (`<build tree>/out`):

```sh
export IMX95_SSH='<board>'            # from the farm lease; never written to a file
scripts/imx95-session.sh deploy       # probe and drm-harness to /tmp, listed in /tmp/imx95-session.files
scp "$B/bin/drm-lease-manager" "$IMX95_SSH":/tmp/drm-lease-manager
scp "$B/bin/dlm-client-test"   "$IMX95_SSH":/tmp/dlm-client-test      # optional, upstream's test client
```

Write the daemon config on the board. The plane ids 31 and 51 are the primary and an overlay of CRTC 39 on the board of the M1 run (observed 2026-11-27). Check them on your board first with `/tmp/probe --raw` (whether the probe may run while Weston owns the display is not recorded here; stop Weston first if it fails); a wrong id makes lease creation fail (`docs/dlm-aarch64.md`):

```sh
ssh "$IMX95_SSH" 'cat > /tmp/dlm.toml' <<'EOF'
[[lease]]
name = "hwlayer"
connectors = ["LVDS-1"]

[LVDS-1]
planes = [31, 51]
EOF
ssh "$IMX95_SSH" 'chmod +x /tmp/drm-lease-manager /tmp/dlm-client-test; file /tmp/drm-lease-manager 2>/dev/null; /tmp/drm-lease-manager --help; mkdir -p /tmp/lease-dlm'
```

Before the run, record the state to restore: `scripts/imx95-session.sh display-owner status` (Weston unit and whether it is active). Expect `weston.service`, `weston.socket` and `wayland-app-launch.service` active on the Reference Multimedia Image (observed 2026-11-27).

## 5. Run: the lease route

Weston holds DRM master, so it is stopped first and started again at the end, whatever happens. `display-owner stop` records whether the unit was active, so `display-owner start` restores only what was running.

```sh
S=scripts/imx95-session.sh
$S display-owner stop
# daemon, in the background on the board, as in lease-run.md (note: --config, not -c; -u needs an argument; device last)
ssh "$IMX95_SSH" 'DLM_RUNTIME_PATH=/tmp/lease-dlm setsid /tmp/drm-lease-manager --config /tmp/dlm.toml -v -u 1 /dev/dri/card0 >/tmp/dlm-daemon.log 2>&1 </dev/null &'
# optional check that the socket exists and the lease reaches a client
ssh "$IMX95_SSH" 'ls /tmp/lease-dlm; DLM_RUNTIME_PATH=/tmp/lease-dlm /tmp/dlm-client-test hwlayer'
# demo: the harness as lease client, two layers on two planes, held 30 s for a camera or an observer
$S --out ./board-logs run dlmd-two-planes -- /tmp/drm-harness --connector LVDS --profile imx95 --source dlm --lease hwlayer --dlm-dir /tmp/lease-dlm --seconds 30 two-planes
# optional: frame statistics through the lease
$S --out ./board-logs run dlmd-rotate -- /tmp/drm-harness --connector LVDS --profile imx95 --source dlm --lease hwlayer --dlm-dir /tmp/lease-dlm --frames 1800 rotate
```

The display is shown only through this lease: the harness never holds a master descriptor on this route. Option meaning: `docs/dlm-aarch64.md` (daemon), `tools/drm-harness/README.md` (harness), `scripts/imx95-session-README.md` (`run` saves `<name>.header`, `.stdout`, `.stderr` under `--out`).

### Expected output

From `docs/m1-imx95/lease-run.md`, drm-lease-manager route (observed 2026-11-27 on the board, kernel `6.6.157-7.8.0`; the figures vary slightly between runs):

- `dlm-client-test hwlayer` prints `crtc-id: 39`, `connector-id: 41`, `plane-id: 51`, exit 0. It lists planes without universal planes, so it does not show the primary.
- `two-planes`, last line:
  `VERDICT two-planes PASS 2 buffers on planes 31,51 (bottom,top)`.
  The `assign` records put layer 0 on plane 31 (primary, full screen, 1280x800) and layer 1 on plane 51 (overlay, 640x400 at 320,200). On the panel: a blue full-screen layer with a red rectangle on top in the middle.
- `rotate` (`--frames 1800`), last line:
  `VERDICT rotate PASS 1800 frames, 3 buffers per plane, 3598 releases in order, 0 dropped, 0 missed vblanks`.
  Observed on that run: latch interval median 16 668 µs, submit-to-latch median 11 303 µs, `Engine::validate` median 140 µs.
- A second harness started while the first holds the lease is refused: `VERDICT two-planes FAIL open ...: lease refused: the lease manager closed the connection` (expected: one client per lease without `-t`).
- `scripts/imx95-session.sh run` exit status is 0 for a PASS; the harness exits 0 only if all scenarios passed.

A different board or kernel can legitimately give other plane ids, figures or a refusal; record those as found and do not edit this section to match.

## 6. Put the board back

Always, also after a failure:

```sh
ssh "$IMX95_SSH" 'pkill -TERM drm-lease-manager; sleep 1; pidof drm-lease-manager drm-harness || echo none left'
ssh "$IMX95_SSH" 'rm -rf /tmp/lease-dlm /tmp/dlm.toml /tmp/dlm-daemon.log /tmp/drm-lease-manager /tmp/dlm-client-test'
$S display-owner start                 # starts Weston only if it was active; waits for "active"
$S cleanup                             # removes /tmp/probe, /tmp/drm-harness and the session records
ssh "$IMX95_SSH" 'systemctl is-active weston.service weston.socket wayland-app-launch.service; ls -la /tmp'
```

Required result: `display-owner start` prints `... is active again`, `systemctl is-active` prints `active` for the units that were active before (if `wayland-app-launch.service` was active and is not, `systemctl start wayland-app-launch.service`), and `/tmp` holds only its system entries as before. This is what the M1 run did (`docs/m1-imx95/lease-run.md`, "Weston before and after"). If Weston cannot be restored, say so in the report. No reboot, no U-Boot environment change, nothing written outside `/tmp`.

## 7. Variant: master-descriptor route

`docs/m1-imx95-report.md` condition 3 is evidence from the master-descriptor route, not from a lease. With Weston stopped (section 5) and no daemon:

```sh
$S --out ./board-logs run two-planes -- /tmp/drm-harness --connector LVDS --profile imx95 --seconds 10 two-planes
```

Expected last line (observed 2026-11-27, `docs/m1-imx95/master-run.md`, `docs/m1-imx95-report.md`): `VERDICT two-planes PASS 2 buffers on planes 31,51 (bottom,top)`. Restore as in section 6 (no daemon to stop). The lease route above is the demo of this page.

## 8. Host verification

Fresh checkout of commit `df507ed60b25d5bbaa1b97903ee9674de29fb223`, **host only, 2026-11-28**. x86-64 Linux host, `rustc 1.99.0 (b940084d7 2026-09-28)` (active toolchain already had the musl target: `toolchain: active`), clang 22.1.8, meson 1.11.2, ninja. The checkout was a clean `git worktree` of that commit on the host (a plain `git clone` was refused by the task sandbox); the build tree was `target/` inside it, so nothing came from another build. Commands as written in section 3:

- `scripts/cross-aarch64.sh`: **passed**, exit 0. The two outputs were `target/aarch64-unknown-linux-musl/release/examples/probe` and `.../release/drm-harness`, each `ELF 64-bit LSB executable, ARM aarch64, version 1 (SYSV), statically linked, not stripped`. Build time 6.2 s and 11.3 s with the crates compiled from scratch (the cargo registry was already cached on that host, so this does not show a cold download).
- `scripts/build-dlm-aarch64.sh <DEPS_DIR>`: **passed**. It ran on this host (route 6 toolchain, Debian arm64 sysroot, already unpacked in the dependency cache; sources fetched or checked out at the three pins, which the script printed at the end: drm-lease-manager `68c48758c46d`, libdrm `25dec5b91fe4`, tomlc99 `62ce93ceb713`). `out/bin/drm-lease-manager` and `out/bin/dlm-client-test`: `ELF 64-bit LSB executable, ARM aarch64, version 1 (GNU/Linux), statically linked, for GNU/Linux 3.7.0, stripped`; `out/lib/libdlmclient.a` and `out/include/libdlmclient/dlmclient.h` present. The only warning is the known `usage()` format warning in upstream `main.c`.
- Not run in that checkout: `scripts/check-all.sh` and `cargo test` (covered by CI and by other items; this item is documentation).

None of this is hardware evidence.

## 9. Walk-through on the board

Not run yet. No step of sections 4 to 6 has been executed from this page. A later item follows this page once on the Verdin iMX95 when the device is free and records dated results here: the commit and toolchain used, the VERDICT lines, whether Weston and `wayland-app-launch.service` came back, and the state of `/tmp`. Until then every expected line above comes from the 2026-11-27 run recorded in `docs/m1-imx95/lease-run.md`, not from a run of this page. If the board stays busy, the section stays "not run".
