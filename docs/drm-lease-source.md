# What a DRM lessee can see and TEST_ONLY (mainline 6.18)

Question: when the probe or the planner runs through a DRM lease (the
NanoPC-T6 kernel lease: primary 33 plus overlays 45/51/57/63/69/75, observed
2026-11-27), what does it see compared with a run as DRM master? Answered from
mainline source only; no board was touched.

Tags: **read from source** (file:function, tag, read date) or **inferred**.
Every statement below was written on 2026-11-28.

## 1. Pin and how the source was read

- Tag `v6.18` of `torvalds/linux` is the commit
  `7d0a66e4bb9081d75c82ec4957c50034cb0ea449` (annotated tag object
  `f7b88edb52c8dd01b7e576390d658ae6eef0e134`, "Linux 6.18", 2025-11-30).
  Read from source, 2026-11-28, via the GitHub refs/tags API.
- Files (all under `drivers/gpu/drm/`, tag v6.18):
  `drm_lease.c`, `drm_mode_object.c`, `drm_plane.c`, `drm_atomic_uapi.c`, each at
  https://raw.githubusercontent.com/torvalds/linux/v6.18/drivers/gpu/drm/ plus
  the file name.
- **Fetch caveat (inferred reliability).** `curl` from the work shell was
  refused by the sandbox, so the files could not be saved or line-numbered under
  `$CONNECTED_DEPS_DIR`. They were read through a web-fetch helper that returns
  text without line numbers. References are therefore `file:function`, no line
  numbers. For `drm_plane.c` only the first 39,375 of 56,582 characters came back
  verbatim; the part past that (`drm_mode_cursor_common`, `drm_mode_page_flip_ioctl`)
  is not verified here. Quotes were relayed by the helper, not diffed against a
  local copy. Re-check the quoted lines on a local copy before relying on them
  for code (inferred, 2026-11-28).
- Not read (outside the four files): `drm_crtc.c` (`drm_mode_getcrtc`,
  `drm_mode_getresources`), `drm_connector.c` (`drm_mode_getconnector`),
  `drm_encoder.c`. See section 6.

## 2. Which objects a lessee enumerates

| # | Statement | Tag | Where |
|---|-----------|-----|-------|
| E1 | A lessee holds an object if its id is in the lessee's `leases` IDR; an owner (no `lessor`) holds everything. | read from source, 2026-11-28 | `drm_lease.c:_drm_lease_held_master`, `drm_lease_held` |
| E2 | `DRM_IOCTL_MODE_GETLEASE` returns the lessee's own `leases` IDR for a lessee and the whole `mode_config.object_idr` for an owner. Only CRTC, connector and plane ids are lease-controlled; framebuffers, blobs and properties are not. | read from source, 2026-11-28 | `drm_lease.c:drm_mode_get_lease_ioctl`; `drm_mode_object.c:drm_mode_object_lease_required` |
| E3 | `GETPLANERESOURCES` lists a plane only if `drm_lease_held(file_priv, plane->base.id)`, after the usual universal-planes / cursor-hotspot skips. A lessee therefore enumerates exactly the leased planes (primary 33 and the overlays 45/51/57/63/69/75 on the T6), and nothing else. | read from source, 2026-11-28 | `drm_plane.c:drm_mode_getplane_res` |
| E4 | The lessor decides what is in the lease: each requested id must exist (`-ENOENT`), must be a CRTC, connector or plane (`-EINVAL`), must not be leased to another lessee (`-EBUSY`); at least one CRTC and one connector are required. | read from source, 2026-11-28 | `drm_lease.c:fill_object_idr`, `validate_lease`, `drm_lease_create` |
| E5 | If the lessor has `universal_planes` off, the lease gets each leased CRTC's primary (and cursor) plane added automatically. With it on, planes are not added; the request must name them and include at least one plane. So a lease that contains overlays is built by a lessor with universal planes on and explicit plane ids. | read from source, 2026-11-28 | `drm_lease.c:fill_object_idr` |
| E6 | CRTCs and connectors reach the lessee through the same `leases` IDR. `GETRESOURCES` filtering (`drm_mode_getresources`) lives in `drm_crtc.c`, not read; it is expected to list only leased CRTCs and connectors. | inferred, 2026-11-28 | `drm_crtc.c:drm_mode_getresources` (not read) |
| E7 | The T6 lessee's CRTC and connector set is not recorded by the probe's lease note; the observation only names planes. The lease's CRTC is assumed to be video_port0 (the plane reach note says 39 and 69 cannot reach VP0). | inferred, 2026-11-28 | docs/vc4-plane-reach.md style reach tables; plan #457 scen-t6 handoff |

## 3. possible_crtcs for a lessee

