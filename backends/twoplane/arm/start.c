/* SPDX-License-Identifier: Apache-2.0 */
/*
 * Start-up of the link-check image (main.c): a vector table with the reset
 * entry, then .data copied from flash, .bss cleared, main called. Nothing
 * else is set up (no clocks, no caches, no interrupts), and the image is
 * never run. The symbols come from the linker scripts.
 */
#include <stdint.h>

extern uint32_t __data_load[], __data_start[], __data_end[], __bss_start[], __bss_end[];
extern uint32_t __stack_top[];
extern int main(void);

void reset_c(void);

void reset_c(void)
{
	uint32_t *src = __data_load, *dst;

	for (dst = __data_start; dst < __data_end; dst++)
		*dst = *src++;
	for (dst = __bss_start; dst < __bss_end; dst++)
		*dst = 0;
	(void)main();
	for (;;)
		;
}

#if defined(__ARM_ARCH_PROFILE) && __ARM_ARCH_PROFILE == 'M'
/* Cortex-M: the core loads the stack pointer and the reset address itself. */
__attribute__((section(".isr_vector"), used))
void (*const vectors[])(void) = {
	(void (*)(void))__stack_top,
	reset_c,
};
#else
/* Cortex-R: the reset vector is the first instruction; set the stack, go. */
__attribute__((section(".isr_vector"), naked, used))
void reset_vector(void)
{
	__asm__ volatile("ldr sp, =__stack_top\n\tb reset_c");
}
#endif
