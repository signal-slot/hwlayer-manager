/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * Full-frame scanout test for the i.MX95 M7 DPU: draw a bar whose position
 * encodes the frame counter and time every display_write().
 */

#include <zephyr/kernel.h>
#include <zephyr/device.h>
#include <zephyr/drivers/display.h>
#include <zephyr/sys/printk.h>
#include <zephyr/sys/util.h>

#if !IS_ENABLED(CONFIG_IMX95_M7_SCANOUT_ENABLE_DPU)

int main(void)
{
	printk("imx95_m7_scanout: refusing to touch the DPU.\n");
	printk("Linux may own it. Rebuild with CONFIG_IMX95_M7_SCANOUT_ENABLE_DPU=y "
	       "only if the M7 owns the DPU on this board.\n");
	return 0;
}

#else

#define DISP_NODE DT_CHOSEN(zephyr_display)
#define WIDTH     DT_PROP(DISP_NODE, width)
#define HEIGHT    DT_PROP(DISP_NODE, height)
#define BAR_W     CONFIG_IMX95_M7_SCANOUT_BAR_WIDTH
#define BAR_STEP  CONFIG_IMX95_M7_SCANOUT_BAR_STEP
#define CNT_BITS  16U
#define CNT_CELL  16U /* pixels per counter bit, on the top CNT_CELL rows */

#define COLOR_BG  0xFF101040U
#define COLOR_BAR 0xFFFFFFFFU
#define COLOR_ON  0xFF40FF40U
#define COLOR_OFF 0xFF404040U

/*
 * Full-frame, 32-byte aligned, pitch-matching buffers take the driver's
 * zero-copy path: the buffer itself becomes the scanout buffer. ARGB8888,
 * as set in the overlay.
 */
static uint32_t fb[2][WIDTH * HEIGHT] __aligned(32);

static void draw(uint32_t *p, uint32_t frame)
{
	uint32_t bar_x = (frame * BAR_STEP) % (WIDTH - BAR_W);

	for (uint32_t i = 0; i < WIDTH * HEIGHT; i++) {
		p[i] = COLOR_BG;
	}
	/* The bar position is the frame counter (times BAR_STEP, modulo). */
	for (uint32_t y = CNT_CELL; y < HEIGHT; y++) {
		for (uint32_t x = bar_x; x < bar_x + BAR_W; x++) {
			p[y * WIDTH + x] = COLOR_BAR;
		}
	}
	/* The low 16 bits of the counter, one cell per bit, MSB left. */
	for (uint32_t b = 0; b < CNT_BITS; b++) {
		uint32_t on = (frame >> (CNT_BITS - 1U - b)) & 1U;

		for (uint32_t y = 0; y < CNT_CELL; y++) {
			for (uint32_t x = b * CNT_CELL; x < (b + 1U) * CNT_CELL; x++) {
				p[y * WIDTH + x] = on ? COLOR_ON : COLOR_OFF;
			}
		}
	}
}

int main(void)
{
	const struct device *dev = DEVICE_DT_GET(DISP_NODE);
	struct display_capabilities caps;
	struct display_buffer_descriptor desc = {
		.buf_size = sizeof(fb[0]),
		.width = WIDTH,
		.height = HEIGHT,
		.pitch = WIDTH,
	};
	uint32_t prev_done = 0;
	int ret;

	if (!device_is_ready(dev)) {
		printk("display device not ready\n");
		return 0;
	}

	display_get_capabilities(dev, &caps);
	printk("display %ux%u formats 0x%x current 0x%x\n", caps.x_resolution,
	       caps.y_resolution, caps.supported_pixel_formats, caps.current_pixel_format);
	if (caps.x_resolution != WIDTH || caps.y_resolution != HEIGHT ||
	    caps.current_pixel_format != PIXEL_FORMAT_ARGB_8888) {
		printk("capabilities differ from the overlay, stopping\n");
		return 0;
	}

	/* The driver has no blanking_off hook; -ENOSYS is expected and harmless. */
	ret = display_blanking_off(dev);
	if (ret != 0 && ret != -ENOSYS) {
		printk("display_blanking_off failed: %d\n", ret);
		return 0;
	}

	/*
	 * The driver offers no VSYNC callback. Its display_write() starts the
	 * pipeline shadow load and blocks until the ExtDst0 shadow-load
	 * interrupt, so the time at which write returns is the closest signal
	 * available (inferred, not measured).
	 */
	printk("frame,write_start_cyc,write_return_cyc,write_us,return_to_return_us\n");

	for (uint32_t frame = 0;
	     CONFIG_IMX95_M7_SCANOUT_FRAMES == 0 || frame < CONFIG_IMX95_M7_SCANOUT_FRAMES;
	     frame++) {
		uint32_t *buf = fb[frame & 1U];
		uint32_t start, done;

		draw(buf, frame);

		start = k_cycle_get_32();
		ret = display_write(dev, 0, 0, &desc, buf);
		done = k_cycle_get_32();
		if (ret != 0) {
			printk("display_write failed: %d\n", ret);
			return 0;
		}

		/* Unsigned subtraction survives wrap of the 32-bit counter. */
		printk("%u,%u,%u,%u,%u\n", frame, start, done,
		       (uint32_t)k_cyc_to_us_floor32(done - start),
		       frame == 0 ? 0U : (uint32_t)k_cyc_to_us_floor32(done - prev_done));
		prev_done = done;
	}

	printk("done\n");
	return 0;
}

#endif /* CONFIG_IMX95_M7_SCANOUT_ENABLE_DPU */
