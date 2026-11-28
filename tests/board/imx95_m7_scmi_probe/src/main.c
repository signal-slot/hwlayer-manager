/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * i.MX95 Cortex-M7: read-only SCMI ownership probe of the display resources.
 *
 * Asks the System Manager (SM), from the M7's SCMI agent, what it reports
 * about the clocks and power domains of the display pipeline. Only queries
 * are sent: protocol version and attributes, base discovery, clock
 * attributes / rate / config / parent / permissions, and power-domain state.
 * This file sends no request that changes a clock, a power domain, a reset
 * line, an agent's permissions or a pin, and it reads no display register.
 *
 * The IDs and names come from docs/imx95-dpu-scmi-ids.md (section 2.1 and
 * section 3). Build-time checks below tie each number to the upstream Zephyr
 * binding where the binding defines it.
 *
 * Output: one line per query,
 *   SCMI <proto> id=<n> name=<name> result=<ok|denied|not_found|err=<code>> value=<...>
 * then one summary line. See README.md.
 */

#include <errno.h>
#include <stdint.h>
#include <string.h>

#include <zephyr/kernel.h>
#include <zephyr/version.h>
#include <zephyr/sys/printk.h>
#include <zephyr/sys/iterable_sections.h>
#include <zephyr/device.h>
#include <zephyr/devicetree.h>
#include <zephyr/drivers/firmware/scmi/protocol.h>
#include <zephyr/drivers/firmware/scmi/util.h>
#include <zephyr/drivers/firmware/scmi/base.h>
#include <zephyr/drivers/firmware/scmi/clk.h>
#include <zephyr/drivers/firmware/scmi/power.h>
#include <zephyr/dt-bindings/clock/imx95_clock.h>
#include <zephyr/dt-bindings/power/imx95_power.h>

/* ------------------------------------------------------------------------ */
/* Display IDs (docs/imx95-dpu-scmi-ids.md)                                  */
/* ------------------------------------------------------------------------ */

struct probe_id {
	uint32_t id;
	const char *name;
};

/*
 * Clock IDs, section 2.1 of docs/imx95-dpu-scmi-ids.md. ID 77 is reserved
 * in the SM header (DEV_SM_CLK_RESERVED77) but named DISPDP in the Linux and
 * Zephyr bindings; it is queried to see what the SM answers for it.
 */
static const struct probe_id clocks[] = {
	{ 18, "VIDEOPLL1_VCO" },
	{ 19, "VIDEOPLL1" },
	{ 37, "LDBPLL_VCO" },
	{ 38, "LDBPLL" },
	{ 62, "CAMAPB" },
	{ 66, "MIPIPHYCFG" },
	{ 67, "MIPIPHYPLLBYPASS" },
	{ 68, "MIPIPHYPLLREF" },
	{ 69, "MIPITESTBYTE" },
	{ 75, "DISPAPB" },
	{ 76, "DISPAXI" },
	{ 77, "RESERVED77_DISPDP" },
	{ 78, "DISPOCRAM" },
	{ 80, "DISP1PIX" },
	{ 81, "DISP2PIX" },
	{ 82, "DISP3PIX" },
};

/* Power domains, section 3 of docs/imx95-dpu-scmi-ids.md. */
static const struct probe_id power_domains[] = {
	{ 13, "DISPLAY" },
	{ 3, "CAMERA" },
};

/* The numbers above must equal the upstream Zephyr binding. */
BUILD_ASSERT(IMX95_CLK_VIDEOPLL1_VCO == 18);
BUILD_ASSERT(IMX95_CLK_VIDEOPLL1 == 19);
BUILD_ASSERT(IMX95_CLK_LDBPLL_VCO == 37);
BUILD_ASSERT(IMX95_CLK_LDBPLL == 38);
BUILD_ASSERT(IMX95_CLK_CAMAPB == 62);
BUILD_ASSERT(IMX95_CLK_MIPIPHYCFG == 66);
BUILD_ASSERT(IMX95_CLK_MIPIPHYPLLBYPASS == 67);
BUILD_ASSERT(IMX95_CLK_MIPIPHYPLLREF == 68);
BUILD_ASSERT(IMX95_CLK_MIPITESTBYTE == 69);
BUILD_ASSERT(IMX95_CLK_DISPAPB == 75);
BUILD_ASSERT(IMX95_CLK_DISPAXI == 76);
BUILD_ASSERT(IMX95_CLK_DISPDP == 77);
BUILD_ASSERT(IMX95_CLK_DISPOCRAM == 78);
BUILD_ASSERT(IMX95_CLK_DISP1PIX == 80);
BUILD_ASSERT(IMX95_CLK_DISP2PIX == 81);
BUILD_ASSERT(IMX95_CLK_DISP3PIX == 82);
BUILD_ASSERT(IMX95_PD_DISPLAY == 13);
BUILD_ASSERT(IMX95_PD_CAMERA == 3);