| # | Statement | Tag | Where |
|---|-----------|-----|-------|
| P1 | `GETPLANE` returns `possible_crtcs = drm_lease_filter_crtcs(file_priv, plane->possible_crtcs)`. | read from source, 2026-11-28 | `drm_plane.c:drm_mode_getplane` |
| P2 | For an owner (or no file/master) the mask is returned unchanged. For a lessee it is **masked and renumbered**: walk `crtc_list`, `count_in` is the CRTC's device index, `count_out` counts only leased CRTCs; if the input bit `count_in` is set and the CRTC is leased, output bit `count_out` is set. Bits of non-leased CRTCs are dropped and the leased CRTCs are packed from bit 0. | read from source, 2026-11-28 | `drm_lease.c:drm_lease_filter_crtcs` |
| P3 | Consequence: the bit position in a lessee's `possible_crtcs` is the index among the leased CRTCs in device order, not the device CRTC index. With one leased CRTC, bit 0 means "that CRTC" and an unreachable plane reports 0. | inferred (from P2), 2026-11-28 | same |
| P4 | `GETPLANE` also reports `crtc_id` only if the plane's current CRTC is leased (else 0), while `fb_id` is **not** lease-filtered. | read from source, 2026-11-28 | `drm_plane.c:drm_mode_getplane` |
| P5 | The CRTC index used by `drm_mode_getplane_res` clients to map `possible_crtcs` bits comes from the CRTC id order of `GETRESOURCES`; for a lessee that list is the leased subset, which matches P2's renumbering. | inferred, 2026-11-28 | `drm_crtc.c:drm_mode_getresources` (not read) |

## 4. TEST_ONLY and non-leased objects

| # | Statement | Tag | Where |
|---|-----------|-----|-------|
| T1 | Object lookup for a file goes through `__drm_mode_object_find`: a CRTC, connector or plane that is not held by the file's lease returns NULL, the same as a missing object ("not included in lease"). | read from source, 2026-11-28 | `drm_mode_object.c:__drm_mode_object_find` |
| T2 | In the atomic ioctl each top-level object id is looked up with `drm_mode_object_find(dev, file_priv, ...)`; failure gives `-ENOENT`. TEST_ONLY only changes the last step (`drm_atomic_check_only` instead of commit); parsing and lookups run in full, so a TEST_ONLY request naming a non-leased plane, CRTC or connector fails with `-ENOENT` before any hardware check. | read from source, 2026-11-28 | `drm_atomic_uapi.c:drm_mode_atomic_ioctl` |
| T3 | A plane's or connector's `CRTC_ID` value is resolved with `drm_crtc_find(dev, file_priv, val)`; a nonzero value that does not resolve (including a non-leased CRTC) returns `-EACCES`. Value 0 is allowed. | read from source, 2026-11-28 | `drm_atomic_uapi.c:drm_atomic_plane_set_property`, `drm_atomic_connector_set_property` |
| T4 | That `drm_crtc_find` returns NULL for a non-leased CRTC is by T1 (it is a thin wrapper over the object lookup; the wrapper is in `drm_crtc.h`, not read). | inferred, 2026-11-28 | `drm_mode_object.c:__drm_mode_object_find` |
| T5 | `FB_ID` uses `drm_framebuffer_lookup`; framebuffers are not lease-controlled, so a lessee can attach any framebuffer id it can find. A missing fb gives no error at the setter. | read from source, 2026-11-28 | `drm_atomic_uapi.c:drm_atomic_plane_set_property`; `drm_mode_object.c:drm_mode_object_lease_required` |
| T6 | `drm_atomic_set_crtc_for_plane` has no lease check and no `file_priv`; the lease is enforced only at lookup. A leased plane given a leased CRTC it cannot reach (T6 plane 69 on video_port0) passes the lease layer and is rejected later by the driver's atomic check. | read from source, 2026-11-28 | `drm_atomic_uapi.c:drm_atomic_set_crtc_for_plane` |
| T7 | Only objects in the request are checked by lease. Atomic check (`drm_atomic_check_only`) can still pull in non-requested state of leased or unleased CRTCs of the device (for example a shared VOP2 resource); whether the lessee's check can fail because of that is not decided by these files. | inferred, 2026-11-28 | `drm_atomic_uapi.c:drm_mode_atomic_ioctl` |
| T8 | Error map for a probe: unleased plane / CRTC / connector as object: `-ENOENT`; unleased CRTC as `CRTC_ID`: `-EACCES`; unreachable-but-leased pair: driver's `-EINVAL` from atomic check (the common case, not read here). | inferred, 2026-11-28 | combined T1-T3 |

## 5. IN_FORMATS and other properties: lessee vs master

