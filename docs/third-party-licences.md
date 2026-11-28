# Third-party licences

The licences of the code this repository depends on or names as a build input. Written 2026-11-28 at commit `5d30922`. It closes the licence-recording part of WP5-05 (`docs/open-issues.md`) and W5-07 (`docs/wp5-gap-analysis.md` §5).

The fit column says whether a licence fits with Apache-2.0 for redistribution of this repository. **Every fit statement is inferred, not legal advice.**

Tags:

- **read from source (date)**: read in the upstream file at the named version or commit on that date.
- **recorded (doc, date)**: read by an earlier item and written in the named document; not re-read here.
- **not read**: nobody has read it. The reason is given.
- **inferred**: a conclusion from the other rows.

## 1. This repository

| Item | Licence | Basis |
| --- | --- | --- |
| All workspace crates | `Apache-2.0` | read from source (2026-11-28): `LICENSE`; `Cargo.toml` `[workspace.package] license = "Apache-2.0"`, inherited by every member with `license.workspace = true` |
| C files (`backends/twoplane/*.c`, `*.h`, `backends/twoplane/tests/c/*.c`, `crates/composition-ffi-staticlib/tests/c/*.c`, `crates/composition-ffi/include/composition_engine.h`) | `Apache-2.0` (the files that carry an SPDX line say so) | read from source (2026-11-28) |
| Copyright holder | `LICENSE` names none: its only copyright line is the appendix template `Copyright [yyyy] [name of copyright owner]`. `NOTICE` therefore says "The hwlayer-manager contributors" | read from source (2026-11-28); the wording in `NOTICE` is a placeholder until the owner names the holder (SoW §12: "naming and licensing require a separate decision") |
| Vendored third-party code | none. The tracked files are the project's own; no crate or upstream source is copied in | read from source (2026-11-28): every tracked file with a copyright or SPDX line was searched |

## 2. Rust crates in `Cargo.lock`

Source: `cargo metadata --locked --format-version 1` on 2026-11-28 (cargo 1.98.0-nightly). The licence is the `license` field of each crate's own `Cargo.toml`, as published on crates.io; **read from source (2026-11-28)** for every row. Every crate comes from `https://crates.io` (registry `https://github.com/rust-lang/crates.io-index`). The version plus the checksum in `Cargo.lock` pins the exact source. `Cargo.lock` lists 26 packages: 8 workspace crates and the 18 external ones below.

"Used by" lists the workspace crates that reach the crate, directly or through other crates. "Kind" is the dependency kind (`normal`, `dev` or `build`) of the edge from the workspace crate, taken from `dep_kinds` in the `cargo metadata` resolve graph; it is `normal` for every row, because no workspace crate declares a `[dev-dependencies]` or `[build-dependencies]` entry on an external crate (checked 2026-11-28). Proc-macro crates are `normal` dependencies of the crate that uses them. "How" says how the crate ends up in a build:

- **linked**: compiled into the binary or library.
- **build only**: a proc-macro or its dependency. It runs in the compiler and is not in the output.
- **other targets only**: behind a `cfg` for targets other than Linux. `cargo tree --target x86_64-unknown-linux-gnu` and `--target aarch64-unknown-linux-gnu` do not include it, so no build this project makes today compiles it.