/* ------------------------------------------------------------------------ */
/* Result bookkeeping                                                        */
/* ------------------------------------------------------------------------ */

static struct {
	unsigned int queries;
	unsigned int ok;
	unsigned int denied;
	unsigned int not_found;
	unsigned int err;
} tally;

static const char *result_str(int ret, char *buf, size_t len)
{
	tally.queries++;
	if (ret == 0) {
		tally.ok++;
		return "ok";
	}
	if (ret == -EACCES) {
		tally.denied++;
		return "denied";
	}
	if (ret == -ENOENT) {
		tally.not_found++;
		return "not_found";
	}
	tally.err++;
	snprintk(buf, len, "err=%d", ret);
	return buf;
}

/* Print one query line. 'value' is printed only when ret == 0. */
static void line(const char *proto, uint32_t id, const char *name, int ret,
		 const char *value)
{
	char rbuf[16];
	const char *res = result_str(ret, rbuf, sizeof(rbuf));

	printk("SCMI %s id=%u name=%s result=%s value=%s\n", proto, id, name,
	       res, ret == 0 ? value : "-");
}

/* SCMI short names are at most 16 bytes and need not be terminated. */
static void copy_name(char *dst, const void *src)
{
	memcpy(dst, src, SCMI_SHORT_NAME_MAX_SIZE);
	dst[SCMI_SHORT_NAME_MAX_SIZE] = '\0';
	for (char *p = dst; *p != '\0'; p++) {
		if (*p == ' ') {
			*p = '_';
		}
	}
}

static struct scmi_protocol *find_protocol(uint32_t id)
{
	STRUCT_SECTION_FOREACH(scmi_protocol, it) {
		if (it->id == id) {
			return it;
		}
	}
	return NULL;
}

/* ------------------------------------------------------------------------ */
/* Base protocol and protocol versions                                       */
/* ------------------------------------------------------------------------ */

static void probe_protocols(void)
{
	char value[64];

	/* Every protocol this image knows, in link order. */
	STRUCT_SECTION_FOREACH(scmi_protocol, it) {
		uint32_t ver = 0;
		uint32_t attr = 0;
		int ret;

		ret = scmi_protocol_get_version(it, &ver);
		snprintk(value, sizeof(value), "0x%08x(%u.%u)", ver, ver >> 16,
			 ver & 0xffffU);
		line("protocol.version", it->id, "-", ret, value);

		ret = scmi_protocol_attributes_get(it, &attr);
		snprintk(value, sizeof(value), "0x%08x", attr);
		line("protocol.attributes", it->id, "-", ret, value);
	}
}

static void probe_base(void)
{
	struct scmi_platform_info info = { 0 };
	struct scmi_agent_info agent = { 0 };
	char name[SCMI_SHORT_NAME_MAX_SIZE + 1];
	char vendor[SCMI_SHORT_NAME_MAX_SIZE + 1];
	char sub[SCMI_SHORT_NAME_MAX_SIZE + 1];
	char value[96];
	int ret;

	/* BASE PROTOCOL_ATTRIBUTES + DISCOVER_VENDOR/SUB_VENDOR/IMPL_VERSION */
	ret = scmi_base_get_platform_info(&info);
	copy_name(vendor, info.vendor_id);
	copy_name(sub, info.sub_vendor_id);
	snprintk(value, sizeof(value),
		 "vendor=%s sub_vendor=%s impl=0x%08x protocols=%u agents=%u",
		 vendor, sub, info.impl_ver, info.num_protocols, info.num_agents);
	line("base.platform", SCMI_PROTOCOL_BASE, "-", ret, value);

	/* BASE_DISCOVER_AGENT for the caller: which agent is this M7? */
	ret = scmi_base_discover_agent(SCMI_BASE_AGENT_ID_OWN, &agent);
	copy_name(name, agent.name);
	snprintk(value, sizeof(value), "agent_id=%u agent_name=%s",
		 agent.agent_id, name);
	line("base.agent_self", SCMI_BASE_AGENT_ID_OWN, "OWN", ret, value);

	if (info.num_agents == 0U) {
		return;
	}

	/* BASE_DISCOVER_AGENT for every agent the platform reports. */
	for (uint32_t i = 0; i < info.num_agents; i++) {
		memset(&agent, 0, sizeof(agent));
		ret = scmi_base_discover_agent(i, &agent);
		copy_name(name, agent.name);
		snprintk(value, sizeof(value), "agent_id=%u agent_name=%s",
			 agent.agent_id, name);
		line("base.agent", i, "-", ret, value);
	}
}

/* ------------------------------------------------------------------------ */
/* Clocks                                                                    */
/* ------------------------------------------------------------------------ */

