# vkms and fences: what the kernel source says

Answers question 2 of [docs/sync-fences.md](../sync-fences.md) §7 ("does
vkms implement `IN_FENCE_FD`?") and fixes which `sync_file` sources a vkms
test can use. Nothing here was run. No vkms module was loaded and no DRM,
udev or seat setting was touched on the node that wrote this note.

Evidence tags: **read from source** is a statement about a function in the
tree at the pinned tag, written as `file:function`. **inferred** is a
conclusion drawn from that source and not seen to happen. Line numbers are
at the tag given in the column header and move between tags.

Pinned trees (sparse checkouts of the stable kernel tree, `drivers/gpu/drm`,
`drivers/dma-buf`), read 2026-11-28:

| Label | Tag | Why |
| --- | --- | --- |
| v6.6 | `v6.6` | the baseline named in the plan |
| v7.3 | `v7.3.6` | newest stable tag that existed on 2026-11-28 |

Everything below holds for both tags unless a row says otherwise.

## 1. Short answers

| Question | Answer | Tag |
| --- | --- | --- |
| Do vkms planes carry `IN_FENCE_FD`? | Yes. The DRM core attaches it to every plane of an atomic driver; vkms adds nothing and removes nothing. | read from source |
| Does the CRTC carry `OUT_FENCE_PTR`? | Yes, same mechanism, on the vkms CRTC. | read from source |
| Does vkms wait for an in-fence on a blocking commit? | Yes, in the core before the state swap. | read from source |
| Does vkms wait for an in-fence on a `DRM_MODE_ATOMIC_NONBLOCK` commit? | No. Its own `atomic_commit_tail` never calls `drm_atomic_helper_wait_for_fences`. A pending in-fence is held in the plane state and not waited on. | read from source (absence of the call), consequence inferred |
| Is the `OUT_FENCE_PTR` fence signalled? | Yes, when the flip event is delivered at the next simulated vblank. | read from source |
| Fence sources for a test | CRTC out-fence of an earlier commit (always); writeback out-fence (default module parameters); `sw_sync` (only if the kernel has `CONFIG_SW_SYNC`). | read from source |

The nonblocking row decides how a test is written (section 5): an in-fence
test against vkms has to use a blocking commit to prove that the driver
waits.

## 2. Where `IN_FENCE_FD` and `OUT_FENCE_PTR` come from

| Fact | Source | Tag |
| --- | --- | --- |
| The two properties are created once per device as standard properties (`prop_in_fence_fd` is a signed-range `IN_FENCE_FD`, `prop_out_fence_ptr` a range `OUT_FENCE_PTR`). | `drivers/gpu/drm/drm_mode_config.c:drm_mode_create_standard_properties` (v6.6 line 295 and 301) | read from source |
| Every plane of a driver with `DRIVER_ATOMIC` gets `IN_FENCE_FD` with default -1. | `drivers/gpu/drm/drm_plane.c:__drm_universal_plane_init` (v6.6 line 340, v7.3 line 467) | read from source |
| Every CRTC of a driver with `DRIVER_ATOMIC` gets `OUT_FENCE_PTR` with default 0. | `drivers/gpu/drm/drm_crtc.c:__drm_crtc_init_with_planes` (v6.6 line 316, v7.3 line 319) | read from source |
| vkms sets `DRIVER_MODESET \| DRIVER_ATOMIC \| DRIVER_GEM`, so both conditions hold. | `drivers/gpu/drm/vkms/vkms_drv.c:vkms_driver` (`driver_features`) | read from source |
| `DRM_VKMS` selects `DRM_KMS_HELPER`, and `DRM` selects `SYNC_FILE`, so the sync_file code the properties need is always built when vkms is. | `drivers/gpu/drm/Kconfig:DRM` (`select SYNC_FILE`, v6.6 line 18, v7.3 line 15) | read from source |
| vkms uses no private plane or CRTC properties for fences and has no fence code of its own; the only `dma_fence` calls in `drivers/gpu/drm/vkms/` are `dma_fence_begin_signalling` and `dma_fence_end_signalling` (lockdep annotations around the vblank timer). | `grep fence drivers/gpu/drm/vkms/*.c` at both tags | read from source |
| Since every plane gets the property, the overlay and cursor planes (`enable_overlay=1`, `enable_cursor=1`, as `scripts/vkms-vm-test.sh` sets them) carry it as well as the primary plane. | `drivers/gpu/drm/vkms/vkms_plane.c:vkms_plane_init` calls `drm_universal_plane_init`; the attach is in the core | inferred |

A client that checks "does the driver list the property" (the rule of
sync-fences §3.2) therefore finds both on vkms.

## 3. What the core does with them (vkms uses the atomic helpers)

| Step | Source | Tag |
| --- | --- | --- |
| `IN_FENCE_FD` set: a second set on the same plane state is `-EINVAL`; `-1` means no fence; otherwise `sync_file_get_fence(fd)` and a bad fd is `-EINVAL`. The fence is stored in `plane_state->fence`. The fd is not consumed (the caller still owns it). | `drivers/gpu/drm/drm_atomic_uapi.c:drm_atomic_plane_set_property` (v6.6 line 528, v7.3 line 551) | read from source |
| `IN_FENCE_FD` read back always returns -1. | `drm_atomic_uapi.c:drm_atomic_plane_get_property` (v6.6 line 617, v7.3 line 665) | read from source |
| `OUT_FENCE_PTR` set: a NULL pointer is ignored, otherwise the kernel writes -1 to the user pointer at once (so a faulting pointer fails the commit) and remembers the pointer for the CRTC. | `drm_atomic_uapi.c:drm_atomic_crtc_set_property` (v6.6 line 450 to 459) | read from source |
| At commit, for each CRTC with an out-fence pointer a pending vblank event is created even without `DRM_MODE_PAGE_FLIP_EVENT`, a fence is made by `drm_crtc_create_fence` and attached to that event (`event->base.fence`), and a sync_file fd is written to the pointer. With `DRM_MODE_ATOMIC_TEST_ONLY` the function returns first, so no fence is made. | `drm_atomic_uapi.c:prepare_signaling`, `setup_out_fence` (v6.6 line 1155 and 1134); `drivers/gpu/drm/drm_crtc.c:drm_crtc_create_fence` | read from source |
| A failed commit writes -1 to the out-fence pointer again. | `drm_atomic_uapi.c` doc comment of OUT_FENCE_PTR and the unwind in `drm_mode_atomic_ioctl` | read from source (comment); unwind not traced |
| Implicit sync is skipped when an explicit fence is set: `drm_gem_plane_helper_prepare_fb` only adds kernel fences when `state->fence` is already set, and otherwise adds the write fence of the buffer. vkms calls it from its `prepare_fb`. | `drivers/gpu/drm/drm_gem_atomic_helper.c:drm_gem_plane_helper_prepare_fb` (v6.6 line 135); `drivers/gpu/drm/vkms/vkms_plane.c:vkms_prepare_fb` (v6.6 line 160 and 172, v7.3 line 181 and 193) | read from source |

## 4. How the flip event follows the fence

### 4.1 In-fence

| Fact | Source | Tag |
| --- | --- | --- |
| A blocking commit (no `DRM_MODE_ATOMIC_NONBLOCK`) calls `drm_atomic_helper_wait_for_fences(dev, state, true)` before `drm_atomic_helper_swap_state`. That is an interruptible `dma_fence_wait` on every plane with a fence, then the fence is released. The ioctl therefore blocks until the fence signals; a signal interrupts it. | `drivers/gpu/drm/drm_atomic_helper.c:drm_atomic_helper_commit` (v6.6 line 2031, v7.3 line 2279) and `drm_atomic_helper_wait_for_fences` (v6.6 line 1594) | read from source |
| The wait with `pre_swap = false` is made in the default `commit_tail`, and only there. | `drm_atomic_helper.c:commit_tail` (v6.6 line 1816, v7.3 line 2064) | read from source |
| vkms installs its own `atomic_commit_tail`, so the default `commit_tail` helper is not used. That function runs `commit_modeset_disables`, `commit_planes`, `commit_modeset_enables`, `fake_vblank`, `commit_hw_done`, `wait_for_flip_done`, flushes the composer work and cleans up the planes. It has no fence wait. | `drivers/gpu/drm/vkms/vkms_drv.c:vkms_atomic_commit_tail` and `vkms_mode_config_helpers` (v6.6 line 64 and 147, v7.3 line 65 and 130) | read from source |
| Consequence: a nonblocking commit with a pending in-fence is not held back by vkms. The new framebuffer becomes the plane state, the composer reads it at the next simulated vblank and the flip event is delivered, whether or not the in-fence has signalled. vkms therefore does not enforce "no scanout of an unready buffer" for nonblocking commits. | the two rows above | inferred |
| An in-fence that is already signalled behaves like no fence in both modes. | `dma_fence_wait` on a signalled fence returns at once | inferred |

### 4.2 Out-fence and the vblank timer

| Fact | Source | Tag |
| --- | --- | --- |
| `vkms_crtc_atomic_flush` takes the pending event of the CRTC state. If `drm_crtc_vblank_get` fails, it sends the event at once (`drm_crtc_send_vblank_event`); otherwise it arms it for the next vblank (`drm_crtc_arm_vblank_event`). | `drivers/gpu/drm/vkms/vkms_crtc.c:vkms_crtc_atomic_flush` (v6.6 line 247, v7.3 line 170 to 183) | read from source |
| `drm_crtc_vblank_on` is called from the CRTC enable, so vblank is on while the CRTC is active and the armed path is the normal one. | `vkms_crtc.c:vkms_crtc_atomic_enable` (v6.6); v7.3 uses `drm_crtc_vblank_atomic_enable` | read from source |
| v6.6: vblank is produced by an `hrtimer` that `vkms_enable_vblank` starts with the period `vblank->framedur_ns` (the mode's frame duration). On each expiry `vkms_vblank_simulate` forwards the timer, calls `drm_crtc_handle_vblank` under `output->lock`, then queues the composer work. | `vkms_crtc.c:vkms_enable_vblank` (line 62), `vkms_vblank_simulate` (line 12) | read from source |
| v7.3: the hrtimer moved into the DRM core. `drm_crtc_vblank_start_timer` starts it, `drm_vblank_timer_function` forwards it and calls the CRTC hook `handle_vblank_timeout`, which vkms sets to `vkms_crtc_handle_vblank_timeout` (calls `drm_crtc_handle_vblank`, queues the composer work). Behaviour is the same. | `drivers/gpu/drm/drm_vblank.c:drm_vblank_timer_function` (line 2175); `vkms_crtc.c:vkms_crtc_handle_vblank_timeout` (line 16) | read from source |
| `drm_crtc_handle_vblank` delivers the armed event through `send_vblank_event`, which calls `drm_send_event_timestamp_locked`. `drm_send_event_helper` signals `e->fence` with the vblank timestamp (`dma_fence_signal_timestamp`) and drops the reference, then queues the user event. The sync_file from `OUT_FENCE_PTR` thus signals at the same moment the `DRM_EVENT_FLIP_COMPLETE` becomes readable. | `drivers/gpu/drm/drm_vblank.c:send_vblank_event` (v6.6 line 1024); `drivers/gpu/drm/drm_file.c:drm_send_event_helper` (v6.6 line 776 to 792, v7.3 line 757) | read from source |
| Timing: the delay from commit to out-fence signal is between 0 and one frame period. The composer work runs on a workqueue after the timer, and `vkms_atomic_commit_tail` flushes it (`flush_work`) before it returns, so the composed frame exists when a blocking commit returns. | `vkms_drv.c:vkms_atomic_commit_tail`; `vkms_crtc.c` | inferred |
| The out-fence is a "retire" fence of the commit: it means the commit's new state is on screen (here: latched at vblank). It is not a fence of the previous buffer's release. A release signal for the old buffer has to be derived from it (sync-fences §3.2). | `DRM_MODE_ATOMIC` doc of `OUT_FENCE_PTR` in `drm_atomic_uapi.c` | read from source (doc comment) |

## 5. `sync_file` sources inside a vkms VM

| Source | Works in vkms? | What it needs | Notes | Tag |
| --- | --- | --- | --- | --- |
| CRTC `OUT_FENCE_PTR` of an earlier commit, used as the next commit's `IN_FENCE_FD` | Yes | nothing beyond vkms (`SYNC_FILE` is selected by `DRM`) | Always available. The fence signals at the next simulated vblank, so it is short-lived: it tests the fd plumbing and the "fence from the same device" path, not a long wait. | read from source |
| Writeback out-fence (`WRITEBACK_OUT_FENCE_PTR` on the writeback connector), used as an in-fence | Yes, on default parameters | `enable_writeback` (default true at both tags: `vkms_drv.c:enable_writeback`), `DRM_WRITEBACK` support in the kernel build | The writeback connector commit queues a job (`vkms_writeback.c:vkms_wb_atomic_commit` calls `drm_writeback_queue_job`) and sets `wb_pending`. `vkms_composer_worker` composes into the writeback buffer and calls `drm_writeback_signal_completion(&out->wb_connector, 0)`, which signals the fence made by `drm_writeback_get_out_fence`. So it signals one composer run after the next vblank. Per the property doc, user space has to wait for it before another commit touching the same CRTC, planes or connector. | read from source |
| `sw_sync` timeline (`/sys/kernel/debug/sync/sw_sync`) | Only if the kernel has `CONFIG_SW_SYNC` | `SW_SYNC` (bool, default n, `depends on SYNC_FILE` and `DEBUG_FS`) and `debugfs` mounted | Open the file, `SW_SYNC_IOC_CREATE_FENCE` (`_IOWR('W', 0, struct sw_sync_create_fence_data)`) returns a sync_file fd for a chosen value on the timeline, and `SW_SYNC_IOC_INC` (`_IOW('W', 1, __u32)`) advances the timeline and signals fences up to it. This is the only source that stays pending until the test decides, so it is the one that can prove "the commit waits" and "a never-signalled fence is a timeout". Needs root. | read from source: `drivers/dma-buf/Kconfig:SW_SYNC`; `drivers/dma-buf/sync_debug.c` (creates `sync/sw_sync`, v6.6 line 176 to 185, v7.3 line 117 to 119); `drivers/dma-buf/sw_sync.c` (ioctl numbers, v6.6 line 55 to 60) |
| A fence exported from a dma-buf (`DMA_BUF_IOCTL_EXPORT_SYNC_FILE`) | Not examined | | Not looked at: vkms buffers are shmem GEM objects with no producer to hold a fence. | inferred |

The existing `examples/writeback.rs` already uses a writeback out-fence, so a
writeback fence needs no kernel change. For a test that must hold a fence
pending, `sw_sync` is the only choice.

## 6. Kernel config the fence test needs

What the VM must have, read from the Kconfig of the tags above. Only the
repository's VM script and init were read; no kernel config file of any
machine was read or run.

| Option | Need | Why | Tag |
| --- | --- | --- | --- |
| `CONFIG_DRM_VKMS` (=m, because the script loads a module) | already required by `scripts/vkms-vm-test.sh` | the device | read from source |
| `CONFIG_DRM` -> `SYNC_FILE` (selected) | implied | the property code and sync_file fds | read from source |
| writeback connector support | nothing extra: `drivers/gpu/drm/vkms/Makefile` lists `vkms_writeback.o` without a condition (v6.6 line 9); the core writeback code is not traced to a Kconfig symbol | writeback out-fence source | read from source (Makefile); core symbol not traced |
| `CONFIG_DEBUG_FS=y` | only for `sw_sync` | `SW_SYNC` depends on it | read from source |
| `CONFIG_SW_SYNC=y` | only for `sw_sync`; it is a `bool`, so it cannot be a module and the script's module copying cannot supply it | pending fences under test control | read from source |
| `CONFIG_SYNC_FILE=y` | implied by `DRM` | | read from source |

How the VM provides the rest, from this repository:

- `tools/vm-init/src/main.rs` mounts `devtmpfs` on `/dev` and `debugfs` on
  `/sys/kernel/debug`, so the `sw_sync` file would be at
  `/sys/kernel/debug/sync/sw_sync` once the kernel has `SW_SYNC` (read from
  source). It runs as PID 1, so it is root.
- `scripts/vkms-vm-test.sh` boots the host's kernel image
  (`VKMS_KERNEL`, default `/usr/src/linux/arch/x86/boot/bzImage`) and copies
  `vkms.ko` and its dependencies from the matching module directory with
  parameters `enable_overlay=1 enable_cursor=1`. It does not pass
  `enable_writeback`, so the default (true) applies (read from source). It
  rejects compressed modules.
- So `SW_SYNC` is a property of the kernel that `VKMS_KERNEL` names. Whether
  the node's kernel has it has to be read from that kernel's config; this
  note did not read it. If it is missing, a fence test can still use the
  CRTC out-fence and writeback sources and has to skip the "stays pending"
  cases, or run against a kernel built with `CONFIG_SW_SYNC=y`.
- In v7.3 `vkms_drv.c` also has `create_default_dev` and
  `enable_plane_pipeline` parameters. The script's `enable_overlay` and
  `enable_cursor` parameters still exist at v7.3.6 (read from source).

## 7. What this means for the fence test and the design

1. `IN_FENCE_FD` discovery on vkms will succeed, so a driver-side check that
   "the plane lists the property" is exercised by vkms (read from source).
2. To show that a commit really waits for an in-fence, use a blocking
   commit with a `sw_sync` fence, signal it from a second thread after a
   delay and assert the commit took at least that long. A nonblocking commit
   with a pending in-fence would pass on vkms without the driver waiting
   (section 4.1, inferred). A nonblocking test can still check that the
   event arrives and the out-fence fd is valid.
3. The out-fence test: pass `OUT_FENCE_PTR`, check the fd is set (not -1),
   poll it and expect it to become readable within about one frame period
   after the flip event (section 4.2).
4. A rejected or test-only commit leaves the out-fence pointer at -1;
   `TEST_ONLY` never produces a fence (section 3). The backend must not close
   -1.
5. A vkms result is a virtual device result. It says nothing about the
   i.MX95 DPU, which is a different driver; see the warning in
   sync-fences §7 and never cite vkms timing for hardware.

## 8. Not covered

- Whether the host kernel behind `VKMS_KERNEL` has `CONFIG_SW_SYNC` (not read).
- vkms behaviour when the in-fence signals with an error (`dma_fence_wait`
  returns the fence error in `drm_atomic_helper_wait_for_fences`, so a
  blocking commit would fail; not traced to the ioctl return value).
- Kernels between v6.6 and v7.3.6 (the two tags are the evidence; nothing in
  between was read).
