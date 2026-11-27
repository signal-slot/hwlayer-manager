/* Link-check image for a Cortex-R5 (armv7r-none-eabihf). The addresses
 * are those of no particular board; the image is never run.
 */
ENTRY(reset_vector)
MEMORY
{
	FLASH (rx)  : ORIGIN = 0x00000000, LENGTH = 1M
	RAM   (rwx) : ORIGIN = 0x00100000, LENGTH = 256K
}
SECTIONS
{
	.text : {
		KEEP(*(.isr_vector))
		*(.text .text.*)
		*(.rodata .rodata.*)
	} > FLASH
	.ARM.exidx : { *(.ARM.exidx*) } > FLASH
	.data : ALIGN(4) {
		__data_start = .;
		*(.data .data.*)
		. = ALIGN(4);
		__data_end = .;
	} > RAM AT > FLASH
	__data_load = LOADADDR(.data);
	.bss (NOLOAD) : ALIGN(4) {
		__bss_start = .;
		*(.bss .bss.* COMMON)
		. = ALIGN(4);
		__bss_end = .;
	} > RAM
	__stack_top = ORIGIN(RAM) + LENGTH(RAM);
	/DISCARD/ : { *(.note.*) }
}
