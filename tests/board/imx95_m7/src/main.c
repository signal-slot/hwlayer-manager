/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * i.MX95 Cortex-M7 hello and read-only DPU probe (hwlayer-manager board test).
 *
 * What it does:
 *   1. Builds a short text report: a banner, and (only when
 *      CONFIG_HWLM_DPU_PROBE=y) a few display controller registers.
 *   2. Prints the report on the console UART.
 *   3. With CONFIG_HWLM_RPMSG=y, waits for Linux remoteproc to bring up the
 *      rpmsg virtio device, announces one endpoint and answers every message
 *      from Linux with the same report.
 *
 * The program never writes a display register. See README.md for the build
 * target, the pinned Zephyr commit and the load procedure.
 *
 * The rpmsg set-up follows upstream Zephyr's
 * samples/subsys/ipc/openamp_rsc_table/src/main_remote.c
 * (Apache-2.0, Copyright (c) 2020 STMicroelectronics).
 */

#include <stdarg.h>
#include <stdio.h>
#include <string.h>

#include <zephyr/device.h>
#include <zephyr/devicetree.h>
#include <zephyr/kernel.h>
#include <zephyr/sys/printk.h>
#include <zephyr/sys/util.h>
#include <zephyr/version.h>

#ifdef CONFIG_HWLM_DPU_PROBE
#include <stddef.h>
#include <soc.h>
#include <zephyr/drivers/firmware/scmi/clk.h>
#include <zephyr/drivers/firmware/scmi/power.h>
#include <zephyr/dt-bindings/clock/imx95_clock.h>
#include <zephyr/dt-bindings/power/imx95_power.h>
#endif

#ifdef CONFIG_HWLM_RPMSG
#include <zephyr/drivers/ipm.h>
#include <openamp/open_amp.h>
#include <metal/io.h>
#include <metal/sys.h>
#include <addr_translation.h>
#include <resource_table.h>
#endif

#define BANNER_TAG "hwlm-imx95-m7"

/* The report is built once at boot and then only read. */
static char report[1024];
static size_t report_len;

static void report_add(const char *fmt, ...)
{
	va_list ap;
	int n;

	if (report_len >= sizeof(report) - 1) {
		return;
	}

	va_start(ap, fmt);
	n = vsnprintf(&report[report_len], sizeof(report) - report_len, fmt, ap);
	va_end(ap);

	if (n < 0) {
		return;
	}
	/* vsnprintf returns the untruncated length; clamp to what was stored. */
	report_len = MIN(report_len + (size_t)n, sizeof(report) - 1);
}

#ifdef CONFIG_HWLM_DPU_PROBE

#define DPU_BASE DT_REG_ADDR(DT_NODELABEL(dpu))
BUILD_ASSERT(DPU_BASE == DISPLAY_SEERIS_BASE,
	     "devicetree dpu node and HAL DISPLAY_SEERIS base disagree");

#define DPU_REG(member) (DPU_BASE + offsetof(DISPLAY_SEERIS_Type, member))
#define DISP_BLK_REG(member)                                                                       \
	(DISPLAY__BLK_CTRL_DISPLAYMIX_BASE + offsetof(DISPLAY_BLK_CTRL_DISPLAYMIX_Type, member))

/*
 * Load one 32-bit word with data bus faults ignored.
 *
 * With FAULTMASK set the core runs at priority -1, and with CCR.BFHFNMIGN set
 * a data bus fault at that priority does not raise an exception: the load
 * completes with an undefined value and BFSR records the error. This turns
 * "the M7 may not access this address" (for example a resource-domain
 * permission check that answers with a bus error) into an error code
 * instead of a HardFault. It does not help when the target never answers.
 *
 * Only M7 system registers (FAULTMASK, CCR, CFSR) are written here; the
 * target address is only read.
 */
