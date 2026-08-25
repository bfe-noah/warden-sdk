/*
 * HPMCU watchdog supervisor — linked into the hpmcu_sram carve-out.
 *
 * The RV1106's 256K on-chip SRAM at 0xFF6C0000 is split by the kernel DT
 * (rv1106.dtsi sram@ff6c0000) into rkisp_sram (0x0..0x3e000) and hpmcu_sram
 * (0x3e000..0x40000, i.e. 0xFF6FE000..0xFF700000). We live entirely inside
 * the 8K hpmcu region:
 *
 *   0xFF6FE000  +0x0000   .text/.rodata/.data/.bss   (this script: 0x1C00)
 *   0xFF6FFC00  +0x1C00   stack (grows down from 0xFF6FFF00)
 *   0xFF6FFF00  +0x1F00   heartbeat mailbox (shared with Linux, see main.c)
 */
OUTPUT_ARCH(riscv)
ENTRY(_start)

MEMORY
{
	SRAM (rwx) : ORIGIN = 0xFF6FE000, LENGTH = 0x1C00
}

SECTIONS
{
	.text : {
		*(.text.start)
		*(.text*)
	} > SRAM

	.rodata : { *(.rodata*) *(.srodata*) } > SRAM
	.data   : { *(.data*) *(.sdata*) } > SRAM

	.bss (NOLOAD) : {
		. = ALIGN(4);
		__bss_start = .;
		*(.bss*) *(.sbss*) *(COMMON)
		. = ALIGN(4);
		__bss_end = .;
	} > SRAM

	/* Stack: from the mailbox floor down toward .bss. */
	__stack_top = 0xFF6FFF00;

	/DISCARD/ : { *(.eh_frame) *(.riscv.attributes) *(.comment) }
}
