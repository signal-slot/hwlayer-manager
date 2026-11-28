# prepare-zephyr.sh

Fetches an upstream Zephyr west workspace and a minimal Zephyr SDK into one
destination root (a deps directory) the way Zephyr documents, pinned. POSIX
`sh`. Pins, reasons and sizes: [docs/zephyr-workspace.md](../docs/zephyr-workspace.md).

    scripts/prepare-zephyr.sh [--dry-run] [--clean-git-config] <dest-root>
    scripts/prepare-zephyr.sh --dry-run <deps-dir>   # print the steps, change nothing
    scripts/prepare-zephyr.sh <deps-dir>             # do it

| argument | meaning |
|---|---|
| `<dest-root>` | required, exactly one; everything is written under `<dest-root>/zephyr` (a trailing `/` is fine) |
| `--dry-run` | print the steps and the export lines, create nothing; skips the host-tool check |
| `--clean-git-config` | run git with `GIT_CONFIG_GLOBAL=/dev/null` and `GIT_CONFIG_NOSYSTEM=1`, for hosts whose git config breaks anonymous HTTPS clones |
| `-h`, `--help` | usage, exit 2 |

Exit 0 ok, 1 unsupported host, missing tool, checksum mismatch or a workspace at
another tag, 2 usage error (unknown option, no or two destinations).

## What it does

Under `<deps-dir>/zephyr/`:

1. `venv/`: Python venv with `west` (pinned) and Zephyr's `requirements-base.txt`.
2. `workspace/`: `west init` at the pinned Zephyr tag, then a shallow
   `west update` of `cmsis`, `cmsis_6` and `hal_nxp` only. No blobs are fetched.
3. `sdk/zephyr-sdk-<version>/`: the minimal SDK bundle plus the `arm-zephyr-eabi`
   toolchain, each checked against a pinned sha256 before extraction. The
   archives (`downloads/`) are deleted afterwards.

Nothing is written outside `<deps-dir>/zephyr`; the SDK's CMake package
registration (which would write to the home directory) is not run. Re-running
skips finished steps; a workspace at a different Zephyr tag is an error. The
versions and checksums are the variables at the top of the script.

Host tools needed (not dry-run): `git`, `python3` with `venv`, `cmake`, `ninja`,
`dtc`, `tar`, `sha256sum`, and `curl` or `wget`.

## Reported values

The last lines printed are the two values to export:

    export ZEPHYR_WORKSPACE=<deps-dir>/zephyr/workspace
    export ZEPHYR_SDK_INSTALL_DIR=<deps-dir>/zephyr/sdk/zephyr-sdk-<version>

followed by a comment line with a `west build` example for the i.MX95 M7
(`imx95_evk/mimx9596/m7`) run from the workspace directory.

## Host restriction

Linux x86_64 only: the pinned SDK archives are for that host. Any other
`uname -s`/`uname -m` exits 1 with an error (before the dry run prints steps).

## Test

`scripts/prepare-zephyr.test.sh` covers only argument handling and `--dry-run`:
usage errors exit 2 and create nothing; a dry run exits 0, writes nothing, prints
the pinned tag, SDK version and west version and the two export paths; and the
pins agree with `docs/zephyr-workspace.md`. The dry-run cases are skipped on
non-Linux-x86_64 hosts. It uses no network and does not run the real install, so
the download, checksum, `west` and extraction steps are not tested by it.
