#!/bin/sh
# Load the SCR1 mailbox-echo firmware into hpmcu_sram (0xFF6FE000) and start the
# core, replicating flared/hpmcu.rs load_and_release() EXACTLY (the proven
# sequence): GRF uncached peripheral window -> CORECRU hold -> firmware to SRAM
# -> clear SRAM mailbox -> SGRF boot addr -> CORECRU release. SRAM path only
# (NOT the 0x40000 boot-load brick hazard).
#
# Do NOT kill warden-flared: it does a ONE-SHOT firmware load at boot then just
# beats the SRAM heartbeat + pets the kernel dw-wdt. Killing it stops the dw-wdt
# petting and the board resets ~15s later. We simply reset the SCR1 and reload;
# flared won't re-load over us (one-shot), and its heartbeat writes are to a
# different SRAM word than our echo firmware uses. (clk_core_mcu is kept alive by
# CLK_IGNORE_UNUSED in clk-rv1106.c, so the released core actually runs.)
FW=${1:-/userdata/echo-fw-words.txt}

# GRF uncached peripheral window (covers CRU + this SRAM + the mailbox
# 0xff5c0000) — WITHOUT this the MCU's peripheral/SRAM accesses are cached and
# invisible to Linux. hpmcu.rs: GRF_BASE 0xff040000 +0x24/+0x28 = 0xff000/0xffc00.
devmem 0xff040024 32 0xff000
devmem 0xff040028 32 0xffc00

# hold CORECRU MCU core in reset while we (re)write firmware
devmem 0xff3b8a04 32 0x001e001e

# write firmware words to 0xFF6FE000 (devmem uses mmap -> SRAM writable)
i=0
while read w; do
  a=$(printf '0x%x' $((0xff6fe000 + i * 4)))
  devmem "$a" 32 "$w"
  i=$((i + 1))
done < "$FW"

# clear the SRAM mailbox/debug area (0xFF6FFF00, 8 words) so stale state can't
# be mistaken for a live echo
j=0
while [ $j -lt 8 ]; do
  devmem "$(printf '0x%x' $((0xff6fff00 + j * 4)))" 32 0
  j=$((j + 1))
done

# set HPMCU boot addr, then release from reset
devmem 0xff076044 32 0xff6fe000
devmem 0xff3b8a04 32 0x001e0000
sleep 1
echo "loaded $i words; SCR1 released."
echo "  DBG_STATE = $(devmem 0xff6fff08 32)  (expect 0x584F424D = 'MBOX')"
