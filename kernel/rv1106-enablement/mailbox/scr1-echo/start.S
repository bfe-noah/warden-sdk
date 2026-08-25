/*
 * HPMCU watchdog entry. The BootROM-style loader (or flared's runtime
 * loader) points CORE_SGRF_HPMCU_BOOT_ADDR here and releases the SCR1 core
 * from CORECRU reset; there is no CRT, no interrupts, machine mode only.
 */
	.section .text.start, "ax"
	.globl _start
_start:
	/* Belt-and-braces: no traps expected, but if one fires, land on a
	 * tight loop at a known address rather than executing garbage. */
	la	t0, _trap
	csrw	mtvec, t0

	la	sp, __stack_top

	/* Clear .bss */
	la	a0, __bss_start
	la	a1, __bss_end
1:	bgeu	a0, a1, 2f
	sw	zero, 0(a0)
	addi	a0, a0, 4
	j	1b

2:	call	main
	/* main never returns; if it does, spin. */
_trap:
3:	j	3b
