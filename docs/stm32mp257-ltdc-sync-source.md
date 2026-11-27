# STM32MP257 LTDC: sync semantics read from source

Status: **not read.** The source checkouts could not be made, so this document
holds no source rows.

Nothing in this document is measured on hardware. No board was accessed.

## Fetch record (2026-11-28)

| Tree | Intended pin | Result |
|---|---|---|
| STMicroelectronics/linux, branch `v6.18-stm32mp` | commit hash not obtained | `git clone --depth 1 --branch v6.18-stm32mp https://github.com/STMicroelectronics/linux` was refused by the sandbox's command-approval layer ("This command requires approval"). It never ran, so there is no HTTP or git error code. |
| torvalds/linux (mainline), named tag | not chosen, no hash obtained | Not attempted. Without the ST tree there was nothing to compare against, and network commands were being refused. |
| Raw-file fallback (`curl` of `ltdc.c` from `raw.githubusercontent.com`) | n/a | A status-only request (output discarded) returned HTTP 200, so the host is reachable. A request that saves the file was refused for approval, so no source text was read. The clone was denied again on retry. |

The reusable dependency directory held no kernel checkout. `stlinux-raw` was
present but empty, and there was no `stm32-linux`.

## Sync semantics to document

Every row stays open until it can be read from `drivers/gpu/drm/stm/ltdc.c` and
`drv.c` in both trees. Each answer will be tagged `source` (with file:line at the
pinned commit) or `inferred`.

| # | Question | Status |
|---|---|---|
| 1 | Page-flip completion: where the vblank or line / register-reload interrupt calls `drm_crtc_send_vblank_event` or arms the event, and whether the shadow-register reload (IMR/VBR) latches updates at vblank | not read |
| 2 | Vblank support: `enable_vblank` / `disable_vblank`, `get_scanout_position`, `get_vblank_timestamp`, vblank-off quirks | not read |
| 3 | Fences: `IN_FENCE_FD` / `OUT_FENCE_PTR` through the atomic helpers (`drm_gem_plane_helper_prepare_fb` or an equivalent), and implicit-fence waiting | not read |
| 4 | Async or tearing commits: `atomic_async_check` / `async_update` | not read |
| 5 | Whether one commit updates all three planes with a single register reload | not read |
| 6 | ST-only changes against mainline in these paths | not read |

## Next step

Rerun this item where `git clone` is permitted. Make a shallow clone of the ST
branch and of mainline at a named tag in the dependency cache. Read `ltdc.c` and
`drv.c` in both, then fill in the table above with pinned commit hashes.
