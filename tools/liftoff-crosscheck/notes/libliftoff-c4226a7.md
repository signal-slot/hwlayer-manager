# libliftoff c4226a7: reading notes for the cross-check

Written 2026-11-28 (item lo-read2 of the cross-check plan). These notes complete section 9 of `docs/libliftoff-crosscheck.md` and correct nothing in it; where this reading differs from the design, section 10 says so. They are paraphrase and identifiers only. libliftoff is MIT-licensed and this repository is Apache-2.0, so no source text is copied here and none may be added.

## 0. Provenance

| Item | Value |
| --- | --- |
| Commit | `c4226a79b7a52f59bb51789b1eda112658609125` (`c4226a7`) |
| Fetch method | **git clone, commit confirmed from the ref files, `git rev-parse` not run.** `git clone https://gitlab.freedesktop.org/emersion/libliftoff.git` into `$CONNECTED_DEPS_DIR/libliftoff` succeeded when the global and system git config were switched off (the default config failed with a credential-helper error). The sandbox then refused any further git command in that directory (`rev-parse`, `checkout`), so the check was made by reading `.git/refs/heads/master` and `.git/packed-refs` (`refs/remotes/origin/master`): both name `c4226a7`, and the working tree is the checkout of that branch. That is a check against git's own refs, not the output of `git rev-parse`. |
| Read on | 2026-11-28 |
| Files read in full | `output.c`, `include/private.h`, `include/libliftoff.h`, `alloc.c`, `plane.c`, `layer.c`, `device.c`, `test/test_alloc.c`, `test/test_prop.c`, `test/meson.build`, `test/libdrm_mock.c`, `test/libdrm_mock.h`, `README.md` |
| Also read | libdrm headers from the project's libdrm checkout in `$CONNECTED_DEPS_DIR/libdrm` (`xf86drmMode.h`, `include/drm/drm_mode.h`), for struct layouts and flag values. Its version was not checked. |
| Not read | `test/test_dynamic.c`, `test_priority.c`, `test_candidate.c`, `bench.c`, `doc/compositor.md`, `example/`, `meson.build` (root) |

Tags: **source (lo@c4226a7)** = read in the files above, file and function named; **recalled** = prior knowledge, not in anything read; **inferred** = follows from the source but was not run. Nothing here was run: no libliftoff code was compiled or executed for these notes.

## 1. Output to CRTC index (the `possible_crtcs` test)

- `liftoff_device_create` copies the CRTC id array of `drmModeGetResources` in the order libdrm returns it, into `device->crtcs` (`crtcs_len` = `count_crtcs`). **source (lo@c4226a7)** `device.c`.
- `liftoff_output_create(device, crtc_id)` searches `device->crtcs` linearly for the id and stores the position as `output->crtc_index`. If the id is not in the list it returns NULL. There is no other lookup (no `drmModeGetCrtc`, no property read). **source** `output.c`.
- The compatibility test is `check_plane_output_compatible` in `alloc.c`: bit `crtc_index` of `plane->possible_crtcs` (the value of `drmModeGetPlane`, read once at `liftoff_plane_create`). **source**
- Details that matter for the fake: the shift is written on an unsigned long and the mask is 32 bits wide, so only CRTC indices 0..31 are meaningful (**inferred**). The index is the position in the fake's `drmModeRes.crtcs`, so the fake must list CRTCs in the xcheck `output` order; then core's output index and libliftoff's `crtc_index` are the same number and `outputs=<mask>` can be copied to `possible_crtcs` unchanged. `crtc_id` is also what `plane_apply` writes into each plane's `CRTC_ID`.
- The same test also drives `remaining_compatible_planes` (count of planes whose bit is set), used for score pruning (section 8). Planes not compatible with the output are still walked (as "skip") and are still disabled in the request (section 4).

## 2. Struct layouts and the core property enum that matter to a fake

### 2.1 Core property enum (`include/private.h`, **source**)

`enum liftoff_core_property`, in this order: `FB_ID`, `CRTC_ID`, `CRTC_X`, `CRTC_Y`, `CRTC_W`, `CRTC_H`, `SRC_X`, `SRC_Y`, `SRC_W`, `SRC_H`, `ZPOS`, `ALPHA`, `ROTATION`, then `LAST`. The names are matched by `core_property_index` in `device.c` against these exact strings: `FB_ID`, `CRTC_ID`, `CRTC_X/Y/W/H`, `SRC_X/Y/W/H`, `zpos`, `alpha`, `rotation` (note the lowercase of the last three; all case-sensitive). Everything else (`type`, `IN_FORMATS`, `pixel blend mode`, `SCALING_FILTER`, `FB_DAMAGE_CLIPS`, `IN_FENCE_FD`, ...) is matched by plain name.

