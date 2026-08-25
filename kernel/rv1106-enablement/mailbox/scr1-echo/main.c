/*
 * HPMCU mailbox echo firmware for the RV1106 (Syntacore SCR1, RV32IMC).
 *
 * Purpose: prove a fully-open A7 <-> HPMCU hardware-mailbox round-trip on our
 * self-built Linux 6.18. The A7 (Linux) writes a {cmd,dat} pair into the
 * mailbox A2B registers; this core polls A2B_STATUS, reads the pair, echoes it
 * verbatim into the B2A registers (which raises the B2A doorbell IRQ back to
 * Linux), and acks. No RT-Thread, no interrupts on the MCU side, machine mode
 * only: a polling loop, exactly like the watchdog firmware.
 *
 * The mailbox IP is at 0xFF5C0000 (the HPMCU-connected instance: Linux resets
 * it via SRST_CORE_MCU, the vendor MCU CMSIS header hard-codes MBOX_BASE
 * 0xFF5C0000). Register model is IP-identical to rk3368 (mainline
 * rockchip-mailbox.c): writing A2B_CMD(x) is the doorbell (hardware sets the
 * A2B_STATUS bit); writing B2A_CMD(x) signals Linux (B2A_STATUS + IRQ).
 *
 * Both the mailbox registers and hpmcu_sram sit in the GRF "peripheral
 * uncached" window (0xFF000000..0xFFC00000) configured at MCU release, so both
 * sides see each other's writes with no cache maintenance (same property the
 * watchdog SRAM mailbox relies on).
 *
 * Loaded + started by the proven flared/hpmcu.rs SRAM sequence
 * (0xFF6FE000 load addr, CORECRU reset hold/release) — NOT the 0x40000
 * boot-load path (that bricks a non-TB board; see boot-loaded-mcu-0x40000
 * hazard). Fits the 8K hpmcu_sram budget (this is a few hundred bytes).
 */

#include <stdint.h>

#define REG32(a)	(*(volatile uint32_t *)(uintptr_t)(a))

/* Mailbox IP (HPMCU-connected instance) — offsets match rockchip-mailbox.c. */
#define MBOX_BASE	0xFF5C0000u
#define A2B_INTEN	REG32(MBOX_BASE + 0x00)
#define A2B_STATUS	REG32(MBOX_BASE + 0x04)
#define A2B_CMD(x)	REG32(MBOX_BASE + 0x08 + (x) * 8)
#define A2B_DAT(x)	REG32(MBOX_BASE + 0x0c + (x) * 8)
#define B2A_STATUS	REG32(MBOX_BASE + 0x2C)
#define B2A_CMD(x)	REG32(MBOX_BASE + 0x30 + (x) * 8)
#define B2A_DAT(x)	REG32(MBOX_BASE + 0x34 + (x) * 8)

#define NUM_CHANS	4

/*
 * Liveness window in hpmcu_sram (reuse the watchdog mailbox layout at
 * 0xFF6FFF00 so Linux tooling can read it the same way). We only touch the
 * MCU-owned slots; the echo firmware does NOT run the watchdog, so it never
 * touches MB_MAGIC/MB_COUNTER or fires a reset.
 *
 *   +0x08  mcu_state : 'M','B','O','X' (0x584F424D LE) once the echo loop runs
 *   +0x0c  last_cmd  : the last cmd echoed (debug)
 *   +0x10  echo_cnt  : number of messages echoed (heartbeat / proof-of-life)
 */
#define SRAM_DBG_BASE	0xFF6FFF00u
#define DBG_STATE	REG32(SRAM_DBG_BASE + 0x08)
#define DBG_LASTCMD	REG32(SRAM_DBG_BASE + 0x0c)
#define DBG_ECHOCNT	REG32(SRAM_DBG_BASE + 0x10)

#define STATE_MBOX	0x584F424Du	/* "MBOX" (LE bytes 'M','B','O','X') */

void main(void)
{
	uint32_t echoes = 0;

	DBG_STATE = STATE_MBOX;
	DBG_LASTCMD = 0;
	DBG_ECHOCNT = 0;

	/* Enable the A2B doorbell for all channels: without A2B_INTEN set on the
	 * receiver (MCU) side, an A7 write to A2B_CMD does not raise A2B_STATUS,
	 * so our poll below never sees a message. (Symmetric to the A7 controller
	 * setting B2A_INTEN in its startup.) */
	A2B_INTEN = (1u << NUM_CHANS) - 1;

	for (;;) {
		uint32_t status = A2B_STATUS;
		int ch;

		for (ch = 0; ch < NUM_CHANS; ch++) {
			if (status & (1u << ch)) {
				uint32_t cmd = A2B_CMD(ch);
				uint32_t dat = A2B_DAT(ch);

				/* Echo verbatim. Writing B2A_CMD raises the B2A
				 * doorbell (hardware sets B2A_STATUS -> Linux
				 * IRQ / readable status). */
				B2A_CMD(ch) = cmd;
				B2A_DAT(ch) = dat;

				/* Ack: clear our A2B_STATUS bit (write-1-clear). */
				A2B_STATUS = (1u << ch);

				DBG_LASTCMD = cmd;
				DBG_ECHOCNT = ++echoes;
			}
		}
	}
}
