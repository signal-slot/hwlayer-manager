# Zephyr workspace and SDK for board applications

Board programs in this repository (`tests/board/*`) are Zephyr applications. They
are built against two things that live outside the repository:

- `ZEPHYR_WORKSPACE`: a west workspace (`<ws>/zephyr`, `<ws>/modules/...`)
- `ZEPHYR_SDK_INSTALL_DIR`: a Zephyr SDK with the `arm-zephyr-eabi` toolchain

`scripts/prepare-zephyr.sh` creates both, pinned, under one destination root.

## Pins (2026-11-28)

| What | Pin | Source |
|---|---|---|
| Zephyr | `v4.5.0` | https://github.com/zephyrproject-rtos/zephyr/releases/tag/v4.5.0 |
| Zephyr SDK | `1.0.1` | https://github.com/zephyrproject-rtos/sdk-ng/releases/tag/v1.0.1 |
| SDK minimal bundle | `zephyr-sdk-1.0.1_linux-x86_64_minimal.tar.xz`, sha256 `ca9bc0ff66fafca1dac9d592a36d953cf16d096a9d09b1c0357f021cf9f6a7eb`, 70.7 MB | `https://github.com/zephyrproject-rtos/sdk-ng/releases/download/v1.0.1/` |
| arm toolchain | `toolchain_gnu_linux-x86_64_arm-zephyr-eabi.tar.xz`, sha256 `21b85981cb5a1818d9bc53d82af80f208946ec038b982ff1907287572ed3a634`, 101.8 MB | same directory |
| west | `1.5.0` (PyPI) | https://pypi.org/project/west/ |
| west modules | `cmsis`, `cmsis_6`, `hal_nxp` (revisions come from the `v4.5.0` `west.yml`) | https://github.com/zephyrproject-rtos/zephyr/blob/v4.5.0/west.yml |

The two SDK checksums equal both the `digest` field of the GitHub release API and
the line in the release's `sha256.sum` (read 2026-11-28).

### Why these versions

- **Zephyr v4.5.0** is the newest stable feature release in the GitHub releases
  list on 2026-11-28 (published 2026-10-28). `v4.3.2` is a later-dated bugfix of
  the older 4.3 line and is not newer. The pin is a tag, not a branch.
- **The M7 board exists at that tag.** `boards/nxp/imx95_evk/board.yml` at `v4.5.0`
  lists SoC `mimx9596` with variants `smp` (a55), `ddr` (m7) and `flash` (m7), and
  `imx95_evk_mimx9596_m7.yaml` has `identifier: imx95_evk/mimx9596/m7`.
  **The board is called `imx95_evk`, not `mimx95_evk`**; `boards/nxp/mimx95_evk`
  does not exist at `v4.5.0` (HTTP 404 on 2026-11-28). The M7 targets are
  therefore `imx95_evk/mimx9596/m7` (TCM), `imx95_evk/mimx9596/m7/ddr` and
  `imx95_evk/mimx9596/m7/flash`. Which of them can be loaded on the Verdin
  iMX95 is a question for the M2 items, not answered here.
- **SDK 1.0.1** is what `SDK_VERSION` in the `v4.5.0` tree contains, and it is the
  newest non-prerelease in sdk-ng (published 2026-03-25).
- Only the Linux x86_64 archives are pinned; the script refuses other hosts.

## Prepare

```sh
scripts/prepare-zephyr.sh --dry-run "$CONNECTED_DEPS_DIR"   # print the steps
scripts/prepare-zephyr.sh "$CONNECTED_DEPS_DIR"             # do it
```

Options: `--dry-run` changes nothing; `--clean-git-config` runs git with
`GIT_CONFIG_GLOBAL=/dev/null` for hosts whose global git config (for example a
client certificate) breaks anonymous HTTPS clones.

Host tools needed: `git`, `python3` with `venv`, `cmake`, `ninja`, `dtc`, `tar`,
`sha256sum`, `curl` or `wget`. The SDK host tools bundle is not installed.

Steps, all under `<dest>/zephyr/` (nothing is written elsewhere; the SDK's CMake
package registration, which would write to `~/.cmake`, is deliberately skipped):

1. `python3 -m venv venv`; `pip install west==1.5.0`.
2. `west init -m https://github.com/zephyrproject-rtos/zephyr --mr v4.5.0 workspace`.
3. `west update --narrow --fetch-opt=--depth=1 cmsis cmsis_6 hal_nxp` (project
   names are the allowlist; every other project in `west.yml` is skipped).
4. Blobs: none fetched. `hal_nxp`'s `module.yml` blobs are Wi-Fi/BT/NPU libraries and
   EVK boot-firmware images (the latter click-through), none linked into an
   i.MX95 M7 build.
5. `pip install -r workspace/zephyr/scripts/requirements-base.txt` (unpinned
   beyond the lower bounds in that file).
6. Download the minimal SDK and the `arm-zephyr-eabi` toolchain, verify both
   sha256 values, extract to `sdk/zephyr-sdk-1.0.1/` and `.../gnu/`, delete the
   archives.
7. Print the two export lines:

```sh
export ZEPHYR_WORKSPACE=<dest>/zephyr/workspace
export ZEPHYR_SDK_INSTALL_DIR=<dest>/zephyr/sdk/zephyr-sdk-1.0.1
```

Re-running skips finished steps. An existing workspace at a different tag is an
error, not silently reused.

## Build an application

```sh
. <dest>/zephyr/venv/bin/activate
export ZEPHYR_WORKSPACE=... ZEPHYR_SDK_INSTALL_DIR=...
cd "$ZEPHYR_WORKSPACE"
west build -b imx95_evk/mimx9596/m7 zephyr/samples/hello_world -d <build dir outside repo and /tmp>
west build -b imx95_evk/mimx9596/m7 <repo>/tests/board/<app> -d <build dir outside repo and /tmp>
```

Use a build directory outside the repository and outside `/tmp`. If an
application needs a further west module, add it to `WEST_MODULES` in the script
rather than running an unrestricted `west update`.

## Status of this document

- Pins, board name, SDK asset names and checksums: observed from the upstream
  sources named above on 2026-11-28.
- Disk size (observed 2026-11-28, `du -sh`): `workspace` 2.5 GB (the zephyr
  repository is a full clone), `sdk` 829 MB, `venv` 53 MB; plus 32 MB for one
  `hello_world` build directory. Downloads are the two archives above (about
  172 MB) and are deleted after extraction.
- Observed 2026-11-28 on an x86_64 Linux host: `sh -n` and `--dry-run` pass; a
  real run into an empty destination succeeded and a second run skipped every
  finished step; `west build -b imx95_evk/mimx9596/m7 zephyr/samples/hello_world`
  (run from the workspace directory, `-d` outside it) linked `zephyr.elf` with
  GCC 14.3.0 from SDK 1.0.1: FLASH 24396 B of 256 KB, RAM 5160 B of 256 KB. This
  is a host build only; nothing was loaded on a board.
- West and CMake keep their own cache in `~/.cache/zephyr`, outside the
  destination. `west build` must be run with the workspace as the current
  directory (west extension commands such as `build` are only found there).
- Which of `m7`, `m7/ddr` and `m7/flash` suits the Verdin iMX95 boot flow is not
  decided here.
