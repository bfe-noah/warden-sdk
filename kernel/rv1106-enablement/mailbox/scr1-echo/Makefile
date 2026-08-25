# HPMCU mailbox-echo firmware — bare-metal RV32IMC for the RV1106 SCR1 core.
# Same xPack riscv-none-embed-gcc 10.2.0 + flags as the watchdog firmware.

XPACK ?= /home/noah/projects/scada/flare-edge/sdk/sysdrv/source/mcu/prebuilts/gcc/linux-x86/riscv64/xpack-riscv-none-embed-gcc-10.2.0-1.2/bin
CROSS ?= $(XPACK)/riscv-none-embed-

CC      = $(CROSS)gcc
OBJCOPY = $(CROSS)objcopy
SIZE    = $(CROSS)size

CFLAGS  = -march=rv32imc -mabi=ilp32 -mcmodel=medany -Os -g \
          -ffreestanding -nostdlib -fno-builtin \
          -ffunction-sections -fdata-sections -Wall -Wextra
LDFLAGS = -T link.lds -nostdlib -Wl,--gc-sections -Wl,-Map=mailbox-echo.map

OBJS = start.o main.o

all: hpmcu-mailbox-echo.bin

hpmcu-mailbox-echo.elf: $(OBJS) link.lds
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $(OBJS)
	$(SIZE) $@

hpmcu-mailbox-echo.bin: hpmcu-mailbox-echo.elf
	$(OBJCOPY) -O binary $< $@
	@ls -la $@

%.o: %.c
	$(CC) $(CFLAGS) -c -o $@ $<

%.o: %.S
	$(CC) $(CFLAGS) -c -o $@ $<

clean:
	rm -f *.o *.elf *.bin *.map

.PHONY: all clean