static int safe_read32(uintptr_t addr, uint32_t *value)
{
	const uint32_t bus_err = SCB_CFSR_PRECISERR_Msk | SCB_CFSR_IMPRECISERR_Msk;
	const uint32_t faultmask = __get_FAULTMASK();
	const uint32_t ccr = SCB->CCR;
	int ret = 0;

	__disable_fault_irq();
	/* CFSR bits are write-one-to-clear; drop stale bus fault status. */
	SCB->CFSR = SCB_CFSR_BUSFAULTSR_Msk;
	SCB->CCR = ccr | SCB_CCR_BFHFNMIGN_Msk;
	__DSB();
	__ISB();

	*value = *(volatile const uint32_t *)addr;
	__DSB();
	__ISB();

	if ((SCB->CFSR & bus_err) != 0U) {
		SCB->CFSR = SCB_CFSR_BUSFAULTSR_Msk;
		*value = 0U;
		ret = -EIO;
	}

	SCB->CCR = ccr;
	__DSB();
	__ISB();
	if (faultmask == 0U) {
		__enable_fault_irq();
	}

	return ret;
}

static void report_reg(const char *name, uintptr_t addr)
{
	uint32_t value;
	int ret = safe_read32(addr, &value);

	if (ret == 0) {
		report_add("dpu: %-28s @0x%08lx = 0x%08x\n", name, (unsigned long)addr, value);
	} else {
		report_add("dpu: %-28s @0x%08lx bus error\n", name, (unsigned long)addr);
	}
}

/*
 * Gate the display reads on what the System Manager reports. Both SCMI
 * calls are queries (POWER_STATE_GET, CLOCK_CONFIG_GET); they change
 * nothing. If the M7 agent may not see the resource, the query fails and
 * the reads are skipped.
 */
static bool display_readable(void)
{
	const struct device *clk_dev = DEVICE_DT_GET(DT_NODELABEL(scmi_clk));
	struct scmi_protocol *proto = clk_dev->data;
	uint32_t state;
	uint32_t config;
	uint32_t rate;
	int ret;

	ret = scmi_power_state_get(IMX95_PD_DISPLAY, &state);
	if (ret != 0) {
		report_add("dpu: SCMI power state query (DISPLAY) failed: %d\n", ret);
		return false;
	}
	report_add("dpu: SCMI DISPLAY power state 0x%08x (%s)\n", state,
		   state == SCMI_POWER_STATE_GENERIC_ON ? "on" : "not on");
	if (state != SCMI_POWER_STATE_GENERIC_ON) {
		return false;
	}

	ret = scmi_clock_config_get(proto, IMX95_CLK_DISPAPB, 0, &config);
	if (ret != 0) {
		report_add("dpu: SCMI clock config query (DISPAPB) failed: %d\n", ret);
		return false;
	}
	ret = scmi_clock_rate_get(proto, IMX95_CLK_DISPAPB, &rate);
	report_add("dpu: SCMI DISPAPB clock config 0x%08x rate %u%s\n", config,
		   ret == 0 ? rate : 0U, ret == 0 ? " Hz" : " (rate query failed)");
	if (SCMI_CLK_CONFIG_ENABLE_DISABLE(config) == 0U) {
		return false;
	}

	return true;
}

static void dpu_probe(void)
{
	uint32_t ts0;
	uint32_t ts1;
	int ret0;
	int ret1;

	if (!display_readable()) {
		report_add("dpu: display not readable now; no display register was read\n");
		return;
	}

	report_reg("DISPLAYMIX CLOCK_CONTROL", DISP_BLK_REG(DISPLAY_ENGINES_CLOCK_CONTROL));
	report_reg("COMCTRL_IPIDENTIFIER", DPU_REG(COMCTRL_IPIDENTIFIER));
	report_reg("FRAMEGEN0_FGENSTS", DPU_REG(DISENG_FRAMEGEN0_FGENSTS));
	report_reg("FRAMEGEN0_HTCFG1", DPU_REG(DISENG_FRAMEGEN0_HTCFG1));
	report_reg("FRAMEGEN0_VTCFG1", DPU_REG(DISENG_FRAMEGEN0_VTCFG1));
	report_reg("FRAMEGEN0_FGCHSTAT", DPU_REG(DISENG_FRAMEGEN0_FGCHSTAT));

	/* Two time stamps 50 ms apart: if they differ, FrameGen0 is running. */
	ret0 = safe_read32(DPU_REG(DISENG_FRAMEGEN0_FGTIMESTAMP), &ts0);
	k_msleep(50);
	ret1 = safe_read32(DPU_REG(DISENG_FRAMEGEN0_FGTIMESTAMP), &ts1);
	if (ret0 == 0 && ret1 == 0) {
		report_add("dpu: FRAMEGEN0_FGTIMESTAMP 0x%08x -> 0x%08x after 50 ms (%s)\n", ts0,
			   ts1, ts0 != ts1 ? "changing" : "unchanged");
	} else {
		report_add("dpu: FRAMEGEN0_FGTIMESTAMP bus error\n");
	}
}