### 2.2 libliftoff's own structs (`private.h`, **source**)

- `liftoff_device`: duped fd, plane list (order matters, section 2.3), output list, CRTC id array, `planes_cap` (= `count_planes` of `drmModeGetPlaneResources` at creation: size of every layer's candidate array), counters.
- `liftoff_output`: `crtc_id`, `crtc_index`, layer list (in priority order after each apply, section 9), optional composition layer, `layers_changed`, reuse counter.
- `liftoff_layer`: dynamic property array (name up to `DRM_PROP_NAME_LEN`, value, previous value, core index), `core_props[]` indices, `force_composition`, assigned plane, candidate plane id array of length `planes_cap`, current and pending priority, `changed`, cached `fb_info` and `prev_fb_info` (`drmModeFB2`).
- `liftoff_plane`: `id`, `possible_crtcs`, `type`, `zpos` (a plain `int`; "greater means closer to the eye"), `props[]` (kept `drmModePropertyRes` pointers), `in_formats_blob`, `core_props[]` pointers, assigned layer.

### 2.3 Plane order and the plane `zpos` (`plane.c`, `liftoff_plane_create`, **source**)

- `type`: value of the property named `type`; a plane without that property is refused (`errno` EINVAL). Primary = 1, overlay = 0, cursor = 2 are the `DRM_PLANE_TYPE_*` values (**source**, used by name; numeric values **recalled**).
- `zpos`: the **current value** of the property named `zpos`, narrowed to `int`. A plane without it gets a guess from its type: primary 0, cursor 2, overlay 1, except an overlay whose id is below the id of the *first plane already in the list* gets -1 (the code names it "primary" but it is just the list head; **inferred**: it is the primary only if a primary was registered first).
- Insertion: a primary plane goes to the front (a later primary is placed before an earlier one). A non-primary plane is inserted just before the first non-primary plane whose zpos it is greater than or equal to; otherwise at the end. So the list is primary planes first, then others by descending zpos, and **among equal zpos the plane registered later comes first**. With `liftoff_device_register_all_planes` the registration order is the order of `drmModeGetPlaneResources`.
- The search (section 8) walks this list. Order is therefore a function of: type, zpos value (or guess), and the order of the plane id array in the fake. The fake must return the plane ids in a stated order (xcheck line order is the natural choice).

## 3. libdrm calls and the fields libliftoff reads

All **source (lo@c4226a7)**; the file is named per row. Fields not listed are not read.

| Call | Where | Fields read from the result | Notes for the fake |
| --- | --- | --- | --- |
| `drmModeGetResources(fd)` | `device_create` | `count_crtcs`, `crtcs[]` | NULL fails device creation. Freed with `drmModeFreeResources`. |
| `drmModeGetPlaneResources(fd)` | `device_create` (count only), `register_all_planes` (count and `planes[]`) | `count_planes`, `planes[]` | `drmModeGetResources` gets the caller's original fd; every later call (this one, `GetPlane`, properties, FB, commit) gets the **dup**. A fake that checks the fd must accept both (**source** `device.c`). |
| `drmModeGetPlane(fd, id)` | `plane_create` | `plane_id`, `possible_crtcs` | Nothing else (not `crtc_id`, `fb_id`, formats, gamma). |
| `drmModeObjectGetProperties(fd, id, DRM_MODE_OBJECT_PLANE)` | `plane_create` | `count_props`, `props[]` (property ids), `prop_values[]` (current values) | Called with the plane object type. Values are used only for `type`, `zpos` and the `IN_FORMATS` blob id. |
| `drmModeGetProperty(fd, prop_id)` | `plane_create`, once per property | `name`, `prop_id`, `flags`, `values[]`, `count_enums`, `enums[].value` | See below. NULL fails the plane (and registration). The result is kept for the plane's lifetime. |
| `drmModeGetPropertyBlob(fd, blob_id)` | `plane_create`, only for the property named `IN_FORMATS` | `data` only (not `length`, not `id`) | NULL fails the plane. |
| `drmModeGetFB2(fd, fb_id)` | `layer_cache_fb_info` | `fb_id`, `width`, `height`, `pixel_format`, `modifier`, `flags`, `handles[4]` | See below. |
| `drmCloseBufferHandle(fd, handle)` | `layer_cache_fb_info` | return value | Called once per non-zero entry of `handles[]` (duplicates once). On failure it only logs an error. |
| `drmModeAtomicAddProperty`, `drmModeAtomicGetCursor`, `drmModeAtomicSetCursor` | `plane.c`, `alloc.c` | return value of Add (negative = error) | Section 4. |
| `drmModeAtomicCommit(fd, req, flags, NULL)` | `device_test_commit` | return value | Section 4. |
| `drmModeAtomicAlloc/Free` | caller (the driver), not libliftoff | | The request is created by the driver and passed to `liftoff_output_apply`. |
| `drmModeFree*` | everywhere | | Free functions of Res, PlaneRes, Plane, ObjectProperties, Property, PropertyBlob, FB2 must exist. |
| `drmModeGetPropertyType` | `plane_set_prop` | `flags` | A `static inline` of the libdrm header (`flags & (legacy type mask | extended type mask)`), not a symbol; the fake need not export it. |

### 3.1 `drmModePropertyRes` (`plane.c`, `plane_set_prop` and the `check_*_prop` helpers)

- Name matching is `strcmp` against the full name. Property identity for `drmModeAtomicAddProperty` is `prop_id`.
- If `flags` has `DRM_MODE_PROP_IMMUTABLE` the layer value is refused with -EINVAL, whatever the value (so an immutable `zpos`, which libliftoff never writes, is harmless, but an immutable `rotation` or `alpha` refuses every layer that sets one).
- Then, by `drmModeGetPropertyType(prop)`:
  - range (`DRM_MODE_PROP_RANGE`): value must be within `values[0]..values[1]` inclusive, unsigned. `count_values` is not consulted: the fake must allocate two entries.
  - signed range (`DRM_MODE_PROP_SIGNED_RANGE`): same, compared as signed 64-bit.
  - enum (`DRM_MODE_PROP_ENUM`): value must equal some `enums[i].value`, `i < count_enums`.
  - bitmask (`DRM_MODE_PROP_BITMASK`): the mask is the union of `1 << enums[i].value`; the value must have no bit outside it. So the enum values of a bitmask property are **bit positions** (as the kernel's `rotation` is), not masks.
  - any other type (including no type bits, `DRM_MODE_PROP_OBJECT`, blob): no check, the value goes to the request.
- If the plane has no property of the layer's property name, the plane fails with -EINVAL, except the skip rules of section 6.
- **Properties the plane code requires** (**source**): `plane_apply` needs `FB_ID` and `CRTC_ID` on **every** registered plane. If either is missing the "disable all planes" step (section 4) gets -EINVAL, and that step is guarded by an `assert` that the result is not -EINVAL, so an assert-enabled build aborts (**source**: `alloc.c`, `liftoff_output_apply`; whether the project's build defines NDEBUG was not read). An immutable or range-limited `FB_ID`/`CRTC_ID` would fail the same way; give them type bits 0 or object type.
- The mock in `test/libdrm_mock.c` registers the basic properties (`type`, `FB_ID`, `CRTC_ID`, `CRTC_X/Y/W/H`, `SRC_X/Y/W/H`) without flags (a comment there says flags are a TODO) and no `zpos`. So the library's own tests never exercise a plane `zpos` property; they exercise the type guess, except `test_prop.c` `immutable-zpos` (section 11).

### 3.2 `IN_FORMATS` blob (`plane.c`, `plane_check_layer_fb`)

- Layout used: the `drm_format_modifier_blob` header (`count_formats`, `formats_offset`, `count_modifiers`, `modifiers_offset`), a `u32` format array, and `drm_format_modifier` entries (64-bit `formats` bitmask, `offset`, `modifier`). `version`, `flags` and the blob `length` are not read.
- Algorithm: the first index of the FB's pixel format in the format array; the **first** modifier entry whose modifier equals the FB's; the format index must lie in that entry's window `offset .. offset + 63` and its bit (index minus offset) must be set. **inferred consequence:** if the same modifier appears in two entries (the kernel splits a plane with more than 64 formats into windows), only the first such entry is consulted, so a format that sits in a later window is rejected. This is a libliftoff limitation the fake cannot work around. A blob with at most 64 formats and one entry per modifier is read as expected, and that is what the fake should produce; if a plane has more than 64 formats in the xcheck file the writer must warn or refuse.
- The whole check is skipped (the plane accepts) if the layer's cached FB info has `fb_id == 0`, if `DRM_MODE_FB_MODIFIERS` (bit 1) is not set in its `flags`, or if the plane has no blob. A plane without `IN_FORMATS` therefore never rejects on format.

### 3.3 `drmModeGetFB2` (`layer.c`, `layer_cache_fb_info`, `alloc.c`, `update_layers_fb_info`)

- Called at the start of every `liftoff_output_apply`, for each layer of the output whose `FB_ID` is non-zero (the cache is cleared first, so it is always one call per such layer per apply).
- NULL return: if `errno` is EINVAL it is treated as "old kernel", the layer keeps an empty FB info, and the format check is skipped (the mock returns NULL with EINVAL for an unknown fb id). Any other errno is returned by `layer_cache_fb_info` but **the caller ignores it** (a TODO comment). So a missing fb leaves the layer without format information rather than failing.
- A fake must fill `fb_id` (the format check and the reuse logic compare it, and zero means "no information"), `width`, `height`, `pixel_format` (fourcc), `modifier`, and `flags` with `DRM_MODE_FB_MODIFIERS` for the format check to run. `handles[]` should be all zero, otherwise each handle triggers `drmCloseBufferHandle` and an error log if that fails (the mock returns -EINVAL from it).
- The width, height, format and modifier are used for nothing else in the plane search (**source**): there is no size or scaling check anywhere in libliftoff, and `SRC_*`/`CRTC_*` are only passed through (and `CRTC_X/Y/W/H` used for the intersection test).

## 4. Atomic request handling, test commits, cursor and rollback

- The request is opaque (`drmModeAtomicReq` is a typedef of an incomplete struct in `xf86drmMode.h`); the fake defines its own struct. libliftoff touches it only through Add, GetCursor, SetCursor and Commit (**source**).
- `liftoff_output_apply` first (unless the previous allocation can be reused, section 9) removes all plane-to-layer mappings of this output, then adds to the request, for **every plane registered on the device** that has no layer (so planes incompatible with the output and planes of other outputs' unused state too): `FB_ID = 0` and `CRTC_ID = 0` (**source** `alloc.c`, `plane.c`, `plane_apply` with a NULL layer). These sit at the front of the request for the whole search.
- At each node `output_choose_layers` records the cursor, then for a (plane, layer) pair calls `plane_apply`: `CRTC_ID = output->crtc_id` first, then every layer property except `zpos` in the layer's property order (so `FB_ID` and others in the order the driver set them). `FB_ID` and `CRTC_ID` are written again for planes that were disabled earlier in the same request: **the request therefore holds duplicate (object, property) pairs and the later one must win.** The mock resolves duplicates by scanning backwards from the cursor (**source**, `test/libdrm_mock.c`); the fake should do the same. (For real libdrm, how duplicates and cursor rewinding interact was not read; **recalled**: do not rely on it. It does not matter for a fake.)
- On a failed `plane_apply` (-EINVAL) the function rewinds to its own saved cursor (except when the very first step, `CRTC_ID`, fails, when nothing was added). On a plane/layer pair that is rejected by the force-composition test, the FB check, or the test commit, the caller rewinds with `drmModeAtomicSetCursor`. After the recursion into the next plane returns, it rewinds again, and after the "skip this plane" branch once more. **The fake must make `SetCursor(n)` truncate the property list to its first n entries and `GetCursor` return the current count; `AddProperty` appends and returns the new count (non-negative).** Anything else breaks backtracking.
- Test commit (`device_test_commit` in `device.c`): clears `DRM_MODE_PAGE_FLIP_EVENT` from the caller's flags, ORs in `DRM_MODE_ATOMIC_TEST_ONLY`, passes a NULL user data, loops while the result is -EINTR or -EAGAIN. Result 0 = this configuration is acceptable. -EINVAL, -ERANGE, -ENOSPC mean "no" and the search goes on (others log an error and **abort the search**: `output_choose_layers` returns them and `liftoff_output_apply` fails with that negative errno). Real libdrm reports errors as negative errno (**recalled**), so the fake returns negative values. The flags from `liftoff_output_apply` are passed through: a driver that passes 0 sees only `TEST_ONLY`; one that passes `DRM_MODE_ATOMIC_ALLOW_MODESET` would see that bit too.
- The request given to the test commit at a node contains: all planes disabled, then the chosen layers' plane settings in plane-list order up to this node. The fake's model sees the complete state each time, never a delta.
- At the end the winning allocation is applied again with `apply_current`: every plane in list order, either disabled or with its layer, appended to the caller's request. `liftoff_output_apply` itself makes no final commit; the caller does. The driver for the cross-check should not commit; it only reads `liftoff_layer_get_plane` and `liftoff_output_needs_composition`, `liftoff_layer_needs_composition`, and the candidate planes.
- The reuse path (section 9) issues **one** test commit and rewinds on failure.
- The `liftoff_device_destroy` closes the dup'd fd; the fake should give `dup` something valid (any open fd, e.g. a pipe end or `/dev/null`).
- The number of test commits is available only as `test_commit_counter` inside the device (private, logged at debug level), reset to zero at each non-reuse apply. A driver that wants the count must count calls in its own `drmModeAtomicCommit`. **inferred**.

## 5. `timeout_ns` and the deadline

- `liftoff_output_apply_options.timeout_ns`: zero (or `options == NULL`) selects the default of 1 ms; any other value is used as is, including negative ones (**source** `alloc.c`).
- `check_deadline(start, timeout_ns)` returns true (keep going) when `now - timeout_ns < start`, computed in signed 64-bit nanoseconds from `CLOCK_MONOTONIC` (clock errors are logged and return false). `start` is taken once in `liftoff_output_apply` after setup.
- Huge timeout: with `INT64_MAX` (what the library's own tests use, **source** `test_alloc.c`, `test_prop.c`), `now - INT64_MAX` is a large negative number and always below `start`, so the deadline never trips and the search is exhaustive. There is no overflow, since `now` is positive (**inferred**). A negative timeout makes `now - timeout_ns` larger than `now`, so it trips at once; the extreme `INT64_MIN` would overflow (undefined behaviour, **inferred**), so the driver must not use it. Recommend `INT64_MAX`.
- Where it is checked: only inside the layer loop of `output_choose_layers`, after the layer is found visible and compatible and **before** the best-score early exit and before `plane_apply`. When it trips, the loop is left with `break`, the code falls through to "try not to use this plane" and recurses on the next plane; every deeper plane does the same and so the remaining planes are visited as "skip" without further test commits, and the leaf compares the partial allocation of the path being walked with the best so far, accepting it if its score is higher and `check_alloc_valid` passes (**source**, matches the design's reading). Nothing is checked between a plane with no compatible layer and the next plane, so the walk past the deadline is short (bounded by planes times layers). Consequence: with a finite timeout the result is the best found so far, which depends on machine speed. The sweep must use the huge timeout so that results are deterministic.

## 6. `pixel blend mode` value 0 and the other skip rules

- Rule (**source** `plane.c`, `plane_apply`): when the plane has no property for a layer property, the property is skipped (no -EINVAL) if it is `alpha` with value 0xFFFF; `rotation` with value `DRM_MODE_ROTATE_0`; `SCALING_FILTER` with value 0; `pixel blend mode` with value 0 (a code comment calls this "pre-multiplied alpha"); or `FB_DAMAGE_CLIPS` with any value. `HOTSPOT_X/Y` are deliberately not skipped. Any other missing property fails the plane. The comparison is against the layer's value only; a plane that **has** the property gets the ordinary checks of section 3.1.
- `DRM_MODE_ROTATE_0` is `1 << 0` = **1** in the libdrm header read (**source**, `drm_mode.h`); a layer `rotation` value of 0 is not "unrotated" for libliftoff and fails planes lacking the property. The xcheck `transform=normal` must therefore write 1.
- Does the blend rule match DRM's enum values? The libdrm headers read here do not define the blend modes (they are not in the uapi header), so the values are **recalled**: the kernel's blend property enum is `None` = 0, `Pre-multiplied` = 1, `Coverage` = 2 (`drm_blend.h`, `DRM_MODE_BLEND_PIXEL_NONE`/`PREMULTI`/`COVERAGE`). On that recollection the libliftoff comment is **wrong about the name of value 0**: 0 is "None" (pixel alpha ignored). The rule still has a sensible reading, since a plane without the property blends in the kernel's default way, but the label is off, and it **does not distinguish** the modes: a layer with `Opaque` (0) is accepted on a plane without the property, while `Premultiplied` (1) and `Coverage` (2) are refused there. If the fake exposes the property with all three enum entries, the check is an ordinary enum check (section 3.1) and the skip never applies. This confirms the design's mitigation (always expose the property when `blend_modes` is listed) and says what happens otherwise: for a plane **without** blend modes, libliftoff accepts only `Opaque`. What the core planner does with a missing `blend_modes` was not part of this reading (xc-corenotes).
- `alpha` absent on the plane: only 0xFFFF passes; a layer alpha of 0 never reaches a plane (the layer is invisible, section 9). For an exposed `alpha` the check is the range check; the fake should give it flags range and values `[0, 0xFFFF]`.

## 7. What a layer is (visibility, properties)

- `liftoff_layer_set_property`: any name; `CRTC_ID` is refused with -EINVAL; a new name appends to the array; setting `FB_ID` clears `force_composition` (**source** `layer.c`). `liftoff_layer_set_fb_composited` sets `FB_ID` to 0 and `force_composition`.
- Visible (`layer_is_visible`): `alpha` property present with value 0 means invisible; otherwise a forced-composition layer is visible, any other needs a non-zero `FB_ID`. Invisible layers are skipped by the search, are not counted for the score, and `liftoff_layer_needs_composition` is false for them. **A layer without an `alpha` property is opaque**, so the driver need not set `alpha=0xFFFF`; but setting it makes planes without an `alpha` property still accept it (skip rule).
- Geometry for the intersection test comes from `CRTC_X/Y/W/H` (missing = 0, so an empty rectangle that intersects nothing). Rect values are read as 64-bit values truncated to `int`, so a negative `CRTC_X` must be passed as the sign-extended 64-bit value (**inferred**). The rectangles are open at the far edge. `SRC_*` are never read by libliftoff.
- `zpos` on a layer is used only for allocation; it is never written to the request (`plane_apply` skips it). Layers without `zpos` have no ordering constraints at all, and one layer with and one without `zpos`: the one with `zpos` sorts first (section 9).
- Layer `zpos` is narrowed to `int` for comparisons.

## 8. The search (for the model and for matching core's results)

**source** `alloc.c`, `output_choose_layers` and helpers, unless marked.

- Depth first over the plane list: at each plane, each visible, unassigned, compatible layer is tried in turn (in the layer order of the output), then "leave this plane empty" last. A plane that already has a layer (another output's) or is not compatible with the output goes straight to "skip".
- Leaf: when the plane list is exhausted, the allocation replaces the best only if its score (number of non-composition layers placed) is **strictly** greater and `check_alloc_valid` holds. So among equal scores the **first found** wins; first found is determined by the plane order and layer order. For identical scores, libliftoff's choice depends on layer order, which depends on priority and zpos (section 9).
- Pruning: before trying layers on a plane, `best_possible_score` = score so far + min(remaining scoring layers, remaining compatible planes). Inside the layer loop, if the best score already reaches it, the **whole node** returns, **including the "skip" branch** (so it is not a dead end only for the layer loop). The test is placed after the deadline check and after the compatibility checks for the current layer, so it is evaluated only when at least one compatible layer exists at that node (**inferred** from the code order).
- `check_layer_plane_compatible` (zpos rules, only when the layer has `zpos`): refuses (1) a layer already allocated; (2) if the layer's zpos is above the zpos of the last layer placed on a non-primary plane and an earlier-placed layer that it intersects has lower zpos than it (that earlier layer sits on a plane over the current one: `has_allocated_layer_over`); (3) if the layer's zpos is below the last placed one and an intersecting, already placed layer is on a plane with zpos not above this plane's (`has_allocated_plane_under`; this is the "equal plane zpos means undefined order" rule: two planes of equal zpos cannot hold intersecting layers whose order matters); (4) a non-primary plane for a layer whose zpos is below the primary layer's while the plane's zpos is above the primary plane's (no underlay). Then, independent of layer zpos: (5) a non-primary plane is refused if some other, unplaced **intersecting** layer with a higher zpos exists, since that layer would have to be composited on top (`has_composited_layer_over`); (6) the composition layer only goes on a primary plane.
- Composition layer: it is not counted in the score; `check_alloc_valid` requires: if a composition layer exists and was not used, every non-composition visible layer must have a plane; if it was used, not all of them may have one (it is refused when everything fits).
- A layer that was refused only by `force_composition` or by `plane_check_layer_fb` still becomes a candidate plane for the layer (`layer_add_candidate_plane`, readable through `liftoff_layer_is_candidate_plane`), because `plane_apply` accepted it; a layer refused by property checks (-EINVAL from `plane_apply`) does not. Candidate sets are reset at each non-reuse apply.
- The search never reads plane or layer size, format (other than `IN_FORMATS`), scaling or modifier except as said; every other hardware rule must come from the test commit.

## 9. Layer priority and `reuse_previous_alloc`

**source** `alloc.c`, `layer.c`, `private.h`.

- Each `liftoff_output_apply` begins with `update_layers_priority(device)`, `update_layers_fb_info(output)`, then `reuse_previous_alloc`. `LIFTOFF_PRIORITY_PERIOD` is 60 page flips (applies, in practice: the counter is per device and counts every `liftoff_output_apply` call).
- Priority: per layer a `pending_priority` is increased by one each apply in which the `FB_ID` differs from its previous value (`layer_update_priority`); it is moved into `current_priority` and cleared only when the device counter reaches 60. So on a **fresh device**, `current_priority` is 0 for every layer in the first 59 applies; only `zpos` and visibility affect the order then.
- Order (`update_layers_order`, called on every apply, even the first, **before** the reuse test): a selection sort of the output's layer list using `layer_is_higher_priority`: composition layer first; visible before invisible; if both have `zpos`, **intersecting** layers by higher zpos (equal zpos: higher current priority), non-intersecting layers by higher current priority and, when equal, by higher zpos; a layer with `zpos` before one without; with neither, by priority. The sort is a swap-based selection sort, so ties (equal zpos and equal priority) do not keep the original order in a defined way (**inferred**). **With distinct layer zpos values, as the harness assigns them (design section 4), the first apply on a fresh device orders layers by descending zpos**, so the layer with the highest zpos is tried first on each plane. That order, together with plane order, defines which of several equal-score allocations is found first.
- Reuse (`reuse_previous_alloc`): fails (-EINVAL, so a full search follows) when `layers_changed` (any layer created, destroyed, or the composition layer changed since the last apply; a new layer sets it), when any layer `layer_needs_realloc` (a property was added or force-composition changed; `FB_ID` went between zero and non-zero; the FB's width, height, format or modifier changed; `alpha` moved to or from 0 or 0xFFFF; any property other than the CRTC rectangle, `IN_FENCE_FD`, `FB_DAMAGE_CLIPS` and `alpha` changed value; or a CRTC rectangle changed so that its intersection with another layer changed), or when the sort moved layers. Otherwise it re-applies the previous mapping (`apply_current`) and makes one test commit; failure rewinds the request and returns the error, which also leads to a full search.
- In a **first** apply the request is empty and `layers_changed` is true, so reuse never succeeds and issues no test commit; the allocation is searched in full. The sweep should therefore create a fresh device, output and layers for every frame (cheap, and it removes every state dependence: priority, previous values, the cached FB info, the reuse path). Whether to also exercise reuse is an open question for the report; this reading says it is a separate behaviour with its own tests in `test_dynamic.c`, not read here.
- After a successful search `mark_layers_clean` copies current values to the previous ones; the output's `layers_changed` flag is cleared.

## 10. Where this reading differs from, or adds to, `docs/libliftoff-crosscheck.md`

- Confirmed: CRTC index from the order of `drmModeGetResources`; only `plane_id` and `possible_crtcs` of `drmModeGetPlane`; zpos is the current value; `libliftoff` never writes zpos; skip-rule list; the deadline test position; the plane order.
- Added: `FB_ID` and `CRTC_ID` must exist on every plane or the library asserts (section 3.1); the whole-node early return also skips the "skip" branch (section 8); the first-modifier-entry limitation of the `IN_FORMATS` check (section 3.2); `DRM_MODE_ROTATE_0` is 1 (section 6); layer order is by descending zpos with equal priority (section 9); the 60-apply priority period; the FB cache is filled on every apply and errors other than EINVAL are ignored.
- The design's remark that DRM's own `None` is value 0 is consistent with this reading (recalled, not in the files read).
- The "plane without `zpos` gets a guess" rule compares the id with the **first plane of the list**, not a known primary (section 2.3).
- All planes of the device are disabled in the request (also the ones of other CRTCs): the fake's model must tolerate disables of planes it does not consider.

## 11. Calibration frames available in the library's tests

Names are as in `test/meson.build`. All run on the same hardware: one CRTC, four planes registered in the order primary, cursor, overlay A, overlay B, with **no `zpos` property** (the zpos guess gives primary 0, cursor 2, overlays 1, so the resulting search order is primary, cursor, overlay B, overlay A, section 2.3), and with the test commit of the mock (a layer may sit only on planes declared compatible with it for the whole run; FB_ID and CRTC_ID set together; optionally "primary must be enabled when any plane is"). Each test applies once with `INT64_MAX` and checks the layer-to-plane map and `liftoff_output_needs_composition`. All 1920x1080 layers are primary-sized, the 100x100 and 200x200 ones small.

### 11.1 `test_alloc.c` (names; expected result paraphrased; **these are not copied code**)

Useful (they exercise a planner rule and need only plane compatibility, which an xcheck model can encode with plane formats or limits):

- `empty`: no layers; nothing placed, no composition.
- `simple-1x`, `simple-1x-fail`: one layer, one compatible plane / none (composition needed).
- `simple-3x`: three layers, each with its own plane (primary, cursor, overlay): all placed.
- `zero-fb-id-fail`: a layer with `FB_ID` 0 is not placed and does not need composition. `zpos-3x-zero-fb-id`, `zpos-3x-zero-alpha`: an invisible third layer (zero `FB_ID` / zero alpha) is ignored and the other two placed, no composition.
- `zpos-2x-fail`: an upper layer is compatible only with the cursor plane but its lower layer is the one on the primary; the cursor layer cannot be placed in this configuration and composition is needed (the rule: the layer on the cursor plane would be above the primary's layer, but the test says it is not placed) (**inferred reading of the expected values**, which list the cursor-only layer as unplaced).
- `zpos-2x-reverse`: the layer with the higher zpos is listed first and goes on the overlay, the lower one on the primary (order in the frame does not matter, zpos does).
- `zpos-3x`: three layers with zpos 1, 2, 3 on primary, an overlay and the cursor: all placed in zpos order.
- `zpos-3x-intersect-fail`, `zpos-3x-intersect-partial`: a layer that cannot get a plane forces the intersecting ones under it to composition; and a layer whose only plane would sit above a plane that must stay below a higher-zpos layer: only one of the two is placed.
- `zpos-3x-disjoint-partial`, `zpos-3x-disjoint`: the same constellations with **non-intersecting** rectangles: the zpos conflict disappears and more layers are placed.
- `zpos-4x-intersect-partial`: four intersecting layers, four planes, but two overlay planes with equal zpos have undefined order: only three planes are used. Directly tests the equal-plane-zpos rule (relevant to planes `zpos=N..M` in core, which may order any way).
- `zpos-4x-disjoint`, `zpos-4x-disjoint-alt`: the two overlay planes with equal zpos used by two non-intersecting layers (and the alternate assignment).
- `zpos-4x-domino-fail`, `zpos-4x-domino-partial`: a top layer that cannot be placed forces composition of the layers it intersects (domino), with variants where a middle layer is still placeable.
- `composition-3x`, `composition-3x-fail`, `composition-3x-partial`, `composition-3x-force`: composition layer on the primary plane; not used when everything fits; used (and so occupying the primary) when something does not fit; `force` has one layer with composition forced. Needs the composition-layer feature, which the design does **not** map (section 4 of the design: not set); list as "out of scope for v1", but they are the calibration for it.
- Setup note for translating: expectations here were built on guessed zpos 0/1/1/2 and plane order primary, cursor, overlay B, overlay A. An xcheck topology with the same four planes needs `zpos=fixed:0`, `fixed:2`, `fixed:1`, `fixed:1` and `zpos_now` equal to those, and ids ordered so that registration order matches (cursor before the two overlays, overlay A before overlay B).
- Compatibility per layer and plane is not an xcheck notion. To reproduce the frames, encode it through formats, a distinct fourcc per compatibility class, or through geometry limits in the topology (design sections 4 and 6), so that each layer's buffer is accepted by exactly the planes named in its `compat` list. This is a translation choice for xc-s7frames / lo-calib, not something the library requires.

Not useful: `basic`, `no-props-fail`, `composition-no-props` in `test_alloc.c` (the "main" cases other than the table: a smoke test with a single plane, a layer with no properties, and a composition layer without properties): they test the API contract, not allocation rules.

### 11.2 `test_prop.c` (names as in `test/meson.build`)

- `default-alpha`, `default-rotation`: a layer with a non-default value is refused by a plane lacking the property, accepted with the default value (0xFFFF / unrotated) and accepted on a plane with the property. Calibrates the skip rules (section 6). Useful.
- `ignore-alpha`: alpha 0 makes the layer invisible: not placed, no composition. Useful.
- `immutable-zpos`: two overlays with immutable zpos 1 and 2; layers with zpos 42 and 43 map in order; swapping the layer zpos swaps planes. Useful: shows the plane's zpos **value** orders the planes, the layer's zpos only its relative position, and immutability of `zpos` does not matter.
- `unmatched`, `unset`: a layer property no plane has (and not skippable) prevents placement; unsetting it allows placement. Useful for "unsupported property" frames (the xcheck v1 has no arbitrary properties; skip unless the format grows one).
- `in-formats`: the IN_FORMATS check by format and modifier, including the modifier-in-blob rule; first commit rejected (modifier absent), second accepted. Useful for the format-rejection path (needs the `DRM_MODE_FB_MODIFIERS` flag in the fake FB).
- `fb-damage-clips`: a layer property named `FB_DAMAGE_CLIPS` is ignored when the plane lacks it. Not mappable in xcheck v1.
- `range`, `signed-range`, `enum`, `bitmask`: libliftoff's own range/enum/bitmask checking of a single property, with the invalid value rejected without any test commit. Useful if the fake exposes `alpha`/`rotation`/blend with flags, values and enums; they confirm the shape the fake must give them (note: the files named `enum` and `bitmask` use the opposite flags from their names: `enum` uses `DRM_MODE_PROP_BITMASK` on `rotation` and `bitmask` uses `DRM_MODE_PROP_ENUM` on `COLOR_RANGE`; both tests do exercise both code paths, only the labels are swapped (**source**)).

## 12. Open points and limits of this reading

- Nothing here was compiled or run. The calibration expectations are what the tests assert, read from the tables, not reproduced.
- Real libdrm behaviours (the result convention of `drmModeAtomicCommit`, the semantics of `drmModeAtomicSetCursor` against duplicate properties) are **recalled**, not read: the fake defines its own.
- The libdrm version used for headers was not checked against the `>= 2.4.108` requirement in libliftoff's `meson.build` (**source**, per the design).
- `test_dynamic.c`, `test_priority.c`, `test_candidate.c` were not read; the reuse and priority sections above come from `alloc.c` and `layer.c`, not from those tests.
- Whether the project's libliftoff build defines `NDEBUG` (the `assert` on the disable step) was not read; the xc-cbuild script decides it.