| Crate | Version | SPDX licence | Used by | Kind | How | Fit with Apache-2.0 (inferred, not legal advice) |
| --- | --- | --- | --- | --- | --- | --- |
| `bitflags` | 2.13.2 | `MIT OR Apache-2.0` | composition-drm, composition-drm-harness, vm-init | normal | linked | yes; Apache-2.0 can be chosen |
| `bytemuck` | 1.25.2 | `Zlib OR Apache-2.0 OR MIT` | composition-drm, composition-drm-harness | normal | linked | yes; Apache-2.0 can be chosen |
| `bytemuck_derive` | 1.12.1 | `Zlib OR Apache-2.0 OR MIT` | composition-drm, composition-drm-harness | normal | build only (proc-macro) | yes |
| `drm` | 0.15.0 | `MIT` | composition-drm, composition-drm-harness (both direct) | normal | linked | yes; keep its MIT notice when a binary is distributed |
| `drm-ffi` | 0.9.1 | `MIT` | composition-drm, composition-drm-harness | normal | linked | yes; keep its MIT notice |
| `drm-fourcc` | 2.2.0 | `MIT` | composition-drm, composition-drm-harness | normal | linked | yes; keep its MIT notice |
| `drm-sys` | 0.8.1 | `MIT` | composition-drm, composition-drm-harness | normal | linked | yes; keep its MIT notice |
| `errno` | 0.3.15 | `MIT OR Apache-2.0` | composition-drm, composition-drm-harness, vm-init (through `rustix`) | normal | other targets only | yes |
| `libc` | 0.2.191 | `MIT OR Apache-2.0` | composition-drm, composition-drm-harness, vm-init (through `drm`, `drm-sys`, `errno`) | normal | other targets only | yes |
| `linux-raw-sys` | 0.9.4 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | composition-drm, composition-drm-harness (through `drm-sys`) | normal | linked | yes; Apache-2.0 can be chosen |
| `linux-raw-sys` | 0.12.1 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | composition-drm, composition-drm-harness, vm-init (through `rustix`) | normal | linked | yes; Apache-2.0 can be chosen |
| `proc-macro2` | 1.0.107 | `MIT OR Apache-2.0` | composition-drm, composition-drm-harness | normal | build only | yes |
| `quote` | 1.0.47 | `MIT OR Apache-2.0` | composition-drm, composition-drm-harness | normal | build only | yes |
| `rustix` | 1.1.6 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | composition-drm, composition-drm-harness, vm-init (all direct) | normal | linked | yes; Apache-2.0 can be chosen |
| `syn` | 3.0.10 | `MIT OR Apache-2.0` | composition-drm, composition-drm-harness | normal | build only | yes |
| `unicode-ident` | 1.0.27 | `(MIT OR Apache-2.0) AND Unicode-3.0` | composition-drm, composition-drm-harness | normal | build only | yes; Unicode-3.0 is a permissive licence for the Unicode data tables |
| `windows-link` | 0.2.1 | `MIT OR Apache-2.0` | composition-drm, composition-drm-harness, vm-init | normal | other targets only | yes |
| `windows-sys` | 0.61.2 | `MIT OR Apache-2.0` | composition-drm, composition-drm-harness, vm-init | normal | other targets only | yes |

What this means per build (inferred from `cargo tree -e normal` on 2026-11-28):

- `composition-core`, `composition-fake`, `composition-ffi`, `composition-ffi-staticlib` and `composition-twoplane` have **no external dependency**. The `thumbv7em` builds and the Zephyr staticlib carry no third-party crate code.
- The `composition-drm` library, its examples, `composition-drm-harness` and `vm-init` link `MIT` crates (`drm`, `drm-ffi`, `drm-fourcc`, `drm-sys`) and dual- or triple-licensed crates. Whoever distributes one of these **binaries** should ship the MIT copyright and permission notices of the four `drm*` crates with it. Today nobody distributes binaries: they are built on a development machine and copied to a board under `/tmp` for a test.
- No crate here has a copyleft licence. No dev-dependency is external.

## 3. Upstream projects named as build or test inputs

These are not in `Cargo.lock` and are not linked into any crate. They are named in the repository's documents as sources to build or read.

### 3.1 drm-lease-manager and libdlmclient (M1 tooling)

Pinned in `docs/dlm-aarch64.md` and fetched and built by `scripts/build-dlm-aarch64.sh`. `libdlmclient` is a directory of the same repository, so one entry covers both. The M1 tooling uses the daemon as a separate process and speaks its socket protocol; no workspace crate links `libdlmclient`.

| Field | Value |
| --- | --- |
| URL | `https://gerrit.automotivelinux.org/gerrit/src/drm-lease-manager` |
| Commit | `68c48758c46d93c8e83abb2fb1ec168fa13d1a53`, committed 2026-08-28, "Add DRM Lease Manager Configuration Guide" |
| Licence | `Apache-2.0` |
| Licence file | `LICENSE`: the Apache License 2.0 text with the appendix; it names no copyright holder (only the template line) |
| Copyright | All 29 `.c`/`.h` files at the commit (`common/`, `drm-lease-manager/` with `test/`, `libdlmclient/` with `test/`, `examples/dlm-client-test/`) carry the full Apache-2.0 header and no other licence text. 26 say `Copyright 2020-2021 IGEL Co., Ltd.`; 3 (`drm-lease-manager/lease-config.c`, `lease-config.h`, `test/lease-config-test.c`) say `Copyright 2022 IGEL Co., Ltd.`. No file has an SPDX line. The other 20 files of the 49 in the tree (`meson.build` files, `README.md`, `doc/configuration-guide.md`, `.clang-format`, `.gitlab` templates, `Doxyfile.in`, `fff.wrap`) have no licence or copyright text, except the `fff-1.0/meson.build` noted below |
| NOTICE | none (no `NOTICE` in the tree at that commit) |
| `meson.build` | `project('lease_manager', 'c', version: '0.1.0', ...)`, no `license:` field. The only `license:` in the tree is `license: 'MIT'` in `subprojects/packagefiles/fff-1.0/meson.build`, the build file for the optional test dependency fff |
| Test-only input | `subprojects/fff.wrap` points at `https://github.com/meekrosoft/fff/archive/v1.0.tar.gz`. Used only with `enable-tests`, which the build script turns off. fff's own licence: not read (the packagefile says `MIT`, not the fff source) |
| Basis | read from source (2026-11-28) from a git fetch of `68c48758c46d93c8e83abb2fb1ec168fa13d1a53` (`git fetch --depth 1 <URL> <commit>`, then `git ls-tree`, `git show` and `git grep` for `SPDX|License|Copyright` over the whole tree). An earlier reading the same day, through the Gerrit REST API (Gitiles URLs on that host return 404, `docs/dlm-aarch64.md`), gave the same licence and holder for the files it read; the git read covers every file. Not a full `git clone` (history was not fetched; only the pinned commit) |
| Fit (inferred, not legal advice) | **yes**: the same licence as this repository. Nothing of it is in the repository. A distributed binary built by the script must carry the Apache-2.0 text; there is no upstream `NOTICE` to pass on |