#endif /* CONFIG_HWLM_DPU_PROBE */

#ifdef CONFIG_HWLM_RPMSG

#if CONFIG_IPM_MAX_DATA_SIZE > 0
#define IPM_SEND(dev, w, id, d, s) ipm_send(dev, w, id, d, s)
#else
#define IPM_SEND(dev, w, id, d, s) ipm_send(dev, w, id, NULL, 0)
#endif

#define SHM_NODE       DT_CHOSEN(zephyr_ipc_shm)
#define SHM_START_ADDR DT_REG_ADDR(SHM_NODE)
#define SHM_SIZE       DT_REG_SIZE(SHM_NODE)

static const struct device *const ipm_handle = DEVICE_DT_GET(DT_CHOSEN(zephyr_ipc));

static metal_phys_addr_t shm_physmap = SHM_START_ADDR;
static metal_phys_addr_t rsc_tab_physmap;
static struct metal_io_region shm_io;
static struct metal_io_region rsc_io;
static struct rpmsg_virtio_device rvdev;
static struct rpmsg_endpoint ept;
static void *rsc_table;
static unsigned int rx_count;

static K_SEM_DEFINE(ipm_sem, 0, 1);

static void ipm_callback(const struct device *dev, void *context, uint32_t id,
			 volatile void *data)
{
	ARG_UNUSED(dev);
	ARG_UNUSED(context);
	ARG_UNUSED(id);
	ARG_UNUSED(data);

	k_sem_give(&ipm_sem);
}

static int mailbox_notify(void *priv, uint32_t id)
{
	ARG_UNUSED(priv);

	IPM_SEND(ipm_handle, 0, id, &id, sizeof(id));
	return 0;
}

static void new_service_cb(struct rpmsg_device *rdev, const char *name, uint32_t src)
{
	ARG_UNUSED(rdev);

	printk(BANNER_TAG ": unexpected name service \"%s\" from 0x%x\n", name, src);
}

/*
 * Every message from Linux is answered with the whole report, split into
 * rpmsg-sized chunks. The callback runs in the main thread (from
 * rproc_virtio_notified()), not in an interrupt, so sending here is allowed.
 */
static int ept_cb(struct rpmsg_endpoint *e, void *data, size_t len, uint32_t src, void *priv)
{
	int chunk = rpmsg_get_tx_buffer_size(e);
	size_t off = 0;

	ARG_UNUSED(data);
	ARG_UNUSED(priv);

	rx_count++;
	printk(BANNER_TAG ": rpmsg rx #%u, %u bytes from 0x%x\n", rx_count, (unsigned int)len,
	       src);

	if (chunk <= 0) {
		printk(BANNER_TAG ": no rpmsg tx buffer size (%d)\n", chunk);
		return RPMSG_SUCCESS;
	}

	while (off < report_len) {
		size_t n = MIN(report_len - off, (size_t)chunk);
		int ret = rpmsg_send(e, &report[off], (int)n);

		if (ret < 0) {
			printk(BANNER_TAG ": rpmsg_send failed: %d\n", ret);
			break;
		}
		off += n;
	}

	return RPMSG_SUCCESS;
}

