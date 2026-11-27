/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * struct ce_twoplane_hw for the AM62P DSS driver of the demo module
 * (include/mcui/am62p_dss.h, drivers/display/display_am62p_dss.c there).
 *
 * NOT WRITTEN YET. This file was written without the demo checkout
 * (MCUI_SHOWCASE_DIR was not set where it was written), so the driver's
 * calls are not known here and are not guessed. Until it is written the
 * firmware does not build: see docs/board-test-am62p-r5f.md.
 *
 * What each member must do, as ce_twoplane.h asks (every call that writes
 * the planes counts one commit; the writes of a commit reach the panel
 * together at a frame start; latched_count reaches commit_count once they
 * have):
 *
 *   app_format_ok         whether the application plane takes CE_FORMAT_*
 *   set_app_buffer        point the application plane at buf->pa, buf's
 *                         size, stride and format; nonzero: nothing written
 *   enable_app_plane      the application plane on or off
 *   safety_flip           present the back safety buffer at the next frame
 *   safety_buffer         the safety buffer not on the plane, and the panel's
 *                         width, height and stride
 *   safety_front          the safety buffer the plane scans now
 *   safety_premultiplied  the safety plane's premultiplied blending
 *   commit_count          the driver's commit counter (GO requests)
 *   latched_count         the commits the DSS has latched (GO bit cleared)
 *   set_vsync_callback    run cb(dev, user) in the driver's vsync interrupt
 *
 * Bind them to the driver's functions only; do not take the demo's
 * display_port_am62p.c or its copy of the backend.
 */
#include "ce_twoplane.h"

#error "hw_am62p.c: bind struct ce_twoplane_hw to the demo module's am62p_dss.h (not written yet)"

extern const struct ce_twoplane_hw ce_twoplane_hw_am62p;