### 3.2 libdrm (build input of drm-lease-manager)

| Field | Value |
| --- | --- |
| URL | `https://gitlab.freedesktop.org/mesa/libdrm.git` |
| Commit | `25dec5b91fe4d2638787d033a0b22b6c1dc145e0`, committed 2024-08-26, "build: bump version to 2.4.123" (tag `libdrm-2.4.123`, per `docs/dlm-aarch64.md`) |
| Licence | `MIT`. `meson.build:26-33` declares `license : 'MIT'`. The tree has no top-level licence file; each source file carries its own MIT (X11-style) header, for example `xf86drm.c`: "Copyright 1999 Precision Insight, Inc., Cedar Park, Texas." and "Copyright 2000 VA Linux Systems, Inc., Sunnyvale, California." |
| Basis | read from source (2026-11-28): shallow git fetch of the commit; `meson.build`, the top-level file list and the header of `xf86drm.c` were read. The headers of the other source files were not read one by one |
| Fit (inferred, not legal advice) | yes. Statically linked into the drm-lease-manager binary by the script, so a distributed binary should carry the MIT notices of the libdrm files it contains. Not part of this repository |

### 3.3 tomlc99 (build input of drm-lease-manager)

| Field | Value |
| --- | --- |
| URL | `https://github.com/cktan/tomlc99` |
| Commit | `62ce93ceb713dc49909d0fef1411f9d37bef83f2`, committed 2026-11-04, "README: point to tomlc17 compat/tomlc99" |
| Licence | `MIT` (`LICENSE`: "MIT License", "Copyright (c) CK Tan") |
| Basis | read from source (2026-11-28): `https://raw.githubusercontent.com/cktan/tomlc99/62ce93ceb713dc49909d0fef1411f9d37bef83f2/LICENSE` and the GitHub commits API |
| Fit (inferred, not legal advice) | yes; keep its notice with a distributed drm-lease-manager binary. Not part of this repository |

### 3.4 libliftoff (read for WP5, not built)

| Field | Value |
| --- | --- |
| URL | `https://gitlab.freedesktop.org/emersion/libliftoff.git` |
| Commit | `c4226a79b7a52f59bb51789b1eda112658609125`, committed 2026-08-24, "Document why HOTSPOT_{X,Y} properties can't be discarded" |
| Licence | `MIT` (`LICENSE`: "MIT License", "Copyright (c) 2019 Simon Ser", the standard MIT text) |
| Basis | read from source (2026-11-28): shallow git fetch of the commit, whole `LICENSE` read. Agrees with `docs/wp5-gap-analysis.md` |
| Fit (inferred, not legal advice) | yes for reading and for a future host-only cross-check that links it; copying its code in would need its MIT notice kept |

### 3.5 Weston (read for WP5, not built)

| Field | Value |
| --- | --- |
| URL | `https://gitlab.freedesktop.org/wayland/weston.git` |
| Commit | `c8d00e2a1dbc98f4a8fb6473a4b485ffce409f3a`, committed 2026-11-27, "backend-drm: Destroy the heads of a backend that fails to start" |
| Licence | `MIT` (the X.org wording). `COPYING` calls itself "the version of the MIT "Expat" License used by X.org". It differs from the SPDX `MIT` text only in the phrase "(including the next paragraph)" in the notice clause; mapping it to the SPDX id `MIT` is inferred |
| Copyright | `COPYING`: Kristian Høgsberg (2008-2012), Intel Corporation (2010-2012), Benjamin Franzke (2010-2011), Collabora, Ltd. (2011-2012), Red Hat (2010). `COPYING` says nothing about files under other licences; individual file headers were not read |
| Basis | read from source (2026-11-28): shallow git fetch of the commit, whole `COPYING` read |
| Fit (inferred, not legal advice) | yes for reading. Weston's contribution rules (human DCO sign-off, limits on AI-authored changes) are a separate matter, recorded in `docs/wp5-gap-analysis.md` §2 and §5 |