| # | Statement | Tag | Where |
|---|-----------|-----|-------|
| F1 | The `IN_FORMATS` (and `IN_FORMATS_ASYNC`) blobs are created once at plane init, with no `drm_file`; `create_in_format_blob` takes no file or lease argument. Every client gets the same blob id and contents. | read from source, 2026-11-28 | `drm_plane.c:create_in_format_blob`, `__drm_universal_plane_init` |
| F2 | `GETPLANE`'s legacy format array is `plane->format_types` copied whole, no lease or modifier filtering. | read from source, 2026-11-28 | `drm_plane.c:drm_mode_getplane` |
| F3 | `OBJ_GETPROPERTIES` finds the object (lease check, `-ENOENT` if not leased) and then returns all its properties; `drm_mode_object_get_properties` takes no `file_priv`, so there is no per-property lease filtering. The only filter is the atomic-capability one (`DRM_MODE_PROP_ATOMIC` properties hidden from clients without `DRM_CLIENT_CAP_ATOMIC`), same for master and lessee. | read from source, 2026-11-28 | `drm_mode_object.c:drm_mode_obj_get_properties_ioctl`, `drm_mode_object_get_properties` |
| F4 | So, on a leased plane, `IN_FORMATS`, `zpos` range, `rotation`, `COLOR_*`, `SCALING_FILTER` and any driver property read identically for lessee and master, provided the lessee enabled the same client caps. Property values (as opposed to definitions) read the plane's current state, which may differ with what the master displays (for example `CRTC_ID`, `FB_ID`, `zpos` value). | inferred (from F1-F3), 2026-11-28 | same |
| F5 | Blob contents are fetched by blob id (`GETPROPBLOB`); blobs are not lease-controlled, so a lessee can read the `IN_FORMATS` blob of a leased plane. Not read in these four files (`drm_property.c`). | inferred, 2026-11-28 | `drm_mode_object.c:drm_mode_object_lease_required` |

## 6. What this means for a probe run through a lease (inferred)

All items inferred, 2026-11-28, from sections 2 to 5:

1. **Plane list is a subset.** On the T6 a lessee sees seven planes (33, 45,
   51, 57, 63, 69, 75), not eight; plane 39 is invisible. A probe counting
   planes will report 7 where a master run reports 8. Any fixture or profile
   test for the T6 that expects 8 planes must be fed a master run, or must
   name the lease subset explicitly.
2. **`possible_crtcs` is renumbered.** A lessee's bits are indices into the
   leased CRTC list. A probe that takes bit i as "device CRTC i" (as the
   master-run fixtures do) is wrong under a lease unless the leased CRTCs are
   exactly the first n. For the T6 lease (one CRTC, assumed video_port0)
   bit 0 means VP0. Plane 69 is leased but cannot reach VP0, so its mask is
   0 for the lessee; the probe sees "leased plane that reaches no leased
   CRTC", which a master run would show as a mask with some other VP bit set.
   The probe cannot tell from the lessee's view which VP that was.
3. **Cross-output reach (the blind spot) shrinks.** Planes shared with other
   VPs are only visible through leased CRTCs, so a lessee run cannot measure
   the full reach table that the master run records (for example the
   `docs/vc4-plane-reach.md` style per-output reach). Reach data must come
   from master runs.
4. **Formats and properties match.** IN_FORMATS, zpos ranges, rotation and
   scaling properties of the seven visible planes should equal the master's
   (F4); a lessee run is valid evidence for per-plane format and property
   tables, as long as the 32-pair probe cut is the same.
5. **TEST_ONLY is restricted to the lease.** A sweep over all eight planes
   cannot be run as lessee; the largest window count a lessee can test is the
   leased-plane count that reach the leased CRTC (six on T6: 33, 45, 51, 57,
   63, 75). The "up to 8 active windows" sweep is a master-only run. Errors
   on out-of-lease ids are `-ENOENT` / `-EACCES` (T8) and must not be counted
   as planner/TEST_ONLY disagreements.
6. **Shared-resource effects are invisible.** If a TEST_ONLY on the lessee
   fails because of hardware shared with an unleased CRTC (T7), the lessee
   sees only the errno. Unleased output state can also make a TEST_ONLY pass or
   fail differently from a master run on the same device.
7. **Evidence rule.** Per the plan, lease runs are portability evidence. A
   profile whose constants (plane count, per-plane reach, 8-layer budget) were
   taken from a lessee run would be wrong for a master; label such fixtures
   "lessee view" with the leased id set.

## 7. Open points for a later read (not part of this item)

- Read `drm_crtc.c`, `drm_connector.c`, `drm_encoder.c` for E6, P5 and the
  connector/encoder masking the `drm_lease.c` DOC comment describes.
- Re-fetch the four files into `$CONNECTED_DEPS_DIR` once `curl` is permitted
  and add line numbers (section 1 caveat).
- Confirm the T6 lease's CRTC set on the board (E7); only planes are recorded.
