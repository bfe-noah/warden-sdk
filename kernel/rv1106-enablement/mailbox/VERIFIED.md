# HPMCU mailbox — [x] 100% VERIFIED on warden-c8a3 (2026-08-25)

A fully-open A7 ↔ HPMCU (RISC-V SCR1) hardware-mailbox round-trip on our self-built
Linux 6.18.46. Open kernel driver + open SCR1 firmware, **zero blobs**.

## Evidence (serial, _b slot = our 6.18)
SCR1 echo firmware running: `DBG_STATE = 0x584F424D` ("MBOX"). Five round-trips,
Linux → mailbox → SCR1 → mailbox → Linux, **all exact**:
```
sent 0x0000beef/0x600df00d -> B2A 0x0000BEEF/0x600DF00D  e=4  OK
sent 0x0000c0de/0x12345678 -> B2A 0x0000C0DE/0x12345678  e=5  OK
sent 0x0000face/0xdeadbeef -> B2A 0x0000FACE/0xDEADBEEF  e=6  OK
sent 0x00001234/0xcafef00d -> B2A 0x00001234/0xCAFEF00D  e=7  OK
sent 0x0000aa55/0x55aa55aa -> B2A 0x0000AA55/0x55AA55AA  e=8  OK
```
Echo counter increments 1:1 with sends; both CMD and DAT echo back verbatim.

## The three fixes it took (none in the research plan — found on hardware)
1. **Controller IRQ count** (`rockchip-mailbox.c`): the rv1106 mailbox has ONE
   shared IRQ (GIC_SPI 1), but the rk3368 driver-data assumes one IRQ per channel
   (num_chans=4) and probe failed `IRQ index 1 not found`. Added an
   `rv1106_drv_data { .num_chans = 1 }` + a `rockchip,rv1106-mailbox` match entry
   (ahead of the rk3368 fallback). Channel 0 is all the doorbell needs. Controller
   then probes clean and clocks the mailbox (pclk_mailbox on).
2. **SCR1 core clock** (`clk-rv1106.c`): `clk_core_mcu` (the coprocessor's 297 MHz
   core clock) had flags 0, so 6.18's `clk_disable_unused()` switched it off and the
   released core never executed (DBG_STATE stayed 0). Marked it `CLK_IGNORE_UNUSED`.
   (5.10 happened to leave it on.)
3. **A2B doorbell semantics** (SCR1 firmware + send order): the MCU-side receiver
   must set `A2B_INTEN` or an A7 write to A2B_CMD never raises A2B_STATUS — the echo
   firmware now sets `A2B_INTEN` at init. And the A2B_DAT write is the doorbell, so
   the sender writes CMD first, then DAT (the mainline order), so both are current
   when the SCR1 reads them.

## Loading the firmware (no reflash, no brick)
The SCR1 echo firmware (`scr1-echo/`, 154 B) is loaded at runtime into hpmcu_sram
(0xFF6FE000) via the proven `flared/hpmcu.rs` sequence (`load-echo-fw.sh`): GRF
uncached peripheral window (0xff040024/28) → CORECRU hold → firmware to SRAM →
SGRF boot addr → CORECRU release. **Do NOT kill warden-flared** (it one-shot-loads
at boot then just beats + pets the dw-wdt; killing it resets the board). We reset
+ reload the SCR1; flared does not re-load. The SRAM-load path avoids the 0x40000
boot-load brick hazard entirely.

## Config / DT
`CONFIG_ROCKCHIP_MBOX=y`; board DTS `&mailbox { status = "okay"; }` (the @ff5c0000
HPMCU-connected instance). The existing /dev/mem SRAM watchdog is untouched (a
separate dead-man's-switch); the mailbox is the general open IPC channel.

## Files
- `scr1-echo/main.c` + `start.S` + `link.lds` + `Makefile` — the open SCR1 echo fw.
- `scr1-echo/load-echo-fw.sh`, `mbox-roundtrip.sh`, `echo-fw-words.txt` — load + test.
- Kernel deltas: `rockchip-mailbox.c` (rv1106 num_chans=1), `clk-rv1106.c`
  (CLK_CORE_MCU IGNORE_UNUSED), `rv1106-warden.dts` (&mailbox okay).