static int rpmsg_start(void)
{
	struct metal_init_params metal_params = METAL_INIT_DEFAULTS;
	struct fw_rsc_vdev_vring *vring;
	struct virtio_device *vdev;
	struct rpmsg_device *rpdev;
	int rsc_size;
	int ret;

	ret = metal_init(&metal_params);
	if (ret != 0) {
		printk(BANNER_TAG ": metal_init failed: %d\n", ret);
		return ret;
	}

	metal_io_init(&shm_io, (void *)SHM_START_ADDR, &shm_physmap, SHM_SIZE, -1, 0,
		      addr_translation_get_ops(shm_physmap));

	rsc_table_get(&rsc_table, &rsc_size);
	rsc_tab_physmap = (uintptr_t)rsc_table;
	metal_io_init(&rsc_io, rsc_table, &rsc_tab_physmap, rsc_size, -1, 0, NULL);

	if (!device_is_ready(ipm_handle)) {
		printk(BANNER_TAG ": IPM device not ready\n");
		return -ENODEV;
	}
	ipm_register_callback(ipm_handle, ipm_callback, NULL);
	ret = ipm_set_enabled(ipm_handle, 1);
	if (ret != 0) {
		printk(BANNER_TAG ": ipm_set_enabled failed: %d\n", ret);
		return ret;
	}

	vdev = rproc_virtio_create_vdev(VIRTIO_DEV_DEVICE, VDEV_ID, rsc_table_to_vdev(rsc_table),
					&rsc_io, NULL, mailbox_notify, NULL);
	if (vdev == NULL) {
		printk(BANNER_TAG ": rproc_virtio_create_vdev failed\n");
		return -ENOMEM;
	}

	printk(BANNER_TAG ": waiting for the Linux rpmsg driver\n");
	rproc_virtio_wait_remote_ready(vdev);

	vring = rsc_table_get_vring0(rsc_table);
	ret = rproc_virtio_init_vring(vdev, 0, vring->notifyid, (void *)vring->da, &rsc_io,
				      vring->num, vring->align);
	if (ret == 0) {
		vring = rsc_table_get_vring1(rsc_table);
		ret = rproc_virtio_init_vring(vdev, 1, vring->notifyid, (void *)vring->da,
					      &rsc_io, vring->num, vring->align);
	}
	if (ret == 0) {
		ret = rpmsg_init_vdev(&rvdev, vdev, new_service_cb, &shm_io, NULL);
	}
	if (ret != 0) {
		printk(BANNER_TAG ": vring/rpmsg init failed: %d\n", ret);
		rproc_virtio_remove_vdev(vdev);
		return ret;
	}

	rpdev = rpmsg_virtio_get_rpmsg_device(&rvdev);
	ret = rpmsg_create_ept(&ept, rpdev, CONFIG_HWLM_RPMSG_EPT_NAME, RPMSG_ADDR_ANY,
			       RPMSG_ADDR_ANY, ept_cb, NULL);
	if (ret != 0) {
		printk(BANNER_TAG ": rpmsg_create_ept failed: %d\n", ret);
		return ret;
	}

	printk(BANNER_TAG ": rpmsg endpoint \"%s\" announced\n", CONFIG_HWLM_RPMSG_EPT_NAME);
	return 0;
}

static void rpmsg_loop(void)
{
	for (;;) {
		k_sem_take(&ipm_sem, K_FOREVER);
		rproc_virtio_notified(rvdev.vdev, VRING1_ID);
	}
}

#endif /* CONFIG_HWLM_RPMSG */

int main(void)
{
	report_add(BANNER_TAG ": hello from Zephyr %s on %s\n", STRINGIFY(BUILD_VERSION),
		   CONFIG_BOARD_TARGET);

#ifdef CONFIG_HWLM_DPU_PROBE
	dpu_probe();
#else
	report_add(BANNER_TAG ": DPU probe not built (CONFIG_HWLM_DPU_PROBE=n)\n");
#endif

	printk("%s", report);

#ifdef CONFIG_HWLM_RPMSG
	if (rpmsg_start() == 0) {
		rpmsg_loop();
	}
#else
	printk(BANNER_TAG ": rpmsg not built; UART only\n");
#endif

	return 0;
}