### 3.6 Zephyr (M2 build input, pinned)

`scripts/prepare-zephyr.sh` pins Zephyr `v4.5.0`, the Zephyr SDK `1.0.1`, and the west modules `cmsis cmsis_6 hal_nxp`. The Zephyr tree and its modules are fetched, not copied into this repository.

| Component | URL | Pinned revision | Licence | Tag |
| --- | --- | --- | --- | --- |
| Zephyr | `https://github.com/zephyrproject-rtos/zephyr` | tag `v4.5.0` (annotated tag object `d6e5dc6ae358a6bbe05d040e0428e4161f09d006`, commit `41d10b59730406bbab4ca9f1fbbe9e96aafb4966`) | Apache License 2.0 (top-level `LICENSE`, first lines) | read from source 2026-11-28 |
| cmsis (west module) | `https://github.com/zephyrproject-rtos/cmsis` | `512cc7e895e8491696b61f7ba8066b4a182569b8`, from `west.yml` at `v4.5.0` | Apache License 2.0 (top-level `LICENSE.txt`, first lines) | read from source 2026-11-28 |
| cmsis_6 (west module) | `https://github.com/zephyrproject-rtos/CMSIS_6` | `1c1840af7a7e757d6e2fec3ddb0e5ce0dfcc93c8`, from `west.yml` at `v4.5.0` | Apache License 2.0 (top-level `LICENSE`, first lines) | read from source 2026-11-28 |
| hal_nxp (west module) | `https://github.com/zephyrproject-rtos/hal_nxp` | `a8b437ef366ff9a6f9bee3377dc189c4d2623f5b`, from `west.yml` at `v4.5.0` | BSD 3-Clause (top-level `LICENSE.BSD-3`, titled "The BSD 3 Clause License"). Files under `mcux/` and elsewhere in the module may carry their own headers; those were not read | read from source 2026-11-28 |
| Zephyr SDK (arm-zephyr-eabi toolchain) | `https://github.com/zephyrproject-rtos/sdk-ng` | tag `v1.0.1` (annotated tag object `9f6d137b702f4fe6989b780ace09a5c5da76a1b9`; release archives named in `scripts/prepare-zephyr.sh`) | Apache License 2.0 for the `sdk-ng` repository (top-level `LICENSE`, first lines); the licences of the toolchain components inside the archives were not read | read from source 2026-11-28 |

Per-file licence headers inside the modules were not read, and neither were the licences of the components bundled in the SDK archives (not measured). Zephyr's `LICENSES/` directory shows that some files and modules are under other licences; those were not read either.

Fit (inferred, not legal advice): Zephyr's main licence is the same as this repository's. A Zephyr application kept here (`tests/board/...`) is the project's own code; the Zephyr tree and its modules are fetched, not copied in.

## 4. Gaps

- libdrm and Weston: only the files named above were read; per-file headers elsewhere in those trees were not (sections 3.2, 3.5).
- fff (drm-lease-manager test-only subproject): not read.
- Per-file headers inside the Zephyr modules, and the components bundled in the Zephyr SDK archives: pinned, but their licences are not read (section 3.6).
- The demo firmware that `docs/twoplane-backend.md` names (`display_port_<soc>.c`, outside this repository): no document pins a URL or commit, and no copy could be read on 2026-11-28. Its licence is not read. It is not a build input of this repository.
- The copyright holder named in `NOTICE` is a placeholder (section 1).

## 5. Regenerating the Rust table

Checked 2026-11-28: every non-workspace package of `cargo metadata --format-version 1 --locked` (26 packages minus the 8 workspace members = 18) appears in section 2 with the same version and the same `license` field. `composition-core` has no dependencies at all, so it appears in no "Used by" cell except as a workspace crate.

To regenerate, after `cargo fetch --locked`:

```sh
cargo metadata --format-version 1 --locked > metadata.json
```

Take every entry of `packages` whose `id` is not in `workspace_members` and print its `name`, `version` and `license`. For the "Used by" and "Kind" columns, walk `resolve.nodes[].deps[]` from each workspace member and read `dep_kinds[].kind` (`null` is `normal`, otherwise `dev` or `build`). `cargo tree -e normal,build,dev --target all -i <crate>` gives the same answer per crate. Update the package count in section 2 and the date above when `Cargo.lock` changes.
