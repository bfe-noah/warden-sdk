#!/bin/sh
# Prove the A7<->HPMCU mailbox round-trip on channel 0. Write A2B_CMD first, then
# A2B_DAT — the A2B_DAT write is the doorbell (raises A2B_STATUS on the MCU side,
# whose echo firmware has enabled A2B_INTEN), so both CMD and DAT are current when
# the SCR1 reads them. The SCR1 echoes {cmd,dat} verbatim into B2A_CMD/B2A_DAT.
CMD=${1:-0x00001234}
DAT=${2:-0xcafef00d}
echo "liveness: DBG_STATE=$(devmem 0xff6fff08 32) (want 0x584F424D 'MBOX')  echoes_before=$(devmem 0xff6fff10 32)"
devmem 0xff5c0008 32 "$CMD"      # A2B_CMD(0)
devmem 0xff5c000c 32 "$DAT"      # A2B_DAT(0) -- doorbell
sleep 1
echo "sent   CMD=$CMD DAT=$DAT"
echo "echoed B2A_CMD=$(devmem 0xff5c0030 32)  B2A_DAT=$(devmem 0xff5c0034 32)"
echo "echoes_after=$(devmem 0xff6fff10 32)"