/*
 * Bit layouts used to decode the replies:
 * - CLOCK_ATTRIBUTES: bit 0 enabled, bit 1 restricted (Zephyr clk.h).
 *   The NXP SM sets "restricted" when the caller's permission is below
 *   EXCLUSIVE (docs/imx95-display-clocks.md section 4.2).
 * - CLOCK_GET_PERMISSIONS: bit 31 state control (Zephyr clk.h); bit 30
 *   parent control and bit 29 rate control (SCMI v3.2 specification).
 *   The NXP SM reports state at SET or above, rate and parent at EXCLUSIVE.
 * - CLOCK_CONFIG_GET: bit 0 of the config word is the enable state.
 */
#define ATTR_ON(a)         (((a) >> 0) & 1U)
#define ATTR_RESTRICTED(a) (((a) >> 1) & 1U)
#define PERM_STATE(p)      (((p) >> 31) & 1U)
#define PERM_PARENT(p)     (((p) >> 30) & 1U)
#define PERM_RATE(p)       (((p) >> 29) & 1U)

static void probe_clock(struct scmi_protocol *proto, const struct probe_id *c)
{
	struct scmi_clock_attributes attrs = { 0 };
	char sm_name[SCMI_CLK_NAME_LEN + 1];
	char value[96];
	uint32_t v = 0;
	int ret;

	ret = scmi_clock_attributes(proto, c->id, &attrs);
	memcpy(sm_name, attrs.clock_name, SCMI_CLK_NAME_LEN);
	sm_name[SCMI_CLK_NAME_LEN] = '\0';
	snprintk(value, sizeof(value), "0x%08x on=%u restricted=%u sm_name=%s",
		 attrs.attributes, ATTR_ON(attrs.attributes),
		 ATTR_RESTRICTED(attrs.attributes),
		 sm_name[0] != '\0' ? sm_name : "-");
	line("clock.attributes", c->id, c->name, ret, value);

	/* Only the lower 32 bits of the rate are returned by Zephyr. */
	ret = scmi_clock_rate_get(proto, c->id, &v);
	snprintk(value, sizeof(value), "%uHz", v);
	line("clock.rate", c->id, c->name, ret, value);

	v = 0;
	ret = scmi_clock_config_get(proto, c->id, 0, &v);
	snprintk(value, sizeof(value), "0x%08x on=%u", v, v & 1U);
	line("clock.config", c->id, c->name, ret, value);

	v = 0;
	ret = scmi_clock_parent_get(proto, c->id, &v);
	snprintk(value, sizeof(value), "parent=%u", v);
	line("clock.parent", c->id, c->name, ret, value);

	v = 0;
	ret = scmi_clock_get_permissions(proto, c->id, &v);
	snprintk(value, sizeof(value), "0x%08x state=%u parent=%u rate=%u", v,
		 PERM_STATE(v), PERM_PARENT(v), PERM_RATE(v));
	line("clock.permissions", c->id, c->name, ret, value);
}

static void probe_clocks(void)
{
	const struct device *clk_dev = DEVICE_DT_GET(DT_NODELABEL(scmi_clk));
	struct scmi_protocol *proto = clk_dev->data;

	if (proto == NULL || proto->id != SCMI_PROTOCOL_CLOCK) {
		/* Fall back to the iterable section if the device data differs. */
		proto = find_protocol(SCMI_PROTOCOL_CLOCK);
	}
	if (proto == NULL) {
		for (size_t i = 0; i < ARRAY_SIZE(clocks); i++) {
			line("clock.attributes", clocks[i].id, clocks[i].name,
			     -ENODEV, "-");
		}
		return;
	}

	for (size_t i = 0; i < ARRAY_SIZE(clocks); i++) {
		probe_clock(proto, &clocks[i]);
	}
}

/* ------------------------------------------------------------------------ */
/* Power domains                                                             */
/* ------------------------------------------------------------------------ */

static void probe_power(void)
{
	char value[48];

	for (size_t i = 0; i < ARRAY_SIZE(power_domains); i++) {
		uint32_t state = 0;
		int ret = scmi_power_state_get(power_domains[i].id, &state);
		const char *s = state == SCMI_POWER_STATE_GENERIC_ON    ? "on"
				: state == SCMI_POWER_STATE_GENERIC_OFF ? "off"
									: "other";

		snprintk(value, sizeof(value), "0x%08x(%s)", state, s);
		line("power.state", power_domains[i].id, power_domains[i].name,
		     ret, value);
	}
}

/* ------------------------------------------------------------------------ */

int main(void)
{
	printk("hwlm-imx95-m7-scmi-probe: read-only SCMI queries, Zephyr %s, %s\n",
	       KERNEL_VERSION_STRING, CONFIG_BOARD_TARGET);

	probe_protocols();
	probe_base();
	probe_clocks();
	probe_power();

	printk("SCMI-PROBE SUMMARY queries=%u ok=%u denied=%u not_found=%u err=%u "
	       "clocks=%u power_domains=%u\n",
	       tally.queries, tally.ok, tally.denied, tally.not_found, tally.err,
	       (unsigned int)ARRAY_SIZE(clocks),
	       (unsigned int)ARRAY_SIZE(power_domains));
	printk("hwlm-imx95-m7-scmi-probe: done\n");

	return 0;
}
